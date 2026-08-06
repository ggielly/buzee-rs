use crate::application::commands;
use crate::application::events::UiEvent;
use crate::application::requests::WorkerRequest;
use crate::application::AppServices;
use crossbeam_channel::{unbounded, Sender};
use std::sync::Arc;

/// Spawn the background worker thread. All rich replies and controller events are
/// sent on the shared `event_tx`. Returns the request sender (UI → worker).
pub fn spawn(services: Arc<AppServices>, event_tx: Sender<UiEvent>) -> Sender<WorkerRequest> {
    let (request_tx, request_rx) = unbounded::<WorkerRequest>();

    // The single worker handles all blocking jobs serially (FIFO). Search and
    // suggestion requests carry a `generation` so the UI can discard stale replies.
    std::thread::spawn(move || {
        let mut generation = 0u64;
        while let Ok(request) = request_rx.recv() {
            generation = generation.wrapping_add(1);
            match request {
                WorkerRequest::SearchSuggestions { query, .. } => {
                    let suggestions = commands::search_suggestions(&services, query);
                    let _ = event_tx.send(UiEvent::SuggestionsFinished { generation, suggestions });
                }
                WorkerRequest::Search {
                    query,
                    page,
                    limit,
                    file_type,
                    date_limit,
                    ..
                } => {
                    let results = commands::search_fts(&services, query, page, limit, file_type, date_limit);
                    let _ = event_tx.send(UiEvent::SearchFinished { generation, results });
                }
                WorkerRequest::RecentDocs { page, limit, file_type } => {
                    let results = commands::recent_docs(&services, page, limit, file_type);
                    let _ = event_tx.send(UiEvent::RecentDocs { results });
                }
                WorkerRequest::DbStats => {
                    let stats = commands::db_stats(&services);
                    let _ = event_tx.send(UiEvent::DbStats { stats });
                }
                WorkerRequest::CountParsed => {
                    let count = commands::count_parsed(&services);
                    let _ = event_tx.send(UiEvent::CountParsed { count });
                }
                WorkerRequest::TextForFile { document_id } => {
                    let text = commands::text_for_file(&services, document_id);
                    let _ = event_tx.send(UiEvent::TextForFile { text });
                }
                WorkerRequest::ExtractPdf { file_path } => {
                    let text = services
                        .runtime()
                        .block_on(commands::extract_pdf_text(&services, file_path));
                    let _ = event_tx.send(UiEvent::PdfTextExtracted { text });
                }
                WorkerRequest::WriteTextToFile { file_path, text } => {
                    commands::write_text(&services, file_path, text);
                    let _ = event_tx.send(UiEvent::TextWritten);
                }
                WorkerRequest::ReadTextFromFile { file_path } => {
                    let text = commands::read_text(&services, file_path);
                    let _ = event_tx.send(UiEvent::TextRead { text });
                }
                WorkerRequest::ImageBase64 { file_path } => {
                    let data = commands::image_base64(&services, file_path);
                    let _ = event_tx.send(UiEvent::ImageBase64 { data });
                }
                WorkerRequest::FetchStatistics => {
                    let stats = commands::statistics(&services);
                    let _ = event_tx.send(UiEvent::Statistics { stats });
                }
                WorkerRequest::FetchDashboard => {
                    let stats = commands::dashboard(&services);
                    let _ = event_tx.send(UiEvent::Dashboard { stats });
                }
                WorkerRequest::IgnorePath { path, is_folder, ignore_indexing } => {
                    commands::ignore_path(&services, path, is_folder, ignore_indexing);
                    let _ = event_tx.send(UiEvent::PathIgnored);
                }
                WorkerRequest::ShowIgnoredPaths => {
                    let paths = commands::ignored_paths(&services);
                    let _ = event_tx.send(UiEvent::IgnoredPaths { paths });
                }
                WorkerRequest::RemoveFromIgnoreList { paths } => {
                    commands::remove_from_ignore_list(&services, paths);
                    let _ = event_tx.send(UiEvent::PathsRemovedFromIgnoreList);
                }
                WorkerRequest::TantivyFilesSearch { query, limit, page } => {
                    let results = commands::tantivy_files_search(&services, query, limit, page);
                    let _ = event_tx.send(UiEvent::TantivyFiles { results });
                }
                WorkerRequest::TantivyBookmarksSearch { query, limit, page } => {
                    let results = commands::tantivy_bookmarks_search(&services, query, limit, page);
                    let _ = event_tx.send(UiEvent::TantivyBookmarks { results });
                }
                WorkerRequest::CsvDump => {
                    commands::csv_dump(&services);
                    let _ = event_tx.send(UiEvent::CsvDumped);
                }
                WorkerRequest::BrowserProfilesChrome => {
                    let profiles = commands::chrome_profiles();
                    let _ = event_tx.send(UiEvent::ChromeProfiles { profiles });
                }
                WorkerRequest::BrowserProfilesArc => {
                    let profiles = commands::arc_profiles();
                    let _ = event_tx.send(UiEvent::ArcProfiles { profiles });
                }
                WorkerRequest::BrowserHistorySearch { profile, query, limit, page } => {
                    let results = commands::browser_history(profile, query, limit, page);
                    let _ = event_tx.send(UiEvent::BrowserHistory { results });
                }
                WorkerRequest::RescanDocuments { rescan_all } => {
                    services.sync.run_sync(false, Vec::new(), rescan_all, true);
                }
                WorkerRequest::ClearIndex => {
                    commands::clear_index(&services);
                    let _ = event_tx.send(UiEvent::IndexCleared);
                }
            }
        }
    });

    request_tx
}

/// Convenience for sending a request from the UI thread (best-effort).
pub fn send(request_tx: &Sender<WorkerRequest>, request: WorkerRequest) {
    let _ = request_tx.send(request);
}