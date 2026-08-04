use std::io::Read;
use std::path::Path;

use diesel::connection::Connection;
use diesel::prelude::*;

use crate::text_extraction::win_ocr::{CachedPdfPage, PdfPageOcr};

/// Returns cached OCR text for a file whose identity matches `file_hash`.
/// Returns `None` when there is no cache hit.
pub fn get_cached_ocr(file_hash: &str, conn: &mut diesel::SqliteConnection) -> Option<String> {
    get_cached_ocr_row(file_hash, conn).map(|row| row.text)
}

/// Whole-file cache lookup for scanned PDFs. The cached entry is only trusted
/// when it was produced by a run that applied at least `required_pages` as its
/// OCR cap (see `store_ocr_result` for PDFs, which stores the applied cap in
/// `page_count`). This prevents a result that predates a raise of the
/// `pdf_max_ocr_pages` limit from being served as if it were complete.
pub fn get_cached_ocr_pdf(
    file_hash: &str,
    conn: &mut diesel::SqliteConnection,
    required_pages: u32,
) -> Option<String> {
    get_cached_ocr_row(file_hash, conn)
        .filter(|row| row.page_count as u32 >= required_pages)
        .map(|row| row.text)
}

fn get_cached_ocr_row(file_hash: &str, conn: &mut diesel::SqliteConnection) -> Option<OcrCacheRow> {
    diesel::sql_query("SELECT text, page_count FROM ocr_cache WHERE file_hash = ?1".to_string())
        .bind::<diesel::sql_types::Text, _>(file_hash)
        .get_result::<OcrCacheRow>(conn)
        .ok()
}

#[derive(QueryableByName)]
struct OcrCacheRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    text: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    page_count: i32,
}

/// Returns the previously stored per-page OCR entries for a document path.
pub fn get_cached_pdf_pages(
    file_path: &str,
    conn: &mut diesel::SqliteConnection,
) -> Vec<CachedPdfPage> {
    diesel::sql_query(
        "SELECT page_index, page_raster_hash, page_text FROM ocr_page_cache WHERE file_path = ?1"
            .to_string(),
    )
    .bind::<diesel::sql_types::Text, _>(file_path)
    .load::<OcrPageRow>(conn)
    .unwrap_or_default()
    .into_iter()
    .map(|row| CachedPdfPage {
        index: row.page_index as u32,
        raster_hash: row.page_raster_hash,
        text: row.page_text,
    })
    .collect()
}

#[derive(QueryableByName)]
struct OcrPageRow {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    page_index: i32,
    #[diesel(sql_type = diesel::sql_types::Text)]
    page_raster_hash: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    page_text: String,
}

/// Merges the per-page OCR cache rows for a document path with `pages`.
///
/// Pages that succeeded this run are upserted; rows whose index is at or past
/// `pages_attempted` (the document shrank or the OCR cap was lowered) are
/// dropped; rows for pages that failed this run are preserved so their
/// previously recognized text stays cached for the next scan. All writes run
/// in a single transaction so an interruption never leaves a half-updated
/// cache.
pub fn store_cached_pdf_pages(
    file_path: &str,
    pages: &[PdfPageOcr],
    pages_attempted: u32,
    conn: &mut diesel::SqliteConnection,
) {
    let timestamp = chrono::Utc::now().timestamp();

    let _ = conn.transaction::<_, diesel::result::Error, _>(|conn| {
        // Drop rows no longer part of the attempted range.
        diesel::sql_query("DELETE FROM ocr_page_cache WHERE file_path = ?1 AND page_index >= ?2")
            .bind::<diesel::sql_types::Text, _>(file_path)
            .bind::<diesel::sql_types::Integer, _>(pages_attempted as i32)
            .execute(conn)?;

        for page in pages {
            diesel::sql_query(
                "INSERT OR REPLACE INTO ocr_page_cache (file_path, page_index, page_raster_hash, page_text, created_at) VALUES (?1, ?2, ?3, ?4, ?5)"
                    .to_string(),
            )
            .bind::<diesel::sql_types::Text, _>(file_path)
            .bind::<diesel::sql_types::Integer, _>(page.index as i32)
            .bind::<diesel::sql_types::Text, _>(&page.raster_hash)
            .bind::<diesel::sql_types::Text, _>(&page.text)
            .bind::<diesel::sql_types::BigInt, _>(timestamp)
            .execute(conn)?;
        }
        Ok(())
    });
}

/// Stores a successful OCR result in the cache.
pub fn store_ocr_result(
    file_hash: &str,
    text: &str,
    page_count: i32,
    language: Option<&str>,
    conn: &mut diesel::SqliteConnection,
) {
    let _ = diesel::sql_query(
        "INSERT OR REPLACE INTO ocr_cache (file_hash, text, page_count, language_tag, created_at) VALUES (?1, ?2, ?3, ?4, ?5)"
            .to_string(),
    )
    .bind::<diesel::sql_types::Text, _>(file_hash)
    .bind::<diesel::sql_types::Text, _>(text)
    .bind::<diesel::sql_types::Integer, _>(page_count)
    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(language)
    .bind::<diesel::sql_types::BigInt, _>(chrono::Utc::now().timestamp())
    .execute(conn);
}

/// Removes stale OCR cache rows: per-page entries whose file no longer exists
/// on disk, and whole-file entries older than `max_age_days`. Called on startup
/// to keep the OCR cache from growing without bound.
pub fn prune_ocr_caches(conn: &mut diesel::SqliteConnection, max_age_days: i64) {
    // Per-page rows keyed by file path: drop those whose file is gone.
    let stale_paths: Vec<String> =
        diesel::sql_query("SELECT DISTINCT file_path FROM ocr_page_cache".to_string())
            .load::<OcrPathRow>(conn)
            .unwrap_or_default()
            .into_iter()
            .map(|row| row.file_path)
            .filter(|path| !std::path::Path::new(path).exists())
            .collect();

    for path in stale_paths {
        let _ = diesel::sql_query("DELETE FROM ocr_page_cache WHERE file_path = ?1")
            .bind::<diesel::sql_types::Text, _>(path)
            .execute(conn);
    }

    // Whole-file entries keyed by content hash: drop entries that are too old.
    let cutoff = chrono::Utc::now().timestamp() - max_age_days * 86_400;
    let _ = diesel::sql_query("DELETE FROM ocr_cache WHERE created_at < ?1")
        .bind::<diesel::sql_types::BigInt, _>(cutoff)
        .execute(conn);
}

#[derive(QueryableByName)]
struct OcrPathRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    file_path: String,
}

/// Computes a fast identity hash for a file: SHA-256 of the first 64 KB of
/// content + file size + modification timestamp.  Fast enough to run on every
/// file without noticeable overhead.
pub fn compute_file_hash(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};

    let meta = std::fs::metadata(path)?;
    let mut hasher = Sha256::new();

    // Hash the first 64 KB of content : enough to distinguish documents
    // without reading the entire file.
    if let Ok(mut file) = std::fs::File::open(path) {
        let mut buf = [0u8; 65536];
        let n = file.read(&mut buf).unwrap_or(0);
        hasher.update(&buf[..n]);
    }

    hasher.update(meta.len().to_le_bytes());
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    hasher.update(mtime.to_le_bytes());

    Ok(format!("{:x}", hasher.finalize()))
}
