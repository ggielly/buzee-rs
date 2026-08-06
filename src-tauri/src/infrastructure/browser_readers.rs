use crate::domain::types::HistoryResult;
use crate::domain::AppError;
use crate::infrastructure::database::models::DocumentSearchResult;
use rusqlite::{Connection, OpenFlags, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const DEFAULT_ARC_PROFILE_ID: &str = "Default";

#[cfg(target_os = "macos")]
const DEFAULT_ARC_PROFILE_PATH: [&str; 3] = ["Application Support", "Arc", "User Data"];
#[cfg(target_os = "macos")]
const DEFAULT_ARC_STATE_PATH: [&str; 4] =
    ["Application Support", "Arc", "User Data", "Local State"];

#[cfg(target_os = "windows")]
const DEFAULT_ARC_PROFILE_PATH: [&str; 3] = ["Roaming", "Arc", "User Data"];
#[cfg(target_os = "windows")]
const DEFAULT_ARC_STATE_PATH: [&str; 4] = ["Roaming", "Arc", "User Data", "Local State"];

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_ARC_PROFILE_PATH: [&str; 3] = ["Roaming", "Arc", "User Data"];
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_ARC_STATE_PATH: [&str; 4] = ["Roaming", "Arc", "User Data", "Local State"];

const DEFAULT_CHROME_PROFILE_ID: &str = "Default";

#[cfg(target_os = "macos")]
const DEFAULT_CHROME_PROFILE_PATH: [&str; 3] = ["Application Support", "Google", "Chrome"];
#[cfg(target_os = "macos")]
const DEFAULT_CHROME_STATE_PATH: [&str; 4] =
    ["Application Support", "Google", "Chrome", "Local State"];

#[cfg(target_os = "windows")]
const DEFAULT_CHROME_PROFILE_PATH: [&str; 3] = ["Google", "Chrome", "User Data"];
#[cfg(target_os = "windows")]
const DEFAULT_CHROME_STATE_PATH: [&str; 4] = ["Google", "Chrome", "User Data", "Local State"];

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_CHROME_PROFILE_PATH: [&str; 3] = ["Google", "Chrome", "User Data"];
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_CHROME_STATE_PATH: [&str; 4] = ["Google", "Chrome", "User Data", "Local State"];

fn user_library_directory_path() -> PathBuf {
    let home_path = dirs::home_dir().expect("Could not find home directory");
    home_path.join("Library")
}

fn user_data_directory_path() -> PathBuf {
    let home_dir = dirs::home_dir().expect("Could not find home directory");

    #[cfg(target_os = "macos")]
    let user_data_directory_path = home_dir
        .join("Library")
        .join("Application Support")
        .join("Firefox")
        .join("Profiles");

    #[cfg(target_os = "windows")]
    let user_data_directory_path = home_dir
        .join("AppData")
        .join("Roaming")
        .join("Mozilla")
        .join("Firefox")
        .join("Profiles");

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let user_data_directory_path = home_dir
        .join(".mozilla")
        .join("firefox");

    user_data_directory_path
}

fn get_firefox_profile_name(user_directory_path: &Path) -> Option<String> {
    let Ok(profiles) = fs::read_dir(user_directory_path) else {
        return None;
    };

    let mut release_profile = None;
    let mut nightly_profile = None;

    for profile in profiles {
        if let Ok(profile) = profile {
            let profile_name = profile.file_name().into_string().ok()?;
            if profile_name.ends_with(".default-release") {
                release_profile = Some(profile_name);
            } else if profile_name.ends_with(".default-nightly") {
                nightly_profile = Some(profile_name);
            }
        }
    }

    release_profile.or(nightly_profile)
}

fn get_firefox_history_db_path() -> Option<PathBuf> {
    let user_directory_path = user_data_directory_path();
    get_firefox_profile_name(&user_directory_path)
        .map(|profile_name| user_directory_path.join(profile_name).join("places.sqlite"))
}

fn get_history_db_path(profile_name: Option<&str>, default_profile_id: &str, default_profile_path: &[&str]) -> PathBuf {
    let profile = profile_name.unwrap_or(default_profile_id);
    let mut path = user_library_directory_path();
    for p in default_profile_path.iter() {
        path = path.join(p);
    }
    path.join(profile).join("History")
}

fn get_local_state_path(default_state_path: &[&str]) -> PathBuf {
    let mut path = user_library_directory_path();
    for p in default_state_path.iter() {
        path = path.join(p);
    }
    path
}

fn load_browser_profiles(default_profile_path: &[&str], default_state_path: &[&str]) -> Vec<HashMap<String, String>> {
    let path = get_local_state_path(default_state_path);
    if !path.exists() {
        return vec![{
            let mut default_profile = HashMap::new();
            default_profile.insert("name".to_string(), "Default".to_string());
            default_profile.insert("id".to_string(), "Default".to_string());
            default_profile
        }];
    }

    let chrome_state = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => {
            log::warn!("Could not read local state file: {}", err);
            return vec![{
                let mut default_profile = HashMap::new();
                default_profile.insert("name".to_string(), "Default".to_string());
                default_profile.insert("id".to_string(), "Default".to_string());
                default_profile
            }];
        }
    };
    let chrome_state: serde_json::Value = match serde_json::from_str(&chrome_state) {
        Ok(value) => value,
        Err(err) => {
            log::warn!("Invalid JSON in local state file: {}", err);
            return vec![{
                let mut default_profile = HashMap::new();
                default_profile.insert("name".to_string(), "Default".to_string());
                default_profile.insert("id".to_string(), "Default".to_string());
                default_profile
            }];
        }
    };
    let profiles = &chrome_state["profile"]["info_cache"];
    let mut result = Vec::new();

    if let serde_json::Value::Object(profiles) = profiles {
        for (key, val) in profiles {
            if let serde_json::Value::Object(val) = val {
                if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                    let mut profile = HashMap::new();
                    profile.insert("name".to_string(), name.to_string());
                    profile.insert("id".to_string(), key.clone());
                    result.push(profile);
                }
            }
        }
    }

    let _ = default_profile_path;
    result
}

fn where_clauses(table_title: &str, terms: &[&str]) -> String {
    terms
        .iter()
        .map(|&term| {
            format!(
                "({}.title LIKE '%{}%' OR {}.url LIKE '%{}%')",
                table_title, term, table_title, term
            )
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn get_history_query(table: &str, terms: &[&str], limit: i64, offset: i64) -> String {
    let where_clauses_string = where_clauses(table, terms);
    format!(
        "SELECT id, url, title, datetime(last_visit_time / 1000000 + (strftime('%s', '1601-01-01')), 'unixepoch', 'localtime') as last_visited \
        FROM {} {} ORDER BY last_visit_time DESC LIMIT {} OFFSET {};",
        table,
        if where_clauses_string.is_empty() { "".to_string() } else { format!("WHERE {}", where_clauses_string) },
        limit,
        offset
    )
}

fn get_firefox_history_query(query: Option<&str>, limit: i64, offset: i64) -> String {
    let terms: Vec<&str> = query.map_or(vec![], |q| q.trim().split_whitespace().collect());
    let where_clause = if terms.is_empty() {
        "".to_string()
    } else {
        format!("WHERE {}", where_clauses("moz_places", &terms))
    };
    format!(
        "
        SELECT
            id, url, title,
            datetime(last_visit_date/1000000, 'unixepoch') as last_visited
        FROM moz_places
        {}
        ORDER BY last_visit_date DESC LIMIT {} OFFSET {};
        ",
        where_clause, limit, offset
    )
}

pub fn search_browser_history(
    profile: &str,
    query: Option<&str>,
    limit: i64,
    offset: i64,
    default_profile_id: &str,
    default_profile_path: &[&str],
    backup_name: &str,
) -> HistoryResult {
    let terms: Vec<&str> = query.unwrap_or("").trim().split_whitespace().collect();
    let query = get_history_query("urls", &terms, limit, offset);
    let db_path = get_history_db_path(Some(profile), default_profile_id, default_profile_path);

    println!("db_path: {:?}", db_path);

    if !db_path.exists() {
        return HistoryResult {
            data: vec![],
            is_loading: false,
            error_view: Some("NotInstalledError".to_string()),
        };
    }

    if let Err(e) = fs::copy(&db_path, db_path.with_file_name(backup_name)) {
        return HistoryResult {
            data: vec![],
            is_loading: false,
            error_view: Some(e.to_string()),
        };
    }

    let db_path = db_path.with_file_name(backup_name);

    let conn = match Connection::open(&db_path) {
        Ok(conn) => conn,
        Err(err) => {
            return HistoryResult {
                data: vec![],
                is_loading: false,
                error_view: Some(err.to_string()),
            }
        }
    };

    let mut stmt = match conn.prepare(&query) {
        Ok(stmt) => stmt,
        Err(err) => {
            return HistoryResult {
                data: vec![],
                is_loading: false,
                error_view: Some(err.to_string()),
            }
        }
    };

    let history_iter = match stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    }) {
        Ok(iter) => iter,
        Err(err) => {
            return HistoryResult {
                data: vec![],
                is_loading: false,
                error_view: Some(err.to_string()),
            }
        }
    };

    let data: Vec<(i64, String, String, String)> = history_iter.filter_map(Result::ok).collect();

    HistoryResult {
        data,
        is_loading: false,
        error_view: None,
    }
}

fn open_connection_with_retries(
    db_path: &Path,
    retries: usize,
    delay: Duration,
) -> Result<Connection> {
    println!("db_path: {:?}", db_path);
    for _ in 0..retries {
        match Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            Ok(conn) => return Ok(conn),
            Err(rusqlite::Error::SqliteFailure(_, Some(ref msg)))
                if msg.contains("database is locked") =>
            {
                thread::sleep(delay);
            }
            Err(err) => return Err(err),
        }
    }
    Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}

fn use_firefox_history_search(query: Option<&str>, limit: i64, offset: i64) -> HistoryResult {
    let db_path = match get_firefox_history_db_path() {
        Some(path) => path,
        None => {
            return HistoryResult {
                data: vec![],
                is_loading: false,
                error_view: Some("NotInstalledError".to_string()),
            }
        }
    };

    if !db_path.exists() {
        return HistoryResult {
            data: vec![],
            is_loading: false,
            error_view: Some("NotInstalledError".to_string()),
        };
    }

    if let Err(e) = fs::copy(&db_path, db_path.with_file_name("history.sqlite")) {
        return HistoryResult {
            data: vec![],
            is_loading: false,
            error_view: Some(e.to_string()),
        };
    }

    let db_path = db_path.with_file_name("history.sqlite");
    let conn = match open_connection_with_retries(&db_path, 5, Duration::from_millis(100)) {
        Ok(conn) => conn,
        Err(err) => {
            return HistoryResult {
                data: vec![],
                is_loading: false,
                error_view: Some(err.to_string()),
            }
        }
    };

    let in_query = get_firefox_history_query(query, limit, offset);
    let mut stmt = match conn.prepare(&in_query) {
        Ok(stmt) => stmt,
        Err(err) => {
            return HistoryResult {
                data: vec![],
                is_loading: false,
                error_view: Some(err.to_string()),
            }
        }
    };

    let history_iter = match stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    }) {
        Ok(iter) => iter,
        Err(err) => {
            return HistoryResult {
                data: vec![],
                is_loading: false,
                error_view: Some(err.to_string()),
            }
        }
    };

    let data: Vec<(i64, String, String, String)> = history_iter.filter_map(Result::ok).collect();

    HistoryResult {
        data,
        is_loading: false,
        error_view: None,
    }
}

pub fn get_chrome_profiles() -> Vec<String> {
    let chrome_profiles = load_browser_profiles(&DEFAULT_CHROME_PROFILE_PATH, &DEFAULT_CHROME_STATE_PATH);
    println!("{:?}", chrome_profiles);
    chrome_profiles
        .iter()
        .filter_map(|profile| profile.get("name").cloned())
        .collect()
}

pub fn get_arc_profiles() -> Vec<String> {
    let arc_profiles = load_browser_profiles(&DEFAULT_ARC_PROFILE_PATH, &DEFAULT_ARC_STATE_PATH);
    println!("{:?}", arc_profiles);
    arc_profiles
        .iter()
        .filter_map(|profile| profile.get("name").cloned())
        .collect()
}

fn map_history_to_results(
    history_result: HistoryResult,
    source_domain: &str,
    file_type: &str,
) -> Vec<DocumentSearchResult> {
    history_result
        .data
        .iter()
        .map(|(_id, url, title, last_visited)| {
            let last_opened =
                chrono::NaiveDateTime::parse_from_str(last_visited, "%Y-%m-%d %H:%M:%S").unwrap_or_default();
            let mut last_opened = last_opened.and_utc().timestamp();
            if last_opened == -11644453800 {
                last_opened = 0;
            }
            DocumentSearchResult {
                id: 0,
                source_domain: source_domain.to_string(),
                created_at: 0,
                name: title.clone(),
                path: url.clone(),
                size: None,
                file_type: file_type.to_string(),
                last_modified: 0,
                last_opened,
                last_parsed: 0,
                last_synced: 0,
                frecency_last_accessed: 0,
                frecency_rank: 0.0,
                is_pinned: false,
                comment: None,
            }
        })
        .collect()
}

pub fn search_chrome(
    profile: String,
    user_query: String,
    limit: i64,
    page: i64,
) -> Result<Vec<DocumentSearchResult>, AppError> {
    let history_result = search_browser_history(
        profile.as_str(),
        Some(user_query.as_str()),
        limit,
        limit * page,
        DEFAULT_CHROME_PROFILE_ID,
        &DEFAULT_CHROME_PROFILE_PATH,
        "HistoryBackup",
    );
    Ok(map_history_to_results(history_result, "Chrome", "chrome-webpage"))
}

pub fn search_arc(
    profile: String,
    user_query: String,
    limit: i64,
    page: i64,
) -> Result<Vec<DocumentSearchResult>, AppError> {
    let history_result = search_browser_history(
        profile.as_str(),
        Some(user_query.as_str()),
        limit,
        limit * page,
        DEFAULT_ARC_PROFILE_ID,
        &DEFAULT_ARC_PROFILE_PATH,
        "HistoryBackup",
    );
    println!("{:?}", history_result);
    Ok(map_history_to_results(history_result, "Arc", "arc-webpage"))
}

pub fn search_firefox(
    user_query: String,
    limit: i64,
    page: i64,
) -> Result<Vec<DocumentSearchResult>, AppError> {
    let history_result = use_firefox_history_search(Some(user_query.as_str()), limit, limit * page);
    println!("history_result: {:?}", history_result);
    Ok(map_history_to_results(history_result, "Firefox", "firefox-webpage"))
}