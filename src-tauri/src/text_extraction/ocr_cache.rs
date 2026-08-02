use std::io::Read;
use std::path::Path;

use diesel::prelude::*;

use crate::text_extraction::win_ocr::{CachedPdfPage, PdfPageOcr};

/// Returns cached OCR text for a file whose identity matches `file_hash`.
/// Returns `None` when there is no cache hit.
pub fn get_cached_ocr(file_hash: &str, conn: &mut diesel::SqliteConnection) -> Option<String> {
    diesel::sql_query(
        "SELECT text FROM ocr_cache WHERE file_hash = ?1".to_string(),
    )
    .bind::<diesel::sql_types::Text, _>(file_hash)
    .get_result::<OcrCacheRow>(conn)
    .ok()
    .map(|row| row.text)
}

#[derive(QueryableByName)]
struct OcrCacheRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    text: String,
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

/// Replaces the per-page OCR cache rows for a document path with `pages`.
pub fn store_cached_pdf_pages(
    file_path: &str,
    pages: &[PdfPageOcr],
    conn: &mut diesel::SqliteConnection,
) {
    // Remove stale rows (e.g. page count shrank) then insert the fresh ones.
    diesel::sql_query("DELETE FROM ocr_page_cache WHERE file_path = ?1".to_string())
        .bind::<diesel::sql_types::Text, _>(file_path)
        .execute(conn)
        .ok();

    let timestamp = chrono::Utc::now().timestamp();
    for page in pages {
        let _ = diesel::sql_query(
            "INSERT INTO ocr_page_cache (file_path, page_index, page_raster_hash, page_text, created_at) VALUES (?1, ?2, ?3, ?4, ?5)"
                .to_string(),
        )
        .bind::<diesel::sql_types::Text, _>(file_path)
        .bind::<diesel::sql_types::Integer, _>(page.index as i32)
        .bind::<diesel::sql_types::Text, _>(&page.raster_hash)
        .bind::<diesel::sql_types::Text, _>(&page.text)
        .bind::<diesel::sql_types::BigInt, _>(timestamp)
        .execute(conn);
    }
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
