//! The native iced UI. Implements the top-level `Application` that owns the
//! shared services, drives search through the worker channel and renders the
//! search view with a status bar.

pub mod message;
pub mod state;
pub mod view;
pub mod theme;
pub mod styles;
pub mod fonts;
pub mod icons;
pub mod context_menu;
pub mod charts;
pub mod dashboard;
pub mod result_table;
pub mod settings;
pub mod ignore;
pub mod screens;

use crate::application::{UiEvent, WorkerRequest};
use crate::application::AppServices;
use crate::domain::types::DateLimit;
use crate::infrastructure::database::models::DocumentSearchResult;
use crate::infrastructure::user_prefs;
use iced::{Element, Subscription, Task};
use message::Message;
use state::{initial_state, BuzeeUiState, ScanPhase};
use std::sync::Arc;
use theme::Theme;

/// Approximate height of one result row, used to scroll the table to the
/// keyboard-selected entry.
const RESULT_ROW_HEIGHT: f32 = 40.0;

/// Flags handed to iced to reconstruct the fully-built application.
pub struct AppFlags {
    pub services: Arc<AppServices>,
    pub request_tx: crossbeam_channel::Sender<WorkerRequest>,
    pub event_rx: crossbeam_channel::Receiver<UiEvent>,
    pub theme: Theme,
}

pub struct BuzeeApp {
    pub services: Arc<AppServices>,
    pub request_tx: crossbeam_channel::Sender<WorkerRequest>,
    pub state: BuzeeUiState,
}

impl BuzeeApp {
    pub fn new(flags: AppFlags) -> (Self, Task<Message>) {
        let state = initial_state(
            flags
                .services
                .preferences
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            flags.event_rx,
            flags.theme,
        );

        let mut app = Self {
            services: flags.services,
            request_tx: flags.request_tx,
            state,
        };
        let cmd = app.initial_commands();
        (app, cmd)
    }

    fn initial_commands(&mut self) -> Task<Message> {
        let hotkey_enabled =
            user_prefs::is_global_shortcut_enabled(&self.services.preferences);
        let hotkey_string = user_prefs::get_global_shortcut_string(&self.services.preferences);
        crate::ui::message::start_hotkey_listener(hotkey_enabled, hotkey_string, self.services.clone());

        self.services.sync.schedule();
        if !self.state.preferences.automatic_background_sync {
            self.services.sync.run_sync(false, Vec::new(), false, false);
        }
        Task::batch(vec![self.refresh_dashboard(), self.refresh_statistics()])
    }

    fn refresh_dashboard(&self) -> Task<Message> {
        crate::application::workers::send(&self.request_tx, WorkerRequest::FetchDashboard);
        Task::none()
    }

    fn refresh_statistics(&self) -> Task<Message> {
        crate::application::workers::send(&self.request_tx, WorkerRequest::FetchStatistics);
        Task::none()
    }

    /// Run a search for the current query/filters. An empty query lists the
    /// most recently opened documents (matching the original app, which calls
    /// `triggerSearch()` when the Search page opens).
    fn run_search(&mut self) {
        self.state.query_gen += 1;
        let file_type = self.state.file_type.clone();
        if self.state.search_input.trim().is_empty() {
            crate::application::workers::send(
                &self.request_tx,
                WorkerRequest::RecentDocs {
                    page: 0,
                    limit: 50,
                    file_type,
                },
            );
        } else {
            let date_limit = self.state.date_range.as_ref().map(|r| DateLimit {
                start: r.1.to_string(),
                end: r.2.to_string(),
                text: r.0.clone(),
            });
            crate::application::workers::send(
                &self.request_tx,
                WorkerRequest::Search {
                    query: self.state.search_input.clone(),
                    page: 0,
                    limit: 50,
                    file_type,
                    date_limit,
                    generation: self.state.query_gen,
                },
            );
        }
    }

    /// Store fresh search results, keep them sorted and refresh the table
    /// column descriptors.
    fn set_search_results(&mut self, results: Vec<DocumentSearchResult>) {
        self.state.results = results;
        self.state.selected_index = 0;
        self.state.selected_result = None;
        self.state.context_menu = None;
        self.sort_results();
        self.sync_result_columns();
    }

    /// Sort `state.results` in place using the current sort key. The
    /// virtualized table renders the (sorted) results directly, so the sort is
    /// applied here rather than as an index permutation in the view.
    fn sort_results(&mut self) {
        let sort = self.state.sort;
        let res = &mut self.state.results;
        res.sort_by(|a, b| {
            let ord = match sort.column {
                state::SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                state::SortColumn::Type => a.file_type.cmp(&b.file_type),
                state::SortColumn::LastModified => a.last_modified.cmp(&b.last_modified),
                state::SortColumn::LastOpened => a.last_opened.cmp(&b.last_opened),
                state::SortColumn::Size => {
                    let sa = a.size.unwrap_or(0.0);
                    let sb = b.size.unwrap_or(0.0);
                    sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
                }
                state::SortColumn::Location => result_table::parent_dir(&a.path)
                    .cmp(&result_table::parent_dir(&b.path)),
            };
            if sort.asc {
                ord
            } else {
                ord.reverse()
            }
        });
    }

    /// Rebuild the data-only table column descriptors from the current state so
    /// `iced_table` can borrow them for the whole frame.
    fn sync_result_columns(&mut self) {
        self.state.result_columns = result_table::columns_for(&self.state);
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        use crate::application::events::UiEvent::*;
        match event {
            Status(worker_event) => {
                self.state.touch();
                use crate::domain::events::WorkerEvent::*;
                match worker_event {
                    SyncStatus { running, popup } => {
                        self.state.scan_running = running;
                        if running {
                            self.state.scan_complete = false;
                            if popup {
                                self.state.scan_popup_active = true;
                                if self.state.scan_phase == ScanPhase::Idle {
                                    self.state.scan_phase = ScanPhase::Scanning;
                                }
                            }
                        } else {
                            self.state.scan_complete = self.state.scan_popup_active;
                        }
                    }
                    FilesAdded { count, .. } => {
                        self.state.scan_phase = ScanPhase::Scanning;
                        self.state.scan_files_added = count;
                    }
                    ScanStarted { total } => {
                        self.state.scan_phase = ScanPhase::Parsing;
                        self.state.scan_total = total;
                        self.state.scan_processed = 0;
                        self.state.scan_success = 0;
                        self.state.scan_failed = 0;
                        self.state.scan_current_file.clear();
                        self.state.scan_items.clear();
                    }
                    ScanProgress { processed, total, success, failed, file, failed_files } => {
                        self.state.scan_phase = ScanPhase::Parsing;
                        self.state.scan_processed = processed;
                        self.state.scan_total = total;
                        self.state.scan_success = success;
                        self.state.scan_failed = failed;
                        self.state.scan_current_file = file;
                        self.state.scan_items = failed_files
                            .into_iter()
                            .map(|f| crate::domain::types::ScanItem {
                                path: f.path,
                                name: f.name,
                                error: Some(f.error),
                            })
                            .collect();
                    }
                    OcrRescanProgress(p) => {
                        self.state.scan_phase = ScanPhase::Ocr;
                        self.state.scan_processed = p.processed;
                        self.state.scan_total = p.total;
                        self.state.scan_success = p.success;
                        self.state.scan_failed = p.failed;
                        self.state.scan_current_file = p.current_file;
                        self.state.scan_items = p
                            .failed_files
                            .into_iter()
                            .map(|f| crate::domain::types::ScanItem {
                                path: f.path,
                                name: f.name,
                                error: Some(f.error),
                            })
                            .collect();
                    }
                    SyncFinished => {}
                }
            }
            Error(err) => {
                log::error!("Worker error: {}", err);
            }
            SuggestionsFinished { generation, suggestions } => {
                if generation >= self.state.query_gen {
                    self.state.suggestions = suggestions;
                }
            }
            SearchFinished { generation, results } => {
                if generation >= self.state.query_gen {
                    self.set_search_results(results);
                    self.state.suggestions = vec![];
                }
            }
            RecentDocs { results } => {
                self.state.recent_docs = results.clone();
                // Populate the Search results with the recent documents only
                // while the query is empty, so a stale reply never overwrites
                // an actual search.
                if self.state.search_input.trim().is_empty() {
                    self.set_search_results(results);
                    self.state.suggestions = vec![];
                }
            }
            DbStats { stats } => self.state.db_stats = stats,
            CountParsed { count } => self.state.parsed_count = count,
            Statistics { stats } => {
                self.state.statistics = Some(stats.clone());
                self.state.parsed_count = stats.parsed_files;
                self.state.status = stats.status.clone();
            }
            Dashboard { stats } => self.state.dashboard = Some(stats),
            TextForFile { text } => self.state.file_text = text,
            PdfTextExtracted { text } => {
                self.state.file_text = text.clone();
                self.state.extract_output = Some(text.join("\n"));
            }
            TextRead { text } => self.state.file_text = vec![text],
            ImageBase64 { .. } => {}
            TextWritten => {}
            PathIgnored => self.refresh_ignored(),
            IgnoredPaths { paths } => self.state.ignored_paths = paths,
            PathsRemovedFromIgnoreList => self.refresh_ignored(),
            TantivyFiles { results } => self.state.tantivy_files = results,
            TantivyBookmarks { results } => self.state.tantivy_bookmarks = results,
            CsvDumped => {}
            ChromeProfiles { profiles } => self.state.chrome_profiles = profiles,
            ArcProfiles { profiles } => self.state.arc_profiles = profiles,
            BrowserHistory { results } => self.state.browser_history = results,
            IndexCleared => {}
            SyncFinished => {
                let _ = self.refresh_dashboard();
            }
            Preferences(prefs) => {
                self.state.preferences = prefs;
                self.state.touch();
            }
            GlobalShortcutPressed => {
                self.state.focus_search = true;
            }
        }
    }

    fn refresh_ignored(&mut self) {
        crate::application::workers::send(&self.request_tx, WorkerRequest::ShowIgnoredPaths);
    }

    /// Persist a changed preference to the DB and reload the shared prefs state
    /// so both the worker and the UI see the new value.
    fn apply_pref_change(&mut self, apply: impl FnOnce()) {
        apply();
        let pool = self.services.db_pool.clone();
        let prefs = self.services.preferences.clone();
        crate::infrastructure::user_prefs::set_user_preferences_state_from_db_value(&prefs, &pool);
        let prefs = prefs.read().unwrap_or_else(|e| e.into_inner());
        let state = prefs.clone();
        self.state.preferences = state;
        let _ = self.services.event_tx.send(UiEvent::Preferences(self.state.preferences.clone()));
    }

    fn set_bool_pref(&mut self, key: message::BoolPref, value: bool) {
        let pool = self.services.db_pool.clone();
        self.apply_pref_change(move || match key {
            message::BoolPref::SearchSuggestions => {
                user_prefs::set_show_search_suggestions_flag_in_db(value, &pool)
            }
            message::BoolPref::LaunchAtStartup => {
                user_prefs::set_launch_at_startup_flag_in_db(value, &pool)
            }
            message::BoolPref::GlobalShortcutEnabled => {
                user_prefs::set_global_shortcut_flag_in_db(value, &pool)
            }
            message::BoolPref::AutomaticBackgroundSync => {
                user_prefs::set_automatic_background_sync_flag_in_db(value, &pool)
            }
            message::BoolPref::DetailedScan => {
                user_prefs::set_detailed_scan_flag_in_db(value, &pool)
            }
            message::BoolPref::ParsePdfs => user_prefs::set_parse_pdfs_flag_in_db(value, &pool),
            message::BoolPref::EnableLogs => user_prefs::set_enable_logs_flag_in_db(value, &pool),
        });
    }

    fn tick(&mut self) {
        self.state.touch();
        // Debounced suggestions: the latest typed query (>= 3 chars) is sent to
        // the worker at most once per tick (~250 ms), so a fast typist does not
        // flood the FIFO worker queue ahead of an actual search.
        if let Some(query) = self.state.pending_suggestions.take() {
            if query.chars().count() >= 3 {
                crate::application::workers::send(
                    &self.request_tx,
                    WorkerRequest::SearchSuggestions {
                        query,
                        generation: self.state.query_gen,
                    },
                );
            }
        }
        // Throttle statistics refreshes (~5s) and skip them entirely while a
        // scan is running: the scan floods the worker queue with DB work and a
        // statistics query on every 250 ms tick starves the FIFO worker thread,
        // which is what made the app appear frozen during a rescan.
        self.state.stats_tick = self.state.stats_tick.wrapping_add(1);
        if self.state.stats_tick % 20 == 0 && !self.state.scan_running {
            let _ = self.refresh_statistics();
        }
        let events = self.state.drain_events();
        for event in events {
            self.handle_ui_event(event);
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => self.tick(),
            Message::SearchInputChanged(value) => {
                // Keep a bounded undo history for the Edit menu.
                if self.state.search_input != value {
                    self.state
                        .edit_undo
                        .push(std::mem::replace(&mut self.state.search_input, value.clone()));
                    if self.state.edit_undo.len() > 50 {
                        self.state.edit_undo.remove(0);
                    }
                    self.state.edit_redo.clear();
                }
                // Queue the query for the debounced suggestion request (sent on
                // the next Tick). An empty/too-short query cancels any pending one.
                self.state.pending_suggestions = if value.chars().count() >= 3 {
                    Some(value)
                } else {
                    None
                };
            }
            Message::RunSearch => {
                self.run_search();
            }
            Message::SearchSuggestionSelected(query) => {
                self.state.search_input = query;
                self.state.screen = state::Screen::Search;
                self.run_search();
            }
            Message::LocationChanged(location) => {
                self.state.location = location;
            }
            Message::FileTypeChanged(file_type) => {
                self.state.file_type = file_type;
                self.run_search();
            }
            Message::DateRangeChanged(range) => {
                self.state.date_range = range;
                self.run_search();
            }
            Message::ViewModeChanged(mode) => {
                self.state.view_mode = mode;
                self.state.context_menu = None;
            }
            Message::CompactChanged(compact) => {
                self.state.compact_view = compact;
            }
            Message::SortChanged(column) => {
                if self.state.sort.column == column {
                    self.state.sort.asc = !self.state.sort.asc;
                } else {
                    self.state.sort = state::SortState { column, asc: true };
                }
                self.sort_results();
                self.sync_result_columns();
            }
            Message::SelectResult(id) => {
                self.state.selected_result = Some(id);
                self.state.context_menu = None;
                self.sync_result_columns();
            }
            Message::CursorMoved(x, y) => {
                self.state.cursor = (x, y);
            }
            Message::ContextMenuRequested => {
                // The context menu only makes sense over the results table.
                if self.state.screen == state::Screen::Search
                    && self.state.view_mode == state::ViewMode::List
                    && !self.state.results.is_empty()
                {
                    if self.state.selected_result.is_none() {
                        self.state.selected_result = self.state.results.first().map(|r| r.id);
                        self.sync_result_columns();
                    }
                    self.state.context_menu = Some(self.state.cursor);
                }
            }
            Message::CloseContextMenu => {
                self.state.context_menu = None;
            }
            Message::NavigateResults(nav) => {
                // Arrow keys are handled globally, but only while no query is
                // being typed so the search input keeps its caret behaviour.
                if !self.state.search_input.trim().is_empty() {
                    return Task::none();
                }
                let count = self.state.results.len();
                if count == 0 {
                    return Task::none();
                }
                use message::ResultsNav;
                let current = self.state.selected_index.min(count - 1);
                let next = match nav {
                    ResultsNav::Down => (current + 1).min(count - 1),
                    ResultsNav::Up => current.saturating_sub(1),
                    ResultsNav::PageDown => (current + 10).min(count - 1),
                    ResultsNav::PageUp => current.saturating_sub(10),
                    ResultsNav::Home => 0,
                    ResultsNav::End => count - 1,
                };
                self.state.selected_index = next;
                if let Some(res) = self.state.results.get(next) {
                    self.state.selected_result = Some(res.id);
                }
                self.sync_result_columns();
                // Keep the selected row visible: jump the results scrollable to
                // the approximate row offset.
                return iced::widget::operation::scroll_to::<Message>(
                    iced::widget::Id::new("results-table"),
                    iced::widget::operation::AbsoluteOffset {
                        x: 0.0,
                        y: next as f32 * RESULT_ROW_HEIGHT,
                    },
                );
            }
            Message::TableSync(offset) => {
                // Keep the virtualized header horizontally in sync with the
                // body when the user scrolls sideways.
                return iced::widget::operation::scroll_to::<Message>(
                    iced::widget::Id::new("results-table-header"),
                    iced::widget::operation::AbsoluteOffset { x: offset.x, y: 0.0 },
                );
            }
            Message::Minimize => {
                // The main window id isn't exposed as a constant in iced 0.14,
                // so resolve it from the single-window app and minimize it.
                return iced::window::latest().and_then(|id| iced::window::minimize(id, true));
            }
            Message::ToggleMaximize => {
                return iced::window::latest().and_then(|id| iced::window::toggle_maximize(id));
            }
            Message::Close => {
                return iced::window::latest().and_then(|id| iced::window::close(id));
            }
            Message::Quit => {
                std::process::exit(0);
            }
            Message::EditUndo => {
                if let Some(previous) = self.state.edit_undo.pop() {
                    self.state.edit_redo.push(std::mem::replace(
                        &mut self.state.search_input,
                        previous,
                    ));
                }
            }
            Message::EditRedo => {
                if let Some(next) = self.state.edit_redo.pop() {
                    self.state.edit_undo.push(std::mem::replace(
                        &mut self.state.search_input,
                        next,
                    ));
                }
            }
            Message::EditCut => {
                let text = std::mem::take(&mut self.state.search_input);
                if !text.is_empty() {
                    self.state.edit_undo.push(String::new());
                    self.state.edit_redo.clear();
                    return iced::clipboard::write::<Message>(text);
                }
            }
            Message::EditCopy => {
                let text = self.state.search_input.clone();
                if !text.is_empty() {
                    return iced::clipboard::write::<Message>(text);
                }
            }
            Message::EditPaste => {
                return iced::clipboard::read().map(Message::ClipboardRead);
            }
            Message::ClipboardRead(content) => {
                if let Some(text) = content {
                    if !text.is_empty() {
                        self.state.edit_undo.push(std::mem::replace(
                            &mut self.state.search_input,
                            text,
                        ));
                        self.state.edit_redo.clear();
                    }
                }
            }
            Message::OpenAbout => self.state.show_about = true,
            Message::CloseAbout => self.state.show_about = false,
            Message::ToggleSync => {
                self.services.sync.run_sync(false, Vec::new(), false, true);
            }
            Message::StartOcr => {
                let ocr = self.services.ocr.clone();
                if ocr.start_full_rescan().is_err() {
                    log::warn!("OCR rescan already running");
                }
            }
            Message::StopOcr => self.services.ocr.cancel(),
            Message::OpenResult(path) => {
                self.state.context_menu = None;
                let platform = self.services.platform.clone();
                let _ = platform.open_path(&path);
            }
            Message::RevealResult(path) => {
                self.state.context_menu = None;
                let platform = self.services.platform.clone();
                let _ = platform.reveal_path(&path);
            }
            Message::GlobalShortcutPressed => {
                self.state.touch();
            }
            Message::Navigate(screen) => {
                self.state.screen = screen;
                self.state.context_menu = None;
                match screen {
                    state::Screen::Dashboard => {
                        let _ = self.refresh_dashboard();
                    }
                    state::Screen::Ignore => self.refresh_ignored(),
                    state::Screen::Search => self.run_search(),
                    _ => {}
                }
            }
            Message::SetBoolPref(key, value) => self.set_bool_pref(key, value),
            Message::ThemeSelected(choice) => {
                self.state.theme = choice.theme;
                self.sync_result_columns();
                let pool = self.services.db_pool.clone();
                if let Ok(theme_json) = serde_json::to_string(&choice.theme) {
                    user_prefs::set_app_theme_in_db(&theme_json, &pool);
                }
            }
            Message::SetOcrSortOrder(order) => {
                let pool = self.services.db_pool.clone();
                self.apply_pref_change(move || user_prefs::set_ocr_sort_order_in_db(order, &pool));
            }
            Message::OcrPagesInputChanged(value) => self.state.ocr_pages_input = value,
            Message::OcrThreadsInputChanged(value) => self.state.ocr_threads_input = value,
            Message::SaveOcrNumbers => {
                let pages = self.state.ocr_pages_input.parse::<i64>().unwrap_or(150).clamp(1, 5000);
                let threads = self.state.ocr_threads_input.parse::<i64>().unwrap_or(1).clamp(1, 4);
                self.state.ocr_pages_input = pages.to_string();
                self.state.ocr_threads_input = threads.to_string();
                let pool = self.services.db_pool.clone();
                self.apply_pref_change(move || {
                    user_prefs::set_pdf_max_ocr_pages_in_db(pages, &pool);
                    user_prefs::set_ocr_threads_in_db(threads, &pool);
                });
            }
            Message::ShortcutInputChanged(value) => self.state.shortcut_input = value,
            Message::SaveShortcut => {
                let shortcut = user_prefs::fix_global_shortcut_string(self.state.shortcut_input.clone());
                self.state.shortcut_input = shortcut.clone();
                let pool = self.services.db_pool.clone();
                self.apply_pref_change(move || user_prefs::set_new_global_shortcut_in_db(shortcut, &pool));
            }
            Message::ClearIndex => {
                crate::application::workers::send(&self.request_tx, WorkerRequest::ClearIndex);
            }
            Message::StopSync => {
                self.services.sync.run_sync(true, Vec::new(), false, false);
            }
            Message::DismissScanPopup => {
                self.state.reset_scan();
                let _ = self.refresh_dashboard();
            }
            Message::RescanDocuments(rescan_all) => {
                crate::application::workers::send(
                    &self.request_tx,
                    WorkerRequest::RescanDocuments { rescan_all },
                );
            }
            Message::IgnoreInputChanged(value) => self.state.ignore_input = value,
            Message::AddIgnorePath => {
                let path = self.state.ignore_input.trim().to_string();
                if !path.is_empty() {
                    crate::application::workers::send(
                        &self.request_tx,
                        WorkerRequest::IgnorePath {
                            path,
                            is_folder: true,
                            ignore_indexing: false,
                        },
                    );
                    self.state.ignore_input.clear();
                }
            }
            Message::RemoveIgnored(paths) => {
                if !paths.is_empty() {
                    crate::application::workers::send(
                        &self.request_tx,
                        WorkerRequest::RemoveFromIgnoreList { paths },
                    );
                }
            }
            Message::ExtractInputChanged(value) => self.state.extract_input = value,
            Message::RunExtractText => {
                let path = self.state.extract_input.trim().to_string();
                if !path.is_empty() {
                    crate::application::workers::send(
                        &self.request_tx,
                        WorkerRequest::ExtractPdf { file_path: path },
                    );
                    self.state.extract_output = None;
                }
            }
            Message::RefreshDashboard => {
                let _ = self.refresh_dashboard();
            }
            Message::Noop => {}
        }

        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        use iced::keyboard::key::Named;
        use iced::keyboard::{Event, Key};

        let tick = iced::time::every(std::time::Duration::from_millis(250)).map(|_| Message::Tick);

        let keys = iced::keyboard::listen().filter_map(|event| match event {
            Event::KeyPressed { key, .. } => {
                let named = match key {
                    Key::Named(named) => named,
                    _ => return None,
                };
                let msg = match named {
                    Named::Escape => Some(Message::CloseContextMenu),
                    Named::ArrowUp => Some(Message::NavigateResults(message::ResultsNav::Up)),
                    Named::ArrowDown => Some(Message::NavigateResults(message::ResultsNav::Down)),
                    Named::PageUp => Some(Message::NavigateResults(message::ResultsNav::PageUp)),
                    Named::PageDown => Some(Message::NavigateResults(message::ResultsNav::PageDown)),
                    Named::Home => Some(Message::NavigateResults(message::ResultsNav::Home)),
                    Named::End => Some(Message::NavigateResults(message::ResultsNav::End)),
                    _ => None,
                };
                msg
            }
            _ => None,
        });

        // Track the cursor and open the context menu on right-click. The open
        // position is the most recent `CursorMoved` point.
        let mouse = iced::event::listen_with(|event, _status, _window| match event {
            iced::event::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Some(Message::CursorMoved(position.x, position.y))
            }
            iced::event::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Right)) => {
                Some(Message::ContextMenuRequested)
            }
            _ => None,
        });

        iced::Subscription::batch([tick, keys, mouse])
    }

    pub fn view(&self) -> Element<'_, Message, Theme> {
        view::root(self)
    }

    pub fn theme(&self) -> Option<Theme> {
        Some(self.state.theme)
    }
}