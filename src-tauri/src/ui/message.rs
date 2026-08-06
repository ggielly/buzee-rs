use crate::application::UiEvent;
use crate::application::AppServices;
use crate::infrastructure::platform::hotkey;
use crate::ui::state::{BuzeeUiState, Screen, SortColumn, ViewMode};
use crate::ui::theme::ThemeChoice;
use std::sync::Arc;

/// Boolean user preferences editable from the Settings screen.
#[derive(Debug, Clone, Copy)]
pub enum BoolPref {
    SearchSuggestions,
    LaunchAtStartup,
    GlobalShortcutEnabled,
    AutomaticBackgroundSync,
    DetailedScan,
    ParsePdfs,
    EnableLogs,
}

/// Keyboard navigation direction for the results list.
#[derive(Debug, Clone, Copy)]
pub enum ResultsNav {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// All user-initiated and system actions the UI can react to.
#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    SearchInputChanged(String),
    RunSearch,
    /// Run a search with an explicit query (used by the suggestions list).
    SearchSuggestionSelected(String),
    Close,
    /// Minimize the window (Window menu).
    Minimize,
    /// Toggle the window between maximized and restored (Window menu).
    ToggleMaximize,
    /// Hard-quit the application.
    Quit,
    /// Edit menu: undo the last change to the search field.
    EditUndo,
    /// Edit menu: redo the last undone change to the search field.
    EditRedo,
    /// Edit menu: copy the search field text to the clipboard and clear it.
    EditCut,
    /// Edit menu: copy the search field text to the clipboard.
    EditCopy,
    /// Edit menu: paste the clipboard content into the search field.
    EditPaste,
    /// The content read from the clipboard (produced by [`Message::EditPaste`]).
    ClipboardRead(Option<String>),
    /// Open the About dialog.
    OpenAbout,
    /// Dismiss the About dialog.
    CloseAbout,
    ToggleSync,
    StartOcr,
    StopOcr,
    OpenResult(String),
    RevealResult(String),
    GlobalShortcutPressed,
    LocationChanged(String),
    FileTypeChanged(Option<String>),
    DateRangeChanged(Option<(String, i64, i64)>),
    ViewModeChanged(ViewMode),
    CompactChanged(bool),
    SortChanged(SortColumn),
    SelectResult(i32),
    /// Record the last known cursor position (logical coordinates).
    CursorMoved(f32, f32),
    /// Open the right-click context menu for the selected result at the last
    /// cursor position.
    ContextMenuRequested,
    /// Dismiss the context menu.
    CloseContextMenu,
    /// Keep the table header horizontally in sync with the body scroll.
    TableSync(iced::widget::operation::AbsoluteOffset),
    /// Move the results selection with the keyboard (arrow keys, Home/End).
    NavigateResults(ResultsNav),
    /// Switch to another top-level screen.
    Navigate(Screen),
    /// Toggle a boolean user preference.
    SetBoolPref(BoolPref, bool),
    /// Select a UI theme from the settings dropdown.
    ThemeSelected(ThemeChoice),
    /// Change the OCR sort order preference.
    SetOcrSortOrder(String),
    /// Persist the numeric OCR inputs.
    SaveOcrNumbers,
    OcrPagesInputChanged(String),
    OcrThreadsInputChanged(String),
    /// Persist the global shortcut text input.
    SaveShortcut,
    ShortcutInputChanged(String),
    /// Request a full index clear.
    ClearIndex,
    /// Request a document rescan (`true` = rescan all).
    RescanDocuments(bool),
    /// Stop the currently running scan (popup cancel button).
    StopSync,
    /// Dismiss the completed-scan popup.
    DismissScanPopup,
    /// Ignore-list form.
    IgnoreInputChanged(String),
    AddIgnorePath,
    RemoveIgnored(Vec<String>),
    /// Extract-text screen.
    ExtractInputChanged(String),
    RunExtractText,
    /// Refresh the dashboard data.
    RefreshDashboard,
    Noop,
}

pub fn inspect(event: UiEvent) -> Message {
    match event {
        UiEvent::GlobalShortcutPressed => Message::GlobalShortcutPressed,
        _ => Message::Noop,
    }
}

/// Start a background thread that listens for the global hotkey and forwards a
/// `GlobalShortcutPressed` UI event when the registered key is pressed. The
/// registration is best-effort: a failure is logged and the app continues.
pub fn start_hotkey_listener(
    enabled: bool,
    shortcut_string: String,
    services: Arc<AppServices>,
) {
    if !enabled {
        log::info!("Global Shortcut is disabled");
        return;
    }
    log::info!("Global Shortcut is enabled");
    let event_tx = services.event_tx.clone();

    std::thread::spawn(move || {
        let manager = match global_hotkey::GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                log::error!("Failed to create global hotkey manager: {}", e);
                return;
            }
        };
        if let Err(e) = hotkey::register_best_effort(&manager, &shortcut_string) {
            log::error!("Failed to register the global shortcut ({}); continuing without it.", e);
            return;
        }

        let receiver = global_hotkey::GlobalHotKeyEvent::receiver();

        loop {
            match receiver.recv() {
                Ok(_event) => {
                    log::info!("Global Shortcut Detected!");
                    let _ = event_tx.send(UiEvent::GlobalShortcutPressed);
                }
                Err(_) => break,
            }
        }
    });
}

/// Convenience to build a `Message` from a low-level worker event; kept here so
/// view/state code does not import the application layer directly when possible.
pub fn from_worker_event(_event: crate::domain::events::WorkerEvent) -> Message {
    Message::Noop
}

/// True when the search box should be focused on launch.
pub fn should_focus_search(state: &BuzeeUiState) -> bool {
    state.search_input.is_empty()
}

/// Normalize the stored shortcut string for hotkey registration.
pub fn normalized_shortcut(prefs: &crate::domain::types::UserPreferencesState) -> String {
    prefs.global_shortcut.clone()
}