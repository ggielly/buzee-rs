use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TantivyDocumentSearchResult {
    pub id: i64,
    pub last_modified: i64,
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DateLimit {
    pub start: String,
    pub end: String,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuerySegments {
    #[serde(rename = "quotedSegments")]
    pub quoted_segments: Vec<String>,
    #[serde(rename = "greedySegments")]
    pub greedy_segments: Vec<String>,
    #[serde(rename = "notSegments")]
    pub not_segments: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DBStat {
    pub file_type: String,
    pub count: i64,
}

use tantivy::IndexReader;
use tantivy::IndexWriter;

pub struct TantivyReaderState {
    pub reader: IndexReader,
}

impl TantivyReaderState {
    pub fn new(given_reader: IndexReader) -> Self {
        Self {
            reader: given_reader,
        }
    }
}

pub struct TantivyWriterState {
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

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub struct SyncRunningState {
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DashboardBuckets {
    pub file_type: String,
    pub count: i64,
    pub size_bytes: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DashboardStats {
    pub total_files: i64,
    pub total_folders: i64,
    pub total_size_bytes: f64,
    pub average_size_bytes: f64,
    pub largest_file_size_bytes: f64,
    pub parsed_files: i64,
    pub parsed_total_size_bytes: f64,
    pub unparsed_files: i64,
    pub pinned_files: i64,
    pub most_frequent_count: i64,
    pub database_size_bytes: u64,
    pub last_scan_time: i64,
    pub next_scan_in_seconds: i64,
    pub auto_sync_enabled: bool,
    pub scan_running: bool,
    pub filetype_counts: Vec<DashboardBuckets>,
    pub top_largest: Vec<crate::infrastructure::database::models::DocumentSearchResult>,
    pub top_recent: Vec<crate::infrastructure::database::models::DocumentSearchResult>,
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserPreferencesState {
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

#[derive(Clone, Serialize, Debug)]
pub struct OcrFailedFile {
    pub path: String,
    pub name: String,
    pub error: String,
}

#[derive(Clone, Serialize, Debug)]
pub struct OcrSuccessFile {
    pub path: String,
    pub name: String,
}

/// A single file processed during an indexing/OCR scan, as shown in the
/// scan popup. `error` is `None` when the file was parsed successfully.
#[derive(Clone, Serialize, Debug)]
pub struct ScanItem {
    pub path: String,
    pub name: String,
    pub error: Option<String>,
}

#[derive(Clone, Serialize, Debug)]
pub struct OcrRescanProgress {
    pub message: String,
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

pub struct OcrRescanState {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoryResult {
    pub data: Vec<(i64, String, String, String)>,
    pub is_loading: bool,
    pub error_view: Option<String>,
}