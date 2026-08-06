use diesel::prelude::*;
use diesel::sql_types::{BigInt, Double, Text};

use crate::domain::types::{DashboardBuckets, DashboardStats, UserPreferencesState, DbPool};
use crate::infrastructure::database::establish_connection;
use crate::infrastructure::database::models::DocumentSearchResult;
use crate::infrastructure::database::schema::document;
use crate::infrastructure::statistics::{database_file_size, seconds_until_next_scan};

#[derive(QueryableByName, Debug)]
struct Scalar {
    #[diesel(sql_type = Double)]
    val: f64,
}

#[derive(QueryableByName, Debug)]
struct BucketCount {
    #[diesel(sql_type = Text)]
    label: String,
    #[diesel(sql_type = BigInt)]
    n: i64,
    #[diesel(sql_type = Double)]
    size_bytes: f64,
}

#[derive(QueryableByName, Debug)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    n: i64,
}

fn load_scalar(conn: &mut SqliteConnection, sql: &str) -> f64 {
    diesel::sql_query(sql)
        .load::<Scalar>(conn)
        .map(|rows| rows.first().map(|r| r.val).unwrap_or(0.0))
        .unwrap_or(0.0)
}

fn count_where(conn: &mut SqliteConnection, where_clause: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) AS n FROM document WHERE {}", where_clause);
    diesel::sql_query(sql)
        .load::<CountRow>(conn)
        .map(|rows| rows.first().map(|r| r.n).unwrap_or(0))
        .unwrap_or(0)
}

fn filetype_buckets(conn: &mut SqliteConnection) -> Vec<(String, i64, f64)> {
    diesel::sql_query(
        "SELECT file_type AS label, COUNT(*) AS n, COALESCE(SUM(size), 0.0) AS size_bytes \
     FROM document GROUP BY file_type ORDER BY n DESC",
    )
    .load::<BucketCount>(conn)
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.label, r.n, r.size_bytes))
    .collect()
}

fn top_largest(conn: &mut SqliteConnection, limit: i64) -> Vec<DocumentSearchResult> {
    use crate::infrastructure::database::schema::document::file_type;
    document::table
        .order(document::size.desc())
        .filter(file_type.ne("folder"))
        .limit(limit)
        .load::<DocumentSearchResult>(conn)
        .unwrap_or_default()
}

fn top_recent(conn: &mut SqliteConnection, limit: i64) -> Vec<DocumentSearchResult> {
    document::table
        .order(document::last_modified.desc())
        .limit(limit)
        .load::<DocumentSearchResult>(conn)
        .unwrap_or_default()
}

pub fn get_dashboard_stats(
    pool: &DbPool,
    sync_running: bool,
    last_sync_time: i64,
    preferences: &UserPreferencesState,
) -> DashboardStats {
    let auto_sync = preferences.automatic_background_sync;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut conn = establish_connection(pool);

    let total_files = count_where(&mut conn, "file_type != 'folder'");
    let total_folders = count_where(&mut conn, "file_type = 'folder'");
    let total_size = load_scalar(
        &mut conn,
        "SELECT COALESCE(SUM(size), 0.0) AS val FROM document",
    );
    let average_size = load_scalar(
        &mut conn,
        "SELECT COALESCE(AVG(size), 0.0) AS val FROM document WHERE size IS NOT NULL",
    );
    let largest_size = load_scalar(
        &mut conn,
        "SELECT COALESCE(MAX(size), 0.0) AS val FROM document WHERE file_type != 'folder'",
    );

    let parsed = count_where(&mut conn, "last_parsed > 0 AND file_type != 'folder'");
    let parsed_total = load_scalar(
        &mut conn,
        "SELECT COALESCE(SUM(size), 0.0) AS val FROM document WHERE last_parsed > 0 AND file_type != 'folder'",
    );
    let unparsed = (total_files - parsed).max(0);
    let pinned = count_where(&mut conn, "is_pinned = 1");
    let most_frequent = count_where(&mut conn, "frecency_rank > 0");

    let buckets = filetype_buckets(&mut conn);

    let top_largest = top_largest(&mut conn, 10);
    let top_recent = top_recent(&mut conn, 10);

    DashboardStats {
        total_files,
        total_folders,
        total_size_bytes: total_size,
        average_size_bytes: average_size,
        largest_file_size_bytes: largest_size,
        parsed_files: parsed,
        parsed_total_size_bytes: parsed_total,
        unparsed_files: unparsed,
        pinned_files: pinned,
        most_frequent_count: most_frequent,
        database_size_bytes: database_file_size(),
        last_scan_time: last_sync_time,
        next_scan_in_seconds: if auto_sync {
            seconds_until_next_scan(last_sync_time, now)
        } else {
            -1
        },
        auto_sync_enabled: auto_sync,
        scan_running: sync_running,
        filetype_counts: buckets
            .into_iter()
            .map(|(t, n, s)| DashboardBuckets {
                file_type: t,
                count: n,
                size_bytes: s,
            })
            .collect(),
        top_largest,
        top_recent,
    }
}