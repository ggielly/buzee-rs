use crate::application::OcrController;
use crate::application::UiEvent;
use crate::domain::IgnoreAllowCacheState;
use crate::domain::events::WorkerEvent;
use crate::domain::types::{DbPool, OcrRescanProgress, UserPreferencesState};
use crate::infrastructure::database::establish_connection;
use crate::infrastructure::database::models::BodyItem;
use crate::infrastructure::database::schema::{document, metadata};
use crate::infrastructure::indexing::{
    self, add_body_to_database_public, add_file_metadata_to_database, chunk_text,
    create_document_item, extract_text_from_path_with_error,
};
use crate::infrastructure::housekeeping::get_home_directory;
use crate::infrastructure::tantivy_index;
use crossbeam_channel::Sender;
use diesel::prelude::*;
use futures::{pin_mut, StreamExt};
use ignore::{Walk, WalkBuilder};
use std::sync::atomic::Ordering;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Send a low-level progress event to the UI channel, wrapped as a status event.
fn emit(tx: &crossbeam_channel::Sender<UiEvent>, event: WorkerEvent) {
    let _ = tx.send(UiEvent::Status(event));
}

fn forbidden_directories() -> Vec<String> {
    let mut all_forbidden_directories: Vec<String> = vec![];
    let forbidden_directories: [&str; 4] = ["node_modules", "venv", "bower_components", "pycache"];
    all_forbidden_directories.extend(forbidden_directories.iter().map(|&s| s.to_string()));
    #[cfg(target_os = "windows")]
    {
        let windows_forbidden_directories: [&str; 6] = [
            "$RECYCLE.BIN",
            "System Volume Information",
            "AppData",
            "ProgramData",
            "Windows",
            "Program Files",
        ];
        all_forbidden_directories.extend(windows_forbidden_directories.iter().map(|&s| s.to_string()));
    }
    #[cfg(target_os = "macos")]
    {
        let home_dir: String = get_home_directory().unwrap_or_default();
        let mac_forbidden_directories: [&str; 2] = [
            &format!("{}/Library", home_dir),
            &format!("{}/Applications", home_dir),
        ];
        all_forbidden_directories.extend(mac_forbidden_directories.iter().map(|&s| s.to_string()));
    }
    all_forbidden_directories
}

fn build_walk_dir(path: &String, skip_path: Vec<String>) -> Walk {
    let mut builder = WalkBuilder::new(path);
    builder
        .hidden(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .follow_links(false)
        .filter_entry(move |entry| {
            if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                return true;
            }
            let curr_path = crate::infrastructure::utils::norm(entry.path().to_str().unwrap_or(""));
            !skip_path.iter().any(|x| curr_path.contains(x))
        });
    builder.build()
}

/// Full directory walk + metadata upsert, mirroring `walk_directory`.
/// Stops early when `running` is turned off (popup cancel).
#[allow(clippy::too_many_arguments)]
pub fn walk_and_index(
    conn: &mut diesel::SqliteConnection,
    file_paths: Vec<String>,
    ignore_cache: &IgnoreAllowCacheState,
    preferences: &UserPreferencesState,
    tx: &Sender<UiEvent>,
    running: &Arc<AtomicBool>,
) -> usize {
    let all_forbidden_directories = forbidden_directories();
    let mut files_added = 0;
    let allowed_filetypes = indexing::all_allowed_filetypes(conn, true);
    let allowed_extensions: Vec<String> = allowed_filetypes
        .iter()
        .map(|filetype| filetype.file_type.to_string())
        .collect();

    for path in file_paths {
        log::info!("Indexing file path: {}", path);
        let walk_dir = build_walk_dir(&path, all_forbidden_directories.clone());
        let mut files_array: Vec<crate::infrastructure::database::models::DocumentItem> = vec![];

        for entry in walk_dir {
            if !running.load(Ordering::SeqCst) {
                log::info!("Scan cancelled during directory walk; stopping");
                return files_added;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    let is_not_found = e
                        .io_error()
                        .map(|ioe| ioe.kind() == std::io::ErrorKind::NotFound)
                        .unwrap_or(false);
                    if !is_not_found {
                        log::error!("Error while walking directory: {}", e);
                    }
                    continue;
                }
            };
            let entry_path = entry.path();

            let file_item = create_document_item(entry_path, &allowed_extensions);
            let file_item = match file_item {
                Ok(file_item) => file_item,
                Err(reason) => {
                    if matches!(
                        reason,
                        indexing::ScanSkipReason::PermissionDenied
                            | indexing::ScanSkipReason::MetadataUnavailable
                    ) {
                        log::info!("Skipping {} during scan: {:?}", entry_path.to_string_lossy(), reason);
                    }
                    continue;
                }
            };

            if ignore_cache.should_skip(&file_item.path) {
                continue;
            }
            if preferences.manual_setup && !ignore_cache.is_allowed(&file_item.path) {
                continue;
            }

            files_array.push(file_item);

            if files_array.len() == 500 {
                add_file_metadata_to_database(&files_array, conn);
                files_added += files_array.len();
                let _ = emit(tx, WorkerEvent::FilesAdded { count: files_added, complete: false });
                files_array.clear();
            }
        }
        if !files_array.is_empty() {
            add_file_metadata_to_database(&files_array, conn);
            files_added += files_array.len();
            let _ = emit(tx, WorkerEvent::FilesAdded { count: files_added, complete: true });
            files_array.clear();
        }
    }

    files_added
}

/// Parse the text content of all eligible files and commit to the Tantivy index
/// and the `body` table, mirroring `parse_content_from_files`.
#[allow(clippy::too_many_arguments)]
pub async fn parse_files(
    pool: &DbPool,
    preferences: &Arc<RwLock<UserPreferencesState>>,
    writer: &Arc<Mutex<tantivy::IndexWriter>>,
    ignore_cache: &IgnoreAllowCacheState,
    tx: &Sender<UiEvent>,
    sync_running_getter: impl Fn() -> bool,
) -> usize {
    let mut files_parsed = 0;
    let document_filetypes = ["docx", "md", "pptx", "txt", "epub"];
    let image_filetypes = ["png", "jpeg", "jpg", "bmp", "tif", "tiff"];
    let image_cutoff_size: f64 = 50_000.0;

    let prefs = preferences.read().unwrap_or_else(|e| e.into_inner()).clone();
    let mut conn = establish_connection(pool);

    type DocRow = (i32, i32, String, String, String, String, i64, i64, Option<String>, Option<f64>);

    let load_rows = |conn: &mut SqliteConnection,
                     filetypes: &[&str],
                     image_filter: bool,
                     size_cutoff: f64|
     -> Vec<DocRow> {
        let mut query = document::table
            .inner_join(metadata::table.on(document::id.eq(metadata::source_id)))
            .filter(document::file_type.eq_any(filetypes))
            .into_boxed();
        if image_filter {
            query = query.filter(document::size.gt(size_cutoff));
        }
        match query
            .select((
                metadata::id,
                document::id,
                document::source_domain,
                document::name,
                document::path,
                document::file_type,
                document::last_modified,
                document::last_parsed,
                document::comment,
                document::size,
            ))
            .order_by(document::size.asc())
            .load::<DocRow>(conn)
        {
            Ok(rows) => rows,
            Err(e) => {
                log::error!("Could not load files to parse: {}", e);
                Vec::new()
            }
        }
    };

    let not_pdf_files_data = load_rows(&mut conn, &document_filetypes, false, 0.0);

    let mut all_files_data = not_pdf_files_data.clone();

    if prefs.parse_pdfs {
        let pdf_files_data = load_rows(&mut conn, &["pdf"], false, 0.0);
        let image_files_data = load_rows(&mut conn, &image_filetypes, true, image_cutoff_size);

        all_files_data = all_files_data
            .into_iter()
            .chain(pdf_files_data)
            .chain(image_files_data)
            .collect();
    }

    let filtered: Vec<(i32, i32, String, String, String, String, i64, i64, Option<String>, Option<f64>)> =
        all_files_data
            .into_iter()
            .filter(|item| {
                let path = &item.4;
                if ignore_cache.should_skip(path) {
                    return false;
                }
                if item.7 != 0 && item.6 < item.7 {
                    return false;
                }
                true
            })
            .collect();

    let total_to_parse = filtered.len();
    let _ = emit(tx, WorkerEvent::ScanStarted { total: total_to_parse });

    let mut body_items: Vec<BodyItem> = vec![];
    let mut body_tantivy_items: Vec<crate::domain::types::TantivyDocumentItem> = vec![];
    let mut body_tantivy_source_ids: Vec<i32> = vec![];
    let mut body_file_chunk_cutoff = 500;
    let mut average_body_file_size = 0.0;
    let mut body_file_size_sum = 0.0;
    let mut body_file_size_count = 0f64;
    let mut files_success = 0usize;
    let mut failed_files: Vec<crate::domain::types::OcrFailedFile> = vec![];

    for file_item in filtered {
        if !sync_running_getter() {
            break;
        }

        let metadata_id = file_item.0;
        let source_id = file_item.1;
        let source_domain = file_item.2;
        let name = file_item.3;
        let path = file_item.4;
        let file_type = file_item.5;
        let last_modified = file_item.6;
        let last_parsed = file_item.7;
        let comment = file_item.8;
        let file_size = file_item.9;

        if last_parsed == 0 || last_modified > last_parsed {
            let (text, error) = indexing::extract_text_from_path_with_error(
                path.clone(),
                file_type.clone(),
                pool,
                preferences,
            )
            .await;
            let is_success = error.is_none() && !text.trim().is_empty();
            if is_success {
                files_success += 1;
            } else {
                failed_files.push(crate::domain::types::OcrFailedFile {
                    path: path.clone(),
                    name: name.clone(),
                    error: error.unwrap_or_else(|| "No text was extracted".to_string()),
                });
            }
            let chunks = chunk_text(text);

            for chunk in chunks {
                body_tantivy_items.push(crate::domain::types::TantivyDocumentItem {
                    source_id: i64::from(source_id),
                    source_table: "document".to_string(),
                    source_domain: source_domain.clone(),
                    name: name.clone(),
                    url: path.clone(),
                    body: chunk.clone(),
                    file_type: file_type.clone(),
                    last_modified: i64::from(last_modified),
                    comment: comment.clone().unwrap_or_else(|| "".to_string()),
                });
                body_items.push(BodyItem {
                    metadata_id,
                    source_id,
                    text: chunk,
                    last_parsed: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
                });
            }

            body_tantivy_source_ids.push(source_id);
            body_file_size_sum += file_size.unwrap_or(0.0);
            body_file_size_count += 1.0;
            average_body_file_size = body_file_size_sum / body_file_size_count;
            files_parsed += 1;

            if files_parsed % 25 == 0 {
                let _ = emit(
                    tx,
                    WorkerEvent::ScanProgress {
                        processed: files_parsed,
                        total: total_to_parse,
                        success: files_success,
                        failed: failed_files.len(),
                        file: path.clone(),
                        failed_files: failed_files.clone(),
                    },
                );
                log::info!("{} files parsed", files_parsed);
            }

            if files_parsed % 10 == 0 {
                if average_body_file_size >= 500_000.0 {
                    body_file_chunk_cutoff = 10;
                } else if average_body_file_size >= 250_000.0 {
                    body_file_chunk_cutoff = 50;
                } else {
                    body_file_chunk_cutoff = 500;
                }
            }

            if body_tantivy_items.len() >= body_file_chunk_cutoff {
                commit_index_batch(
                    writer,
                    &mut conn,
                    &mut body_tantivy_items,
                    &mut body_tantivy_source_ids,
                    &mut body_items,
                );
                body_file_size_sum = 0.0;
                body_file_size_count = 0.0;
                average_body_file_size = 0.0;
            }
        }

        if !sync_running_getter() {
            break;
        }
    }

    if !body_tantivy_items.is_empty() {
        commit_index_batch(
            writer,
            &mut conn,
            &mut body_tantivy_items,
            &mut body_tantivy_source_ids,
            &mut body_items,
        );
    }

    files_parsed
}

fn commit_index_batch(
    writer: &Arc<Mutex<tantivy::IndexWriter>>,
    conn: &mut diesel::SqliteConnection,
    body_tantivy_items: &mut Vec<crate::domain::types::TantivyDocumentItem>,
    body_tantivy_source_ids: &mut Vec<i32>,
    body_items: &mut Vec<BodyItem>,
) {
    let indexing_commit_response =
        tantivy_index::delete_and_add_docs_to_index(writer, body_tantivy_source_ids, body_tantivy_items);
    if indexing_commit_response.is_err() {
        log::error!(
            "Error updating Tantivy Index: {:?}",
            indexing_commit_response
        );
    }
    indexing::add_body_to_database_public(body_items, conn);
    indexing::update_last_parsed_in_document_table(conn, body_tantivy_source_ids.clone());
    body_tantivy_items.clear();
    body_tantivy_source_ids.clear();
    body_items.clear();
}

/// OCR-only rescan of PDF and image files, mirroring `rescan_ocr_documents` /
/// `rescan_ocr_files`. Re-processes already-parsed files with bounded concurrency
/// and streams progress to the UI. Returns `true` when completed, `false` when
/// cancelled.
#[allow(clippy::too_many_arguments)]
pub async fn rescan_ocr(
    pool: &DbPool,
    preferences: &Arc<RwLock<UserPreferencesState>>,
    writer: &Arc<Mutex<tantivy::IndexWriter>>,
    ignore_cache: &IgnoreAllowCacheState,
    tx: &Sender<UiEvent>,
    ocr: &Arc<OcrController>,
    sort_order: String,
    threads: i64,
    paths: Option<Vec<String>>,
) -> bool {
    const IMAGE_FILETYPES: [&str; 6] = ["png", "jpeg", "jpg", "bmp", "tif", "tiff"];
    const IMAGE_CUTOFF_SIZE: f64 = 50_000.0;

    let mut conn = establish_connection(pool);

    let mut query = document::table
        .inner_join(metadata::table.on(document::id.eq(metadata::source_id)))
        .filter(
            document::file_type.eq("pdf").or(document::file_type
                .eq_any(IMAGE_FILETYPES)
                .and(document::size.gt(IMAGE_CUTOFF_SIZE))),
        )
        .select((
            metadata::id,
            document::id,
            document::source_domain,
            document::name,
            document::path,
            document::file_type,
            document::last_modified,
            document::last_parsed,
            document::comment,
            document::size,
        ))
        .into_boxed();

    if let Some(paths) = paths {
        query = query.filter(document::path.eq_any(paths));
    }

    query = match sort_order.as_str() {
        "size_desc" => query.order_by(document::size.desc()),
        "name_asc" => query.order_by(document::name.asc()),
        "name_desc" => query.order_by(document::name.desc()),
        "modified_desc" => query.order_by(document::last_modified.desc()),
        "modified_asc" => query.order_by(document::last_modified.asc()),
        "opened_desc" => query.order_by(document::last_opened.desc()),
        "opened_asc" => query.order_by(document::last_opened.asc()),
        _ => query.order_by(document::size.asc()),
    };

    let all_files_data = query.load(&mut conn).unwrap_or_default();
    run_ocr_rescan(all_files_data, pool, preferences, writer, ignore_cache, tx, ocr, threads).await
}

#[allow(clippy::too_many_arguments)]
async fn run_ocr_rescan(
    all_files_data: Vec<(i32, i32, String, String, String, String, i64, i64, Option<String>, Option<f64>)>,
    pool: &DbPool,
    preferences: &Arc<RwLock<UserPreferencesState>>,
    writer: &Arc<Mutex<tantivy::IndexWriter>>,
    _ignore_cache: &IgnoreAllowCacheState,
    tx: &Sender<UiEvent>,
    ocr: &Arc<OcrController>,
    threads: i64,
) -> bool {
    let total_to_parse = all_files_data.len();
    let concurrency = threads.clamp(1, 4) as usize;

    let _ = emit(tx, WorkerEvent::OcrRescanProgress(OcrRescanProgress {
        message: "started".to_string(),
        total: total_to_parse,
        processed: 0,
        success: 0,
        failed: 0,
        remaining: total_to_parse,
        threads,
        current_file: String::new(),
        failed_files: vec![],
        success_files: vec![],
    }));
    let _ = emit(tx, WorkerEvent::ScanStarted { total: total_to_parse });

    let mut body_items: Vec<BodyItem> = vec![];
    let mut body_tantivy_items: Vec<crate::domain::types::TantivyDocumentItem> = vec![];
    let mut body_tantivy_source_ids: Vec<i32> = vec![];
    const BATCH_CUTOFF: usize = 500;
    let mut files_parsed = 0usize;
    let mut files_success = 0usize;
    let mut failed_files: Vec<crate::domain::types::OcrFailedFile> = vec![];
    let mut success_files: Vec<crate::domain::types::OcrSuccessFile> = vec![];
    let mut completed = true;

    let stream = futures::stream::iter(all_files_data.into_iter().map(|item| {
        let pool = pool.clone();
        let preferences = preferences.clone();
        async move {
            let cancelled = ocr.cancelled.load(Ordering::SeqCst);
            let (text, error) = if cancelled {
                (String::new(), Some("Rescan cancelled".to_string()))
            } else {
                extract_text_from_path_with_error(item.4.clone(), item.5.clone(), &pool, &preferences).await
            };
            (item, text, error)
        }
    }))
    .buffer_unordered(concurrency);
    pin_mut!(stream);

    let mut conn = establish_connection(pool);

    while let Some((file_item, text, error)) = stream.next().await {
        if ocr.cancelled.load(Ordering::SeqCst) || !ocr.running.load(Ordering::SeqCst) {
            completed = false;
            break;
        }

        let metadata_id = file_item.0;
        let source_id = file_item.1;
        let source_domain = file_item.2;
        let name = file_item.3.clone();
        let path = file_item.4.clone();
        let file_type = file_item.5.clone();
        let last_modified = file_item.6;
        let comment = file_item.8;

        let is_success = error.is_none() && !text.trim().is_empty();
        if is_success {
            files_success += 1;
            success_files.push(crate::domain::types::OcrSuccessFile {
                path: path.clone(),
                name: name.clone(),
            });
        } else {
            let why = error.unwrap_or_else(|| "No text was extracted".to_string());
            failed_files.push(crate::domain::types::OcrFailedFile {
                path: path.clone(),
                name: name.clone(),
                error: why,
            });
        }

        {
            if let Ok(mut stored) = ocr.failed_files.lock() {
                *stored = failed_files.clone();
            }
            if let Ok(mut stored_success) = ocr.success_files.lock() {
                *stored_success = success_files.clone();
            }
        }

        if is_success {
            for chunk in chunk_text(text) {
                body_tantivy_items.push(crate::domain::types::TantivyDocumentItem {
                    source_id: i64::from(source_id),
                    source_table: "document".to_string(),
                    source_domain: source_domain.clone(),
                    name: name.clone(),
                    url: path.clone(),
                    body: chunk.clone(),
                    file_type: file_type.clone(),
                    last_modified: i64::from(last_modified),
                    comment: comment.clone().unwrap_or_default(),
                });
                body_items.push(BodyItem {
                    metadata_id,
                    source_id,
                    text: chunk,
                    last_parsed: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64,
                });
            }
            body_tantivy_source_ids.push(source_id);
        }

        files_parsed += 1;

        let current_file = path.clone();

        // Throttle the progress stream so a large rescan does not flood the UI
        // channel with one event per file.
        if files_parsed % 25 == 0 {
            let _ = emit(tx, WorkerEvent::OcrRescanProgress(OcrRescanProgress {
                message: "progress".to_string(),
                total: total_to_parse,
                processed: files_parsed,
                success: files_success,
                failed: failed_files.len(),
                remaining: total_to_parse.saturating_sub(files_parsed),
                threads,
                current_file: path,
                failed_files: failed_files.clone(),
                success_files: success_files.clone(),
            }));
        }

        if files_parsed % 50 == 0 {
            let _ = emit(
                tx,
                WorkerEvent::ScanProgress {
                    processed: files_parsed,
                    total: total_to_parse,
                    success: files_success,
                    failed: failed_files.len(),
                    file: current_file,
                    failed_files: failed_files.clone(),
                },
            );
            log::info!("OCR rescan: {} files parsed", files_parsed);
        }

        if body_tantivy_items.len() >= BATCH_CUTOFF {
            commit_index_batch(writer, &mut conn, &mut body_tantivy_items, &mut body_tantivy_source_ids, &mut body_items);
        }
    }

    if !body_tantivy_items.is_empty() {
        commit_index_batch(writer, &mut conn, &mut body_tantivy_items, &mut body_tantivy_source_ids, &mut body_items);
    }

    {
        if let Ok(mut stored) = ocr.failed_files.lock() {
            *stored = failed_files.clone();
        }
        if let Ok(mut stored_success) = ocr.success_files.lock() {
            *stored_success = success_files.clone();
        }
    }

    let _ = emit(tx, WorkerEvent::OcrRescanProgress(OcrRescanProgress {
        message: if completed { "finished".to_string() } else { "cancelled".to_string() },
        total: total_to_parse,
        processed: files_parsed,
        success: files_success,
        failed: failed_files.len(),
        remaining: total_to_parse.saturating_sub(files_parsed),
        threads,
        current_file: String::new(),
        failed_files: failed_files.clone(),
        success_files: success_files.clone(),
    }));

    completed
}