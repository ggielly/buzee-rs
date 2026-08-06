use crate::domain::types::{AppStatistics, UserPreferencesState, DbPool};
use crate::infrastructure::database::establish_connection;
use crate::infrastructure::database::search;
use crate::infrastructure::housekeeping::{get_documents_directory, APP_DIRECTORY};
use crate::infrastructure::utils::norm;

pub const DB_NAME: &str = "buzee.db";

pub const AUTO_SCAN_INTERVAL_SECS: i64 = 1800;

const NO_RECORD_GUESS_SECS: i64 = AUTO_SCAN_INTERVAL_SECS;

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let value = bytes as f64;
    if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn seconds_until_next_scan(last_scan_time: i64, now: i64) -> i64 {
    if last_scan_time <= 0 {
        return NO_RECORD_GUESS_SECS;
    }
    (last_scan_time + AUTO_SCAN_INTERVAL_SECS - now).max(0)
}

pub fn derive_status(scan_running: bool, auto_sync_enabled: bool) -> &'static str {
    if scan_running {
        "scanning"
    } else if auto_sync_enabled {
        "ready"
    } else {
        "idle"
    }
}

fn base_directory() -> Option<String> {
    get_documents_directory().map(|dir| norm(&format!("{}/{}", dir, APP_DIRECTORY)))
}

pub fn database_file_size() -> u64 {
    let Some(base) = base_directory() else {
        return 0;
    };
    let db = format!("{}/{}", base, DB_NAME);
    file_size(&db) + file_size(&format!("{}-wal", db))
}

fn file_size(path: &str) -> u64 {
    std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

pub fn get_app_statistics(
    pool: &DbPool,
    sync_running: bool,
    last_sync_time: i64,
    preferences: &UserPreferencesState,
) -> AppStatistics {
    let auto_sync = preferences.automatic_background_sync;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let total = {
        let mut conn = establish_connection(pool);
        search::get_total_document_count(&mut conn).unwrap_or(0)
    };
    let parsed = {
        let conn = establish_connection(pool);
        search::get_file_parsed_count(conn).unwrap_or(0)
    };

    AppStatistics {
        status: derive_status(sync_running, auto_sync).to_string(),
        total_files: total,
        parsed_files: parsed,
        database_size_bytes: database_file_size(),
        last_scan_time: last_sync_time,
        next_scan_in_seconds: if auto_sync {
            seconds_until_next_scan(last_sync_time, now)
        } else {
            -1
        },
        auto_sync_enabled: auto_sync,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_is_human_readable() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }

    #[test]
    fn countdown_reaches_zero_when_due() {
        let now = 100_000;
        assert_eq!(seconds_until_next_scan(99_000, now), 800);
        assert_eq!(seconds_until_next_scan(98_200, now), 0);
        assert_eq!(seconds_until_next_scan(0, now), AUTO_SCAN_INTERVAL_SECS);
    }

    #[test]
    fn status_derivation() {
        assert_eq!(derive_status(true, true), "scanning");
        assert_eq!(derive_status(true, false), "scanning");
        assert_eq!(derive_status(false, true), "ready");
        assert_eq!(derive_status(false, false), "idle");
    }
}