pub mod hotkey;
pub mod lifecycle;
pub mod macos_activation;
pub mod open_reveal;
pub mod quicklook;

use crate::infrastructure::housekeeping::get_home_directory;
use std::path::PathBuf;

/// Platform abstraction over the OS-specific operations the UI needs.
pub trait PlatformService: Send + Sync {
    /// Open a file (in its default app) or a folder.
    fn open_path(&self, file_path: &str) -> Result<(), String>;

    /// Reveal a file in its containing folder, or open a folder.
    fn reveal_path(&self, file_path: &str) -> Result<(), String>;

    /// Launch the OS quick-preview (QuickLook on macOS, Peek on Windows).
    fn native_preview(&self, file_path: &str) -> Result<(), String>;

    /// Return the OS name string (e.g. "windows", "macos").
    fn os_name(&self) -> String;

    /// Start a drag-and-drop gesture for the given paths (best-effort, no-op
    /// where the OS does not support an out-of-window drag).
    fn start_drag(&self, paths: Vec<PathBuf>);

    /// Home directory used for resolving platform paths.
    fn home_directory(&self) -> Option<PathBuf>;
}

/// Default platform implementation based on `std::process::Command` + `open`.
pub struct DefaultPlatformService;

impl DefaultPlatformService {
    pub fn new() -> Self {
        DefaultPlatformService
    }
}

impl PlatformService for DefaultPlatformService {
    fn open_path(&self, file_path: &str) -> Result<(), String> {
        log::info!("Opening file or folder: {}", file_path);
        open::that(file_path).map_err(|e| e.to_string())
    }

    fn reveal_path(&self, file_path: &str) -> Result<(), String> {
        log::info!("Opening folder for {}", file_path);
        open_reveal::reveal_in_folder(file_path)
    }

    fn native_preview(&self, file_path: &str) -> Result<(), String> {
        quicklook::open_preview(file_path)
    }

    fn os_name(&self) -> String {
        std::env::consts::OS.to_string()
    }

    fn start_drag(&self, paths: Vec<PathBuf>) {
        log::info!("Start drag requested for {:?} paths", paths.len());
    }

    fn home_directory(&self) -> Option<PathBuf> {
        get_home_directory().map(PathBuf::from)
    }
}