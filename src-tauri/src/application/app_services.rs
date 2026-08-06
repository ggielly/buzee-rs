use crate::application::events::UiEvent;
use crate::domain::IgnoreAllowCacheState;
use crate::domain::events::WorkerEvent;
use crate::domain::types::{DbPool, UserPreferencesState};
use crate::infrastructure::platform::PlatformService;
use crossbeam_channel::Sender;
use std::sync::atomic::Ordering;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, RwLock};
use tantivy::{Index, IndexReader, IndexWriter};
use tokio::runtime::Runtime;

/// Shared services behind the whole UI. Each field is designed to be freely
/// shared across worker threads without a global matching-lock.
pub struct AppServices {
    pub db_pool: DbPool,
    pub tantivy_reader: Arc<IndexReader>,
    pub tantivy_writer: Arc<Mutex<IndexWriter>>,
    pub tantivy_index: Arc<Index>,
    pub preferences: Arc<RwLock<UserPreferencesState>>,
    pub sync: Arc<SyncController>,
    pub ocr: Arc<OcrController>,
    pub ignore_allow_cache: Arc<RwLock<IgnoreAllowCacheState>>,
    pub platform: Arc<dyn PlatformService>,
    pub event_tx: Sender<UiEvent>,
    runtime: Arc<Runtime>,
}

impl AppServices {
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Emit a low-level worker event wrapped as a UI status event, best-effort.
    pub fn emit(&self, event: WorkerEvent) {
        let _ = self.event_tx.send(UiEvent::Status(event));
    }
}

/// Build the shared services from the bootstrapped resources.
pub fn build(services: BuildServices, event_tx: Sender<UiEvent>) -> Arc<AppServices> {
    let runtime = Arc::new(Runtime::new().expect("Failed to build Tokio runtime"));

    let sync = Arc::new(SyncController::new(
        runtime.clone(),
        services.preferences.clone(),
        services.db_pool.clone(),
        services.tantivy_writer.clone(),
        services.ignore_allow_cache.clone(),
        event_tx.clone(),
    ));

    let ocr = Arc::new(OcrController::new(
        runtime.clone(),
        services.preferences.clone(),
        services.db_pool.clone(),
        services.tantivy_writer.clone(),
        services.ignore_allow_cache.clone(),
        event_tx.clone(),
    ));

    Arc::new(AppServices {
        db_pool: services.db_pool,
        tantivy_reader: services.tantivy_reader,
        tantivy_writer: services.tantivy_writer,
        tantivy_index: services.tantivy_index,
        preferences: services.preferences,
        sync,
        ocr,
        ignore_allow_cache: services.ignore_allow_cache,
        platform: services.platform,
        event_tx,
        runtime,
    })
}

/// Inputs required to assemble `AppServices`. Kept separate so the bootstrap in
/// `main` can construct the expensive resources once and hand them over.
pub struct BuildServices {
    pub db_pool: DbPool,
    pub tantivy_reader: Arc<IndexReader>,
    pub tantivy_writer: Arc<Mutex<IndexWriter>>,
    pub tantivy_index: Arc<Index>,
    pub preferences: Arc<RwLock<UserPreferencesState>>,
    pub ignore_allow_cache: Arc<RwLock<IgnoreAllowCacheState>>,
    pub platform: Arc<dyn PlatformService>,
}

/// Tracks the background file-sync job. Mirrors the old `SyncRunningState`.
pub struct SyncController {
    pub running: Arc<AtomicBool>,
    pub last_sync_time: Arc<AtomicI64>,
    /// Incremented on every scan start; a scan finalizes only if it is still the
    /// current generation (so a takeover scan's end never clobbers the newer one).
    generation: Arc<AtomicU64>,
    runtime: Arc<Runtime>,
    preferences: Arc<RwLock<UserPreferencesState>>,
    pub db_pool: DbPool,
    pub tantivy_writer: Arc<Mutex<IndexWriter>>,
    pub ignore_allow_cache: Arc<RwLock<IgnoreAllowCacheState>>,
    event_tx: Sender<UiEvent>,
}

impl SyncController {
    pub fn new(
        runtime: Arc<Runtime>,
        preferences: Arc<RwLock<UserPreferencesState>>,
        db_pool: DbPool,
        tantivy_writer: Arc<Mutex<IndexWriter>>,
        ignore_allow_cache: Arc<RwLock<IgnoreAllowCacheState>>,
        event_tx: Sender<UiEvent>,
    ) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            last_sync_time: Arc::new(AtomicI64::new(0)),
            generation: Arc::new(AtomicU64::new(0)),
            runtime,
            preferences,
            db_pool,
            tantivy_writer,
            ignore_allow_cache,
            event_tx,
        }
    }

    fn emit(&self, event: WorkerEvent) {
        let _ = self.event_tx.send(UiEvent::Status(event));
    }

    /// Snapshot of the sync status, matching `sync_status`.
    pub fn snapshot(&self) -> (String, i64) {
        let running = self.running.load(Ordering::SeqCst);
        let last_sync_time = self.last_sync_time.load(Ordering::SeqCst);
        if running {
            ("true".to_string(), last_sync_time)
        } else {
            ("false".to_string(), last_sync_time)
        }
    }

    /// Set the DB scan_running flag and in-memory running state (matching
    /// `set_scan_running_status`).
    pub fn set_scan_running(&self, status: bool, set_time: bool, conn: &mut diesel::SqliteConnection) {
        set_scan_running_state(&self.running, &self.last_sync_time, status, set_time, conn);
    }

    /// Run a full sync (walk + optional content parse). Toggles the running flag
    /// on/off and streams progress events. Spawns the work on the shared runtime.
    /// When `clear_first` is set the search index is wiped before walking
    /// ("rescan all"); this happens inside the background task so the worker
    /// thread never blocks on the potentially slow index clear.
    pub fn run_sync(&self, switch_off: bool, file_paths: Vec<String>, clear_first: bool, popup: bool) {
        let already_running = self.running.load(Ordering::SeqCst);
        // A user-triggered scan while one is running takes over: stop the old
        // one and start a fresh scan below.
        let restart = already_running && !switch_off;
        if already_running || switch_off {
            log::info!("File sync already running; stopping now");
            if let Ok(mut conn) =
                crate::infrastructure::database::establish_direct_connection_to_db()
            {
                self.set_scan_running(false, true, &mut conn);
            }
            self.emit(WorkerEvent::SyncStatus { running: false, popup });
            if !restart {
                return;
            }
        }

        self.emit(WorkerEvent::SyncStatus { running: true, popup });

        let runtime = self.runtime.clone();
        let rt_future = runtime.clone();
        let db_pool = self.db_pool.clone();
        let preferences = self.preferences.clone();
        let writer = self.tantivy_writer.clone();
        let ignore_allow_cache = self.ignore_allow_cache.clone();
        let event_tx = self.event_tx.clone();
        let running = self.running.clone();
        let last_sync_time = self.last_sync_time.clone();
        let mut file_paths = file_paths;
        let my_gen = self.generation.fetch_add(1, Ordering::SeqCst);
        let gen_ref = self.generation.clone();

        runtime.spawn(async move {
            let mut conn = db_pool.get().expect("Failed to get DB connection");
            set_scan_running_state(&running, &last_sync_time, true, true, &mut conn);
            drop(conn);

            if clear_first {
                log::info!("Rescan all: clearing the search index");
                let _ = crate::infrastructure::tantivy_index::delete_all_docs_from_index(&writer);
                if let Ok(mut clear_conn) = db_pool.get() {
                    crate::infrastructure::indexing::clear_last_parsed_dates_from_db(&mut clear_conn);
                }
            }

            let home_directory =
                crate::infrastructure::housekeeping::get_home_directory()
                    .unwrap_or("/".to_string());
            if file_paths.is_empty() {
                file_paths.push(home_directory);
            }

            let ignore_cache = ignore_allow_cache.read().unwrap_or_else(|e| e.into_inner()).clone();
            let prefs_snapshot = preferences.read().unwrap_or_else(|e| e.into_inner()).clone();
            let detailed_scan = prefs_snapshot.detailed_scan;

            let db_pool_walk = db_pool.clone();
            let ignore_cache_walk = ignore_cache.clone();
            let event_tx_walk = event_tx.clone();
            let running_walk = running.clone();
            let files_added = tokio::task::spawn_blocking(move || {
                let mut conn = db_pool_walk.get().expect("Failed to get DB connection");
                crate::application::sync_ops::walk_and_index(
                    &mut conn,
                    file_paths,
                    &ignore_cache_walk,
                    &prefs_snapshot,
                    &event_tx_walk,
                    &running_walk,
                )
            })
            .await
            .unwrap_or(0);
            log::info!("Files added/updated: {}", files_added);

            if detailed_scan {
                log::info!("Parsing content from files");
                let pool_parse = db_pool.clone();
                let prefs_parse = preferences.clone();
                let writer_parse = writer.clone();
                let ignore_parse = ignore_cache.clone();
                let tx_parse = event_tx.clone();
                let running_parse = running.clone();
                let rt_parse = rt_future.clone();
                let files_parsed = tokio::task::spawn_blocking(move || {
                    let getter = move || running_parse.load(Ordering::SeqCst);
                    rt_parse.block_on(crate::application::sync_ops::parse_files(
                        &pool_parse,
                        &prefs_parse,
                        &writer_parse,
                        &ignore_parse,
                        &tx_parse,
                        getter,
                    ))
                })
                .await
                .unwrap_or(0);
                log::info!("Files parsed: {}", files_parsed);
            }

            // Finalize only if this scan is still the active generation, so a takeover
            // scan's end never clobbers the flag/events of the newer scan.
if gen_ref.load(Ordering::SeqCst) == my_gen {
                let mut end_conn = db_pool.get().expect("Failed to get DB connection");
                set_scan_running_state(&running, &last_sync_time, false, true, &mut end_conn);
                let _ = event_tx.send(UiEvent::Status(WorkerEvent::SyncStatus { running: false, popup: false }));
                let _ = event_tx.send(UiEvent::SyncFinished);
            }
        });
    }

    /// Start the periodic background-sync timer (interval 1800 s), mirroring
    /// `setup_cron_job`. The check runs unconditionally; the sync only starts when
    /// `automatic_background_sync` is enabled and the last run is older than 1800 s.
    pub fn schedule(&self) {
        let runtime = self.runtime.clone();
        let rt_future = runtime.clone();
        let preferences = self.preferences.clone();
        let running = self.running.clone();
        let last_sync_time = self.last_sync_time.clone();
        let event_tx = self.event_tx.clone();
        let db_pool = self.db_pool.clone();
        let writer = self.tantivy_writer.clone();
        let gen_arc = self.generation.clone();
        let ignore_allow_cache = self.ignore_allow_cache.clone();

        runtime.spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1800));
            interval.tick().await;
            loop {
                interval.tick().await;
                let enabled = preferences
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .automatic_background_sync;
                if !enabled {
                    continue;
                }
                let is_running = running.load(Ordering::SeqCst);
                let last = last_sync_time.load(Ordering::SeqCst);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                if !is_running && now - last > 1800 {
                    log::info!("Background sync timer fired");
                    let mut conn = db_pool.get().expect("Failed to get DB connection");
                    set_scan_running_state(&running, &last_sync_time, true, true, &mut conn);
                    drop(conn);
                    let _ = event_tx.send(UiEvent::Status(WorkerEvent::SyncStatus { running: true, popup: false }));
                    let my_gen = gen_arc.fetch_add(1, Ordering::SeqCst);
                    let gen_ref = gen_arc.clone();

                    let ignore_cache = ignore_allow_cache.read().unwrap_or_else(|e| e.into_inner()).clone();
                    let prefs_snapshot = preferences.read().unwrap_or_else(|e| e.into_inner()).clone();
                    let detailed_scan = prefs_snapshot.detailed_scan;
                    let db_pool2 = db_pool.clone();
                    let prefs2 = preferences.clone();
                    let tx2 = event_tx.clone();
                    let running2 = running.clone();
                    let last2 = last_sync_time.clone();

                    let ignore_cache2 = ignore_cache.clone();
                    let running_walk = running2.clone();
                    let files_added = tokio::task::spawn_blocking(move || {
                        let mut conn = db_pool2.get().expect("Failed to get DB connection");
                        crate::application::sync_ops::walk_and_index(
                            &mut conn,
                            vec![crate::infrastructure::housekeeping::get_home_directory()
                                .unwrap_or("/".to_string())],
                            &ignore_cache2,
                            &prefs_snapshot,
                            &tx2,
                            &running_walk,
                        )
                    })
                    .await
                    .unwrap_or(0);
                    log::info!("Background sync: files added/updated: {}", files_added);

                    if detailed_scan {
                        let pool_parse = db_pool.clone();
                        let prefs_parse = prefs2.clone();
                        let writer_parse = writer.clone();
                        let ignore_parse = ignore_cache.clone();
                        let tx_parse = event_tx.clone();
                        let running_parse = running.clone();
                        let rt_parse = rt_future.clone();
                        let files_parsed = tokio::task::spawn_blocking(move || {
                            let getter = move || running_parse.load(Ordering::SeqCst);
                            rt_parse.block_on(crate::application::sync_ops::parse_files(
                                &pool_parse,
                                &prefs_parse,
                                &writer_parse,
                                &ignore_parse,
                                &tx_parse,
                                getter,
                            ))
                        })
                        .await
                        .unwrap_or(0);
                        log::info!("Background sync: files parsed: {}", files_parsed);
                    }

                    if gen_ref.load(Ordering::SeqCst) == my_gen {
                        let mut end_conn = db_pool.get().expect("Failed to get DB connection");
                        set_scan_running_state(&running2, &last2, false, true, &mut end_conn);
                        let _ = event_tx.send(UiEvent::Status(WorkerEvent::SyncStatus { running: false, popup: false }));
                        let _ = event_tx.send(UiEvent::SyncFinished);
                    }
                }
            }
        });
    }
}

/// Shared implementation of the scan_running state write (in-memory + DB).
pub fn set_scan_running_state(
    running: &Arc<AtomicBool>,
    last_sync_time: &Arc<AtomicI64>,
    status: bool,
    set_time: bool,
    conn: &mut diesel::SqliteConnection,
) {
    use crate::infrastructure::database::schema::app_data;
    use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

    log::info!("Setting scan_running status to: {}", status);
    running.store(status, Ordering::SeqCst);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    if set_time {
        last_sync_time.store(now, Ordering::SeqCst);
        write_scan_running(conn, |conn| {
            diesel::update(app_data::table)
                .set((app_data::scan_running.eq(status), app_data::last_scan_time.eq(now)))
                .execute(conn)
        });
    } else {
        write_scan_running(conn, |conn| {
            diesel::update(app_data::table)
                .set(app_data::scan_running.eq(status))
                .execute(conn)
        });
    }
}

/// Best-effort `UPDATE` that tolerates a transient SQLite lock: retries a few
/// times with a short back-off and logs instead of panicking, so a busy database
/// cannot kill the sync job.
fn write_scan_running(
    conn: &mut diesel::SqliteConnection,
    exec: impl Fn(&mut diesel::SqliteConnection) -> diesel::QueryResult<usize>,
) {
    for attempt in 1..=5u32 {
        match exec(conn) {
            Ok(_) => return,
            Err(e) => {
                log::warn!("Could not update scan_running (attempt {}): {}", attempt, e);
                if attempt < 5 {
                    std::thread::sleep(std::time::Duration::from_millis(150 * attempt as u64));
                }
            }
        }
    }
}

/// Tracks the OCR rescan job, mirroring the old `OcrRescanState`.
pub struct OcrController {
    pub running: Arc<AtomicBool>,
    pub cancelled: Arc<AtomicBool>,
    pub failed_files: Mutex<Vec<crate::domain::types::OcrFailedFile>>,
    pub success_files: Mutex<Vec<crate::domain::types::OcrSuccessFile>>,
    runtime: Arc<Runtime>,
    preferences: Arc<RwLock<UserPreferencesState>>,
    pub db_pool: DbPool,
    pub tantivy_writer: Arc<Mutex<IndexWriter>>,
    pub ignore_allow_cache: Arc<RwLock<IgnoreAllowCacheState>>,
    event_tx: Sender<UiEvent>,
}

impl OcrController {
    pub fn new(
        runtime: Arc<Runtime>,
        preferences: Arc<RwLock<UserPreferencesState>>,
        db_pool: DbPool,
        tantivy_writer: Arc<Mutex<IndexWriter>>,
        ignore_allow_cache: Arc<RwLock<IgnoreAllowCacheState>>,
        event_tx: Sender<UiEvent>,
    ) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            failed_files: Mutex::new(vec![]),
            success_files: Mutex::new(vec![]),
            runtime,
            preferences,
            db_pool,
            tantivy_writer,
            ignore_allow_cache,
            event_tx,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Attempt to start a rescan. Returns `Err(AppError::JobAlreadyRunning)` when
    /// a rescan is already active (guards against double-start).
    pub fn try_start(&self) -> Result<(), crate::domain::AppError> {
        let already = self.running.swap(true, Ordering::SeqCst);
        if already {
            return Err(crate::domain::AppError::JobAlreadyRunning);
        }
        self.cancelled.store(false, Ordering::SeqCst);
        if let Ok(mut failed) = self.failed_files.lock() {
            failed.clear();
        }
        if let Ok(mut success) = self.success_files.lock() {
            success.clear();
        }
        Ok(())
    }

    pub fn cancel(&self) {
        if !self.is_running() {
            return;
        }
        log::info!("Stopping OCR rescan");
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn failed_files_snapshot(&self) -> Vec<crate::domain::types::OcrFailedFile> {
        self.failed_files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn success_files_snapshot(&self) -> Vec<crate::domain::types::OcrSuccessFile> {
        self.success_files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Start a full OCR rescan of every eligible document.
    pub fn start_full_rescan(self: Arc<Self>) -> Result<(), crate::domain::AppError> {
        self.try_start()?;
        mark_db_scan_running(true);

        let threads = crate::infrastructure::user_prefs::get_ocr_threads(&self.preferences);
        let sort_order = crate::infrastructure::user_prefs::get_ocr_sort_order(&self.preferences);

        self.launch(sort_order, threads, None);
        Ok(())
    }

    /// Start a targeted OCR rescan of the given paths.
    pub fn start_targeted_rescan(
        self: Arc<Self>,
        paths: Vec<String>,
    ) -> Result<(), crate::domain::AppError> {
        if paths.is_empty() {
            return Ok(());
        }
        self.try_start()?;
        mark_db_scan_running(true);

        let threads = crate::infrastructure::user_prefs::get_ocr_threads(&self.preferences);
        self.launch("size_asc".to_string(), threads, Some(paths));
        Ok(())
    }

    fn launch(self: Arc<Self>, sort_order: String, threads: i64, paths: Option<Vec<String>>) {
        let runtime = self.runtime.clone();
        let db_pool = self.db_pool.clone();
        let preferences = self.preferences.clone();
        let writer = self.tantivy_writer.clone();
        let ignore_allow_cache = self.ignore_allow_cache.clone();
        let event_tx = self.event_tx.clone();
        let ocr_self = self;
        let running = ocr_self.running.clone();
        let cancelled = ocr_self.cancelled.clone();

        runtime.spawn(async move {
            let ignore_cache = ignore_allow_cache.read().unwrap_or_else(|e| e.into_inner()).clone();
            let completed = crate::application::sync_ops::rescan_ocr(
                &db_pool,
                &preferences,
                &writer,
                &ignore_cache,
                &event_tx,
                &ocr_self,
                sort_order,
                threads,
                paths,
            )
            .await;

            mark_db_scan_running(false);
            running.store(false, Ordering::SeqCst);
            cancelled.store(false, Ordering::SeqCst);
            log::info!("OCR rescan finished, completed: {}", completed);
        });
    }
}

/// Update the `app_data.scan_running` flag (used to reflect an active OCR rescan
/// in the status bar). This does not touch the sync controller's own timer.
pub fn mark_db_scan_running(status: bool) {
    use crate::infrastructure::database::schema::app_data;
    use diesel::{ExpressionMethods, RunQueryDsl};
    if let Ok(mut conn) = crate::infrastructure::database::establish_direct_connection_to_db() {
        if let Err(e) = diesel::update(app_data::table)
            .set(app_data::scan_running.eq(status))
            .execute(&mut conn)
        {
            log::error!("Could not mark DB scan_running={}: {}", status, e);
        }
    }
}