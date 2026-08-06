use crate::domain::types::{OcrFailedFile, OcrRescanProgress};

/// Events emitted by long-running workers and consumed by the UI thread.
/// Replaces the old `send_message_to_frontend` string-based bridge.
#[derive(Debug, Clone)]
pub enum WorkerEvent {
    SyncStatus {
        running: bool,
        /// True when the scan was triggered by the user (shows the progress
        /// popup). Automatic/startup scans set this to false.
        popup: bool,
    },
    FilesAdded {
        count: usize,
        complete: bool,
    },
    ScanStarted {
        total: usize,
    },
    ScanProgress {
        processed: usize,
        total: usize,
        success: usize,
        failed: usize,
        /// The most recently processed file path.
        file: String,
        /// Cumulative list of files that failed to parse.
        failed_files: Vec<OcrFailedFile>,
    },
    SyncFinished,
    OcrRescanProgress(OcrRescanProgress),
}