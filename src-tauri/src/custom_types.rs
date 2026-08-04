use serde::{Deserialize, Serialize};

// create the error type that represents all errors possible in our program
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn new(message: &str) -> Self {
        Self::Io(std::io::Error::new(std::io::ErrorKind::Other, message))
    }
}

impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Self {
        Self::new(&e.to_string())
    }
}

// we must manually implement serde::Serialize
impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

// This struct is for INSERTING documents into the Tantivy Index
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TantivyDocumentItem {
    pub source_id: i64,
    pub source_table: String,
    pub source_domain: String,
    pub name: String,
    pub url: String,
    pub body: String,
    pub file_type: String,
    pub last_modified: i64,
    pub comment: String,
}

// Struct for TantivyDocumentSearchResult
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TantivyDocumentSearchResult {
    pub id: i64,
    pub last_modified: i64,
}

// Struct for TantivyBrowserHistorySearchResult
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub struct TantivyBrowserHistorySearchResult {
    pub id: i64,
    pub source_table: String,
    pub source_domain: String,
    pub is_pinned: Option<bool>,
    pub comment: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub url: Option<String>,
    pub last_visited: Option<i64>,
    pub frecency_rank: Option<f64>,
    pub frecency_last_accessed: Option<i64>,
}

// Struct for TantivyBookmarkSearchResult
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TantivyBookmarkSearchResult {
    pub id: i64,
    pub source_table: String,
    pub source_domain: String,
    pub is_pinned: Option<bool>,
    pub comment: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub url: Option<String>,
    pub saved_at: Option<i64>,
    pub last_opened: Option<i64>,
    pub word_count: Option<i64>,
    pub is_favorite: Option<bool>,
    pub is_archived: Option<bool>,
    pub is_read: Option<bool>,
    pub tags: Option<String>,
    pub frecency_rank: Option<f64>,
    pub frecency_last_accessed: Option<i64>,
}

// Struct for TantivyEmailSearchResult
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub struct TantivyEmailSearchResult {
    pub id: i64,
    pub source_table: String,
    pub source_domain: String,
    pub is_pinned: Option<bool>,
    pub comment: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub url: Option<String>,
    pub sender: Option<String>,
    pub recipient: Option<String>,
    pub cc: Option<String>,
    pub bcc: Option<String>,
    pub attachments: Option<String>,
    pub tags: Option<String>,
    pub frecency_rank: Option<f64>,
    pub frecency_last_accessed: Option<i64>,
}

// DateLimit struct
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DateLimit {
    pub start: String,
    pub end: String,
    pub text: String,
}

// Query Segments struct
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuerySegments {
    #[serde(rename = "quotedSegments")]
    pub quoted_segments: Vec<String>,
    #[serde(rename = "greedySegments")]
    pub greedy_segments: Vec<String>,
    #[serde(rename = "notSegments")]
    pub not_segments: Vec<String>,
}

// Payload for IPC events
#[derive(Clone, Serialize)]
pub struct Payload {
    pub message: String,
    pub data: String,
}

// Define a struct for passing DB Stats
#[derive(Debug, Serialize, Deserialize)]
pub struct DBStat {
    pub file_type: String,
    pub count: i64,
}

// // Struct for AppHandle
// pub(crate) struct AppHandleState {
//   pub stored_app_handle: tauri::AppHandle
// }

// impl AppHandleState {
//   // using new because Default doesn't let you pass arguments
//   pub fn new(app_handle: tauri::AppHandle) -> Self {
//     Self {
//       stored_app_handle: app_handle
//     }
//   }
// }

// Struct for Database Connection
// pub(crate) struct DBConnectionState {
//   pub stored_db_conn: PooledConnection<ConnectionManager<SqliteConnection>>
// }

// impl DBConnectionState {
//   // using new because Default doesn't let you pass arguments
//   pub fn new(mut conn: PooledConnection<ConnectionManager<SqliteConnection>>) -> Self {
//     Self {
//       stored_db_conn: conn
//     }
//   }
// }

use tantivy::IndexReader;
// Struct for Tantivy Reader
pub(crate) struct TantivyReaderState {
    pub reader: IndexReader,
}

impl TantivyReaderState {
    pub fn new(given_reader: IndexReader) -> Self {
        Self {
            reader: given_reader,
        }
    }
}

use tantivy::IndexWriter;
// Struct for Tantivy Writer (singleton — shared across all write operations)
pub(crate) struct TantivyWriterState {
    pub writer: IndexWriter,
}

impl TantivyWriterState {
    pub fn new(given_writer: IndexWriter) -> Self {
        Self {
            writer: given_writer,
        }
    }
}

use diesel::r2d2::{ConnectionManager, Pool};
use diesel::SqliteConnection;

// Struct for Database Connection Pool
pub(crate) struct DBConnPoolState {
    pub conn_pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl DBConnPoolState {
    pub fn new(pool: Pool<ConnectionManager<SqliteConnection>>) -> Self {
        Self { conn_pool: pool }
    }
}

// Struct for Sync Running State
pub(crate) struct SyncRunningState {
    pub sync_running: bool,
    pub last_sync_time: i64,
}

impl Default for SyncRunningState {
    fn default() -> Self {
        Self {
            sync_running: false,
            last_sync_time: 0,
        }
    }
}

use std::collections::{HashMap, HashSet};

/// A prefix trie over raw bytes. Folder-prefix matching (`path.starts_with(p)`)
/// is O(path length) instead of a linear scan over every folder in the list.
#[derive(Default)]
pub(crate) struct PrefixSet {
    children: HashMap<u8, Box<PrefixSet>>,
    is_terminal: bool,
}

impl PrefixSet {
    pub fn insert(&mut self, s: &str) {
        let mut node = self;
        for b in s.as_bytes() {
            node = node.children.entry(*b).or_default();
        }
        node.is_terminal = true;
    }

    /// Returns true if any inserted string is a prefix of `path`.
    pub fn contains_prefix_of(&self, path: &str) -> bool {
        let mut node = self;
        for b in path.as_bytes() {
            if node.is_terminal {
                return true;
            }
            match node.children.get(b) {
                Some(next) => node = next,
                None => return false,
            }
        }
        node.is_terminal
    }
}

/// Cached allow/ignore lists to avoid re-querying the DB on every file during scan.
#[derive(Default)]
pub(crate) struct IgnoreAllowCacheState {
    pub ignored_file_paths: HashSet<String>,
    pub ignored_folder_prefixes: PrefixSet,
    pub ignored_indexonly_file_paths: HashSet<String>,
    pub ignored_indexonly_folder_prefixes: PrefixSet,
    pub allowed_file_paths: HashSet<String>,
    pub allowed_folder_prefixes: PrefixSet,
}

impl IgnoreAllowCacheState {
    pub fn from_db(conn: &mut diesel::SqliteConnection) -> Self {
        use crate::indexing::{get_all_allowed_paths, get_all_ignored_paths};

        let ignored_items = get_all_ignored_paths(conn);
        let allowed_items = get_all_allowed_paths(conn);

        let mut ignored_file_paths = HashSet::new();
        let mut ignored_folder_prefixes = PrefixSet::default();
        let mut ignored_indexonly_file_paths = HashSet::new();
        let mut ignored_indexonly_folder_prefixes = PrefixSet::default();
        let mut allowed_file_paths = HashSet::new();
        let mut allowed_folder_prefixes = PrefixSet::default();

        for item in &ignored_items {
            if item.is_folder {
                if item.ignore_indexing {
                    ignored_folder_prefixes.insert(&item.path);
                } else {
                    ignored_indexonly_folder_prefixes.insert(&item.path);
                }
            } else {
                if item.ignore_indexing {
                    ignored_file_paths.insert(item.path.clone());
                } else {
                    ignored_indexonly_file_paths.insert(item.path.clone());
                }
            }
        }

        for item in &allowed_items {
            if item.is_folder {
                allowed_folder_prefixes.insert(&item.path);
            } else {
                allowed_file_paths.insert(item.path.clone());
            }
        }

        Self {
            ignored_file_paths,
            ignored_folder_prefixes,
            ignored_indexonly_file_paths,
            ignored_indexonly_folder_prefixes,
            allowed_file_paths,
            allowed_folder_prefixes,
        }
    }

    /// Returns true if the path is explicitly allowed (exact file or under an allowed folder).
    pub fn is_allowed(&self, path: &str) -> bool {
        self.allowed_file_paths.contains(path)
            || self.allowed_folder_prefixes.contains_prefix_of(path)
    }

    /// Returns true if the path is ignored with `ignore_indexing` (fully excluded).
    pub fn is_ignored(&self, path: &str) -> bool {
        self.ignored_file_paths.contains(path)
            || self.ignored_folder_prefixes.contains_prefix_of(path)
    }

    /// Returns true if the path is ignored with `ignore_indexing == false` (index-only removal).
    pub fn is_ignored_index_only(&self, path: &str) -> bool {
        self.ignored_indexonly_file_paths.contains(path)
            || self
                .ignored_indexonly_folder_prefixes
                .contains_prefix_of(path)
    }

    /// Returns true if the path should be skipped (ignored, not overridden by allow list).
    pub fn should_skip(&self, path: &str) -> bool {
        // If explicitly allowed, never skip
        if self.is_allowed(path) {
            return false;
        }
        // If in ignore list with ignore_indexing=true, skip
        self.is_ignored(path)
    }
}

// Statistics shown in the status bar.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppStatistics {
    pub status: String,
    pub total_files: i64,
    pub parsed_files: i64,
    pub database_size_bytes: u64,
    pub last_scan_time: i64,
    pub next_scan_in_seconds: i64,
    pub auto_sync_enabled: bool,
}

impl Default for AppStatistics {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            total_files: 0,
            parsed_files: 0,
            database_size_bytes: 0,
            last_scan_time: 0,
            next_scan_in_seconds: -1,
            auto_sync_enabled: false,
        }
    }
}

// A single entry in the file-type / category breakdown for the dashboard.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DashboardBuckets {
    pub file_type: String,
    pub count: i64,
    pub size_bytes: f64,
}

// Comprehensive statistics shown on the Dashboard home page.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DashboardStats {
    // Totals
    pub total_files: i64,
    pub total_folders: i64,
    pub total_size_bytes: f64,
    pub average_size_bytes: f64,
    pub largest_file_size_bytes: f64,
    // Parsing
    pub parsed_files: i64,
    pub parsed_total_size_bytes: f64,
    pub unparsed_files: i64,
    // Pinned
    pub pinned_files: i64,
    // Frecency
    pub most_frequent_count: i64,
    // Database + scans
    pub database_size_bytes: u64,
    pub last_scan_time: i64,
    pub next_scan_in_seconds: i64,
    pub auto_sync_enabled: bool,
    pub scan_running: bool,
    // Breakdowns
    pub filetype_counts: Vec<DashboardBuckets>,
    // Recent / largest documents (limited)
    pub top_largest: Vec<crate::database::models::DocumentSearchResult>,
    pub top_recent: Vec<crate::database::models::DocumentSearchResult>,
}

impl Default for DashboardStats {
    fn default() -> Self {
        Self {
            total_files: 0,
            total_folders: 0,
            total_size_bytes: 0.0,
            average_size_bytes: 0.0,
            largest_file_size_bytes: 0.0,
            parsed_files: 0,
            parsed_total_size_bytes: 0.0,
            unparsed_files: 0,
            pinned_files: 0,
            most_frequent_count: 0,
            database_size_bytes: 0,
            last_scan_time: 0,
            next_scan_in_seconds: -1,
            auto_sync_enabled: false,
            scan_running: false,
            filetype_counts: Vec::new(),
            top_largest: Vec::new(),
            top_recent: Vec::new(),
        }
    }
}

// Struct for Global Shortcut String
#[derive(Serialize, Clone)]
#[allow(dead_code)]
pub(crate) struct GlobalShortcutState {
    pub shortcut_string: String,
    pub shortcut_enabled: bool,
}

impl Default for GlobalShortcutState {
    fn default() -> Self {
        Self {
            shortcut_string: "Alt+Space".to_string(),
            shortcut_enabled: false,
        }
    }
}

// Struct for User Preference
#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct UserPreferencesState {
    pub first_launch_done: bool,
    pub onboarding_done: bool,
    pub show_search_suggestions: bool,
    pub launch_at_startup: bool,
    pub show_in_dock: bool,
    pub global_shortcut_enabled: bool,
    pub global_shortcut: String,
    pub automatic_background_sync: bool,
    pub detailed_scan: bool,
    pub roadmap_survey_answered: bool,
    pub parse_pdfs: bool,
    pub manual_setup: bool,
    pub enable_logs: bool,
    pub pdf_max_ocr_pages: i64,
    pub ocr_threads: i64,
    pub ocr_sort_order: String,
}

impl Default for UserPreferencesState {
    fn default() -> Self {
        Self {
            first_launch_done: false,
            onboarding_done: false,
            show_search_suggestions: true,
            launch_at_startup: true,
            show_in_dock: true,
            global_shortcut_enabled: true,
            global_shortcut: "Alt+Space".to_string(),
            automatic_background_sync: true,
            detailed_scan: true,
            roadmap_survey_answered: false,
            parse_pdfs: true,
            manual_setup: false,
            enable_logs: false,
            pdf_max_ocr_pages: 150,
            ocr_threads: 1,
            ocr_sort_order: "size_asc".to_string(),
        }
    }
}

// A single file whose OCR extraction failed during a rescan.
#[derive(Clone, Serialize, Debug)]
pub(crate) struct OcrFailedFile {
    pub path: String,
    pub name: String,
    pub error: String,
}

// A single file whose OCR extraction succeeded during a rescan.
#[derive(Clone, Serialize, Debug)]
pub(crate) struct OcrSuccessFile {
    pub path: String,
    pub name: String,
}

// Rich progress payload streamed to the frontend during an OCR rescan. Serialized
// to JSON and sent as the `data` string of an "ocr-rescan-progress" event.
#[derive(Clone, Serialize, Debug)]
pub(crate) struct OcrRescanProgress {
    pub message: String, // "started" | "progress" | "finished"
    pub total: usize,
    pub processed: usize,
    pub success: usize,
    pub failed: usize,
    pub remaining: usize,
    pub threads: i64,
    pub current_file: String,
    pub failed_files: Vec<OcrFailedFile>,
    pub success_files: Vec<OcrSuccessFile>,
}

// Tracks the on-demand OCR rescan (triggered from the settings page) so the
// frontend can stop it and tell whether one is already in flight. Failed files
// are kept so they can be retried after the rescan has finished.
pub(crate) struct OcrRescanState {
    pub running: std::sync::atomic::AtomicBool,
    pub cancelled: std::sync::atomic::AtomicBool,
    pub failed_files: std::sync::Mutex<Vec<OcrFailedFile>>,
    pub success_files: std::sync::Mutex<Vec<OcrSuccessFile>>,
}

impl OcrRescanState {
    pub fn new() -> Self {
        Self {
            running: std::sync::atomic::AtomicBool::new(false),
            cancelled: std::sync::atomic::AtomicBool::new(false),
            failed_files: std::sync::Mutex::new(vec![]),
            success_files: std::sync::Mutex::new(vec![]),
        }
    }
}

use tauri::menu::Menu;
use tauri::Wry;

// Struct for Context Menu
pub(crate) struct ContextMenuState {
    pub folder: Menu<Wry>,
    pub docs: Menu<Wry>,
    pub other: Menu<Wry>,
    pub table_header: Menu<Wry>,
    pub status_bar: Menu<Wry>,
}

impl ContextMenuState {
    pub fn new(
        folder_context_menu: Menu<Wry>,
        docs_context_menu: Menu<Wry>,
        other_context_menu: Menu<Wry>,
        table_header_menu: Menu<Wry>,
        status_bar_menu: Menu<Wry>,
    ) -> Self {
        Self {
            folder: folder_context_menu,
            docs: docs_context_menu,
            other: other_context_menu,
            table_header: table_header_menu,
            status_bar: status_bar_menu,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoryResult {
    pub data: Vec<(i64, String, String, String)>,
    pub is_loading: bool,
    pub error_view: Option<String>,
}
