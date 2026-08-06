use crate::domain::events::WorkerEvent;
use crate::domain::types::{
    AppStatistics, DBStat, DashboardStats, TantivyBookmarkSearchResult,
    TantivyDocumentSearchResult, UserPreferencesState,
};
use crate::infrastructure::database::models::{DocumentSearchResult, IgnoreList};

/// High-level events consumed by the UI. Wraps the low-level sync/scan/OCR
/// progress events plus the typed replies to worker requests.
#[derive(Debug, Clone)]
pub enum UiEvent {
    Status(WorkerEvent),
    Error(String),

    Preferences(UserPreferencesState),
    SuggestionsFinished {
        generation: u64,
        suggestions: Vec<String>,
    },
    SearchFinished {
        generation: u64,
        results: Vec<DocumentSearchResult>,
    },
    RecentDocs {
        results: Vec<DocumentSearchResult>,
    },
    DbStats {
        stats: Vec<DBStat>,
    },
    CountParsed {
        count: i64,
    },
    TextForFile {
        text: Vec<String>,
    },
    PdfTextExtracted {
        text: Vec<String>,
    },
    TextWritten,
    TextRead {
        text: String,
    },
    ImageBase64 {
        data: String,
    },
    Statistics {
        stats: AppStatistics,
    },
    Dashboard {
        stats: DashboardStats,
    },
    PathIgnored,
    IgnoredPaths {
        paths: Vec<IgnoreList>,
    },
    PathsRemovedFromIgnoreList,
    TantivyFiles {
        results: Vec<TantivyDocumentSearchResult>,
    },
    TantivyBookmarks {
        results: Vec<TantivyBookmarkSearchResult>,
    },
    CsvDumped,
    ChromeProfiles {
        profiles: Vec<String>,
    },
    ArcProfiles {
        profiles: Vec<String>,
    },
    BrowserHistory {
        results: Vec<DocumentSearchResult>,
    },
    IndexCleared,
    SyncFinished,
    GlobalShortcutPressed,
}