use crate::domain::types::DateLimit;

/// Requests the UI sends to the worker thread. Each blocking job is handled
/// serially by a single worker; search/suggestion requests carry a `generation`
/// that lets the UI discard stale replies.
#[derive(Debug, Clone)]
pub enum WorkerRequest {
    SearchSuggestions {
        query: String,
        generation: u64,
    },
    Search {
        query: String,
        page: i32,
        limit: i32,
        file_type: Option<String>,
        date_limit: Option<DateLimit>,
        generation: u64,
    },
    RecentDocs {
        page: i32,
        limit: i32,
        file_type: Option<String>,
    },
    DbStats,
    CountParsed,
    TextForFile {
        document_id: i32,
    },
    ExtractPdf {
        file_path: String,
    },
    WriteTextToFile {
        file_path: String,
        text: String,
    },
    ReadTextFromFile {
        file_path: String,
    },
    ImageBase64 {
        file_path: String,
    },
    FetchStatistics,
    FetchDashboard,
    IgnorePath {
        path: String,
        is_folder: bool,
        ignore_indexing: bool,
    },
    ShowIgnoredPaths,
    RemoveFromIgnoreList {
        paths: Vec<String>,
    },
    TantivyFilesSearch {
        query: String,
        limit: i32,
        page: i32,
    },
    TantivyBookmarksSearch {
        query: String,
        limit: i32,
        page: i32,
    },
    CsvDump,
    BrowserProfilesChrome,
    BrowserProfilesArc,
    BrowserHistorySearch {
        profile: String,
        query: String,
        limit: i32,
        page: i32,
    },
    RescanDocuments {
        rescan_all: bool,
    },
    ClearIndex,
}