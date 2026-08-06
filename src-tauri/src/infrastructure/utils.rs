use crate::domain::AppError;
use crate::domain::types::UserPreferencesState;
use crate::domain::DbPool;
use crate::infrastructure::database::search::{get_file_id_from_path, get_parsed_text_for_file};
use crate::infrastructure::indexing::extract_text_from_path;
use diesel::SqliteConnection;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::{fs, io, io::Write};

pub fn get_metadata(path: &Path) -> io::Result<fs::Metadata> {
    let metadata = fs::metadata(path)?;
    Ok(metadata)
}

pub fn norm(path: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        str::replace(path, "/", "\\")
    }

    #[cfg(target_os = "macos")]
    {
        str::replace(path, "\\", "/")
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        path.to_string()
    }
}

pub async fn extract_text_from_pdf(
    file_path: String,
    conn: &mut SqliteConnection,
    pool: &DbPool,
    preferences: &Arc<RwLock<UserPreferencesState>>,
) -> Result<Vec<String>, AppError> {
    let mut text = vec![];
    let file_id = get_file_id_from_path(&file_path, conn).unwrap_or(0);
    if file_id > 0 {
        text = get_parsed_text_for_file(file_id, conn).unwrap_or_default();
    }

    if text.is_empty() {
        let extracted_text =
            extract_text_from_path(file_path, "pdf".to_string(), pool, preferences).await;
        text = extracted_text.split("\n").map(|s| s.to_string()).collect();
    }
    Ok(text)
}

pub async fn save_text_to_file(file_path: String, text: String) {
    if let Err(e) = fs::File::create(&file_path).and_then(|mut f| f.write_all(text.as_bytes())) {
        log::error!("Failed to save text to {}: {}", file_path, e);
    }
}

pub async fn read_text_from_file(file_path: String) -> Result<String, AppError> {
    Ok(fs::read_to_string(file_path)?)
}

pub async fn read_image_to_base64(file_path: String) -> Result<String, AppError> {
    use base64::prelude::*;
    let image = fs::read(file_path)?;
    let base64_image = BASE64_STANDARD.encode(&image);
    Ok(base64_image)
}