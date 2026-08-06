use crate::application::UiEvent;
use crate::domain::types::{
    AppStatistics, DBStat, DashboardStats, TantivyBookmarkSearchResult,
    TantivyDocumentSearchResult, UserPreferencesState,
};
use crate::infrastructure::database::models::{DocumentSearchResult, IgnoreList};
use crate::ui::result_table::ResultTableColumn;
use crate::ui::theme::{Theme, ThemeChoice};
use std::sync::{Arc, Mutex};

/// Top-level screens reachable from the sidebar, mirroring the original routes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Dashboard,
    Search,
    Settings,
    Ignore,
    ExtractText,
    Tips,
}

/// How the results surface is displayed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    List,
    Grid,
}

/// A sortable result-table column.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortColumn {
    #[default]
    Name,
    Type,
    LastModified,
    LastOpened,
    Size,
    Location,
}

/// The active sort of the result table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SortState {
    pub column: SortColumn,
    pub asc: bool,
}

/// The current phase of a background scan, shown in the scan popup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScanPhase {
    #[default]
    Idle,
    /// Walking the directory tree and recording file metadata.
    Scanning,
    /// Parsing file content into the search index.
    Parsing,
    /// OCR rescanning PDFs/images.
    Ocr,
}

/// Mutable UI state held by the application.
pub struct BuzeeUiState {
    pub preferences: UserPreferencesState,
    pub search_input: String,
    pub query_gen: u64,
    pub suggestions: Vec<String>,
    /// Query waiting for the next tick to be sent as a suggestion request
    /// (debounced so typing does not flood the worker queue).
    pub pending_suggestions: Option<String>,
    pub results: Vec<DocumentSearchResult>,
    pub recent_docs: Vec<DocumentSearchResult>,
    pub db_stats: Vec<DBStat>,
    pub parsed_count: i64,
    pub statistics: Option<AppStatistics>,
    pub dashboard: Option<DashboardStats>,
    pub file_text: Vec<String>,
    pub ignored_paths: Vec<IgnoreList>,
    pub tantivy_files: Vec<TantivyDocumentSearchResult>,
    pub tantivy_bookmarks: Vec<TantivyBookmarkSearchResult>,
    pub chrome_profiles: Vec<String>,
    pub arc_profiles: Vec<String>,
    pub browser_history: Vec<DocumentSearchResult>,
    pub focus_search: bool,
    pub status: String,
    /// Search filters.
    pub location: String,
    pub file_type: Option<String>,
    /// (label, start_unix, end_unix); `None` = all time.
    pub date_range: Option<(String, i64, i64)>,
    /// Result view state.
    pub view_mode: ViewMode,
    pub compact_view: bool,
    pub sort: SortState,
    pub selected_result: Option<i32>,
    /// Last known cursor position (logical coordinates), used to place the
    /// right-click context menu.
    pub cursor: (f32, f32),
    /// Open right-click context menu position (`None` = hidden).
    pub context_menu: Option<(f32, f32)>,
    /// Data-only column descriptors for the virtualized results table (rebuilt
    /// on each sort/selection/results change so `iced_table` can borrow them).
    pub result_columns: Vec<ResultTableColumn>,
    /// Index into `results` selected via keyboard navigation.
    pub selected_index: usize,
    pub show_location_menu: bool,
    pub screen: Screen,
    /// Pending text input for the ignore-list "add" form.
    pub ignore_input: String,
    /// Pending text input for the extract-text screen.
    pub extract_input: String,
    /// Latest extraction output (non-empty = show result panel).
    pub extract_output: Option<String>,
    /// Pending numeric inputs for OCR settings.
    pub ocr_pages_input: String,
    pub ocr_threads_input: String,
    /// Pending global-shortcut text input.
    pub shortcut_input: String,
    /// Whether a worker request is in flight (shows busy states).
    pub busy: bool,
    /// Live scan state, shown as a modal popup while a scan runs.
    pub scan_running: bool,
    /// True once the user triggers a scan; drives the popup. Startup and
    /// automatic background scans never activate it.
    pub scan_popup_active: bool,
    pub scan_phase: ScanPhase,
    pub scan_files_added: usize,
    pub scan_processed: usize,
    pub scan_total: usize,
    /// Number of files parsed successfully during the current scan.
    pub scan_success: usize,
    /// Number of files that failed to parse during the current scan.
    pub scan_failed: usize,
    /// Most recently processed file, shown as "currently indexing".
    pub scan_current_file: String,
    /// Cumulative list of files processed during this scan, with optional
    /// error messages for the ones that failed.
    pub scan_items: Vec<crate::domain::types::ScanItem>,
    /// True briefly after a scan finishes, so the popup can show a "complete"
    /// state until the user dismisses it.
    pub scan_complete: bool,
    /// Tick counter used to throttle statistics refreshes during a scan.
    pub stats_tick: u32,
    pub theme: Theme,
    pub themes: Vec<ThemeChoice>,
    event_rx: Arc<Mutex<crossbeam_channel::Receiver<UiEvent>>>,
}

impl BuzeeUiState {
    pub fn new(preferences: UserPreferencesState) -> Self {
        // The event channel is created by main and handed to the app; this
        // constructor takes it from a placeholder that main replaces.
        let (tx, rx) = crossbeam_channel::unbounded::<UiEvent>();
        let _ = tx;
        Self {
            preferences,
            search_input: String::new(),
            query_gen: 0,
            suggestions: vec![],
            pending_suggestions: None,
            results: vec![],
            recent_docs: vec![],
            db_stats: vec![],
            parsed_count: 0,
            statistics: None,
            dashboard: None,
            file_text: vec![],
            ignored_paths: vec![],
            tantivy_files: vec![],
            tantivy_bookmarks: vec![],
            chrome_profiles: vec![],
            arc_profiles: vec![],
            browser_history: vec![],
            focus_search: true,
            status: "ready".to_string(),
            location: "my computer".to_string(),
            file_type: None,
            date_range: None,
            view_mode: ViewMode::List,
            compact_view: false,
            sort: SortState::default(),
            selected_result: None,
            cursor: (0.0, 0.0),
            context_menu: None,
            result_columns: vec![],
            selected_index: 0,
            show_location_menu: true,
            screen: Screen::Dashboard,
            ignore_input: String::new(),
            extract_input: String::new(),
            extract_output: None,
            ocr_pages_input: "150".to_string(),
            ocr_threads_input: "1".to_string(),
            shortcut_input: "Alt+Space".to_string(),
            busy: false,
            scan_running: false,
            scan_popup_active: false,
            scan_phase: ScanPhase::Idle,
            scan_files_added: 0,
            scan_processed: 0,
            scan_total: 0,
            scan_success: 0,
            scan_failed: 0,
            scan_current_file: String::new(),
            scan_items: vec![],
            scan_complete: false,
            stats_tick: 0,
            theme: Theme::default(),
            themes: Theme::bundled_themes(),
            event_rx: Arc::new(Mutex::new(rx)),
        }
    }

    pub fn set_event_rx(&mut self, rx: crossbeam_channel::Receiver<UiEvent>) {
        self.event_rx = Arc::new(Mutex::new(rx));
    }

    /// Drain the pending UI events into an owned vector.
    pub fn drain_events(&mut self) -> Vec<UiEvent> {
        let mut events = Vec::new();
        let rx = self.event_rx.clone();
        let mut rx = rx.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            match rx.try_recv() {
                Ok(event) => events.push(event),
                Err(_) => break,
            }
        }
        events
    }

    pub fn touch(&mut self) {
        let running = self.statistics.as_ref().map(|s| s.status.clone()).unwrap_or_default();
        self.status = running;
    }

    /// Reset the scan-popup state (after a finished scan is dismissed).
    pub fn reset_scan(&mut self) {
        self.scan_running = false;
        self.scan_popup_active = false;
        self.scan_phase = ScanPhase::Idle;
        self.scan_files_added = 0;
        self.scan_processed = 0;
        self.scan_total = 0;
        self.scan_success = 0;
        self.scan_failed = 0;
        self.scan_current_file.clear();
        self.scan_items.clear();
        self.scan_complete = false;
    }
}

/// Build the initial state using a real event receiver; used by main.
pub fn initial_state(
    preferences: UserPreferencesState,
    event_rx: crossbeam_channel::Receiver<UiEvent>,
    theme: Theme,
) -> BuzeeUiState {
    let mut state = BuzeeUiState::new(preferences);
    state.set_event_rx(event_rx);
    state.theme = theme;
    state
}