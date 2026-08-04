use crate::custom_types::DBConnPoolState;
use crate::custom_types::Error;
use crate::housekeeping::{get_documents_directory, APP_DIRECTORY};
use crate::utils::norm;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool, PooledConnection};
use diesel::SqliteConnection;
use queries::{
    ALLOW_LIST_TABLE_CREATE_STATEMENT, APP_DATA_TABLE_CREATE_STATEMENT,
    BODY_TABLE_CREATE_STATEMENT, DOCUMENT_INDEXES, DOCUMENT_TABLE_CREATE_STATEMENT,
    FILE_TYPES_TABLE_CREATE_STATEMENT, IGNORE_LIST_TABLE_CREATE_STATEMENT,
    METADATA_FTS_VIRTUAL_TABLE_CREATE_STATEMENT, METADATA_TABLE_CREATE_STATEMENT,
    OCR_CACHE_TABLE_CREATE_STATEMENT, OCR_PAGE_CACHE_TABLE_CREATE_STATEMENT,
    TRIGGER_INSERT_DOCUMENT_METADATA, TRIGGER_UPDATE_DOCUMENT_METADATA,
    USER_PREFS_TABLE_ALTER_ADD_ENABLE_LOGS, USER_PREFS_TABLE_ALTER_ADD_MAX_OCR_PAGES,
    USER_PREFS_TABLE_ALTER_ADD_OCR_SORT_ORDER, USER_PREFS_TABLE_ALTER_ADD_OCR_THREADS,
    USER_PREFS_TABLE_CREATE_STATEMENT,
};
use std::sync::Mutex;
use tauri::Manager;

const DB_NAME: &str = r#"buzee.db"#;

// pub mod crud;
pub mod models;
mod queries;
pub mod schema;
pub mod search;
// mod response_models;

fn get_db_url() -> Result<String, Error> {
    let app_dir = get_documents_directory()
        .ok_or_else(|| Error::new("Could not resolve the documents directory"))?;
    log::info!("app_dir: {}", app_dir);
    let database_path = format!("{}/{}/{}", app_dir, APP_DIRECTORY, DB_NAME);
    let database_path = norm(&database_path);
    let database_url: String;
    #[cfg(target_os = "windows")]
    {
        database_url = format!("sqlite:///{}", database_path);
    }
    #[cfg(target_os = "macos")]
    {
        database_url = format!("sqlite://{}", database_path);
    }
    Ok(database_url)
}

// Per-connection SQLite settings. `journal_mode=WAL` is intentionally excluded:
// it is set once at startup in housekeeping::initialize() and persists at the
// database level (and it cannot be changed once a connection has entered a
// transaction).
const CONNECTION_PRAGMAS: [&str; 5] = [
    "PRAGMA foreign_keys = ON;",
    "PRAGMA busy_timeout = 5000;",
    "PRAGMA synchronous = NORMAL;",
    "PRAGMA cache_size = -64000;",
    "PRAGMA auto_vacuum = FULL;",
];

fn apply_connection_pragmas(conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
    for pragma in CONNECTION_PRAGMAS {
        diesel::sql_query(pragma).execute(conn)?;
    }
    Ok(())
}

/// Applies the per-connection PRAGMAs once, when a new connection is created
/// by the pool — not on every checkout from the pool.
#[derive(Debug)]
struct SqlitePragmaCustomizer;

impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for SqlitePragmaCustomizer {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        apply_connection_pragmas(conn)
    }
}

pub fn get_connection_pool() -> Result<Pool<ConnectionManager<SqliteConnection>>, Error> {
    let database_url = get_db_url()?;
    log::info!("Creating connection pool for db at: {}", &database_url);
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder()
        .test_on_check_out(true)
        .max_size(10)
        .connection_customizer(Box::new(SqlitePragmaCustomizer))
        .build(manager)
        .map_err(|e| Error::new(&e.to_string()))
}

// How many times to retry a pooled connection checkout before giving up. Pool
// outages (e.g. a momentarily locked SQLite database) are transient, so a
// single failure must not panic the app.
const MAX_CONNECTION_ATTEMPTS: u32 = 5;

pub fn establish_connection(
    app: &tauri::AppHandle,
) -> PooledConnection<ConnectionManager<SqliteConnection>> {
    let state_mutex = app.state::<Mutex<DBConnPoolState>>();
    let state = state_mutex.lock().unwrap();
    let pool = &state.conn_pool;

    let mut attempt = 0;
    loop {
        attempt += 1;
        match pool.get() {
            Ok(connection) => return connection,
            Err(e) => {
                log::error!(
                    "Could not get DB connection from pool (attempt {} of {}): {}",
                    attempt,
                    MAX_CONNECTION_ATTEMPTS,
                    e
                );
                if attempt >= MAX_CONNECTION_ATTEMPTS {
                    panic!(
                        "Could not get a DB connection from the pool after {} attempts: {}",
                        attempt, e
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(100 * attempt as u64));
            }
        }
    }
}

pub fn establish_direct_connection_to_db() -> Result<SqliteConnection, Error> {
    let database_url = get_db_url()?;
    log::info!("Creating direct connection to db at: {}", &database_url);
    let mut connection =
        SqliteConnection::establish(&database_url).map_err(|e| Error::new(&e.to_string()))?;

    apply_connection_pragmas(&mut connection).map_err(|e| Error::new(&e.to_string()))?;

    Ok(connection)
}

// Create all tables and triggers in the db if they don't exist
pub fn create_tables_if_not_exists(
    conn: &mut SqliteConnection,
) -> Result<usize, diesel::result::Error> {
    // User Prefs and App Data Tables
    diesel::sql_query(USER_PREFS_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;
    // Best-effort migrations for pre-existing databases (no-op if the columns already exist)
    let _ = diesel::sql_query(USER_PREFS_TABLE_ALTER_ADD_ENABLE_LOGS.to_string()).execute(conn);
    let _ = diesel::sql_query(USER_PREFS_TABLE_ALTER_ADD_MAX_OCR_PAGES.to_string()).execute(conn);
    let _ = diesel::sql_query(USER_PREFS_TABLE_ALTER_ADD_OCR_THREADS.to_string()).execute(conn);
    let _ = diesel::sql_query(USER_PREFS_TABLE_ALTER_ADD_OCR_SORT_ORDER.to_string()).execute(conn);
    diesel::sql_query(APP_DATA_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;
    diesel::sql_query(IGNORE_LIST_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;
    diesel::sql_query(ALLOW_LIST_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;
    diesel::sql_query(FILE_TYPES_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;
    diesel::sql_query(OCR_CACHE_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;
    diesel::sql_query(OCR_PAGE_CACHE_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;

    // Data Tables
    diesel::sql_query(DOCUMENT_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;
    diesel::sql_query(BODY_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;
    diesel::sql_query(METADATA_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;
    diesel::sql_query(METADATA_FTS_VIRTUAL_TABLE_CREATE_STATEMENT.to_string()).execute(conn)?;

    // Indexes on document table for fast lookups
    diesel::sql_query(DOCUMENT_INDEXES.to_string()).execute(conn)?;

    // Triggers
    diesel::sql_query(TRIGGER_INSERT_DOCUMENT_METADATA.to_string()).execute(conn)?;
    diesel::sql_query(TRIGGER_UPDATE_DOCUMENT_METADATA.to_string()).execute(conn)?;
    Ok(1)
}

// use diesel::sql_query;
// #[derive(QueryableByName, Debug)]
// struct TableInfo {
//     #[sql_type = "diesel::sql_types::Text"]
//     name: String
// }

// fn get_table_columns(conn: &mut SqliteConnection, table_name: &str) -> Result<Vec<String>, diesel::result::Error> {
//     let query = format!("SELECT name FROM pragma_table_info('{}')", table_name);
//     let results = sql_query(query).load::<TableInfo>(conn)?;

//     let mut columns = Vec::new();
//     for result in results {
//         columns.push(result.name);
//     }
//     Ok(columns)
// }
