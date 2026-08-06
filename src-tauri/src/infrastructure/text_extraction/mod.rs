use crate::domain::types::DbPool;
use crate::domain::types::UserPreferencesState;
use std::error::Error;
use std::sync::{Arc, RwLock};

pub mod csv;
pub mod docx;
pub mod epub;
pub mod image;
pub mod mobi;
#[cfg(all(target_os = "windows", feature = "ocr"))]
pub mod ocr_cache;
pub mod pdf;
pub mod pptx;
pub mod txt;
pub mod win_ocr;
pub mod xlsx;

pub struct Extractor;

impl Extractor {
    pub fn new() -> Self {
        Extractor
    }

    pub async fn extract_text_from_file(
        &self,
        file_path: String,
        file_type: String,
        pool: &DbPool,
        preferences: &Arc<RwLock<UserPreferencesState>>,
    ) -> Result<String, Box<dyn Error>> {
        match file_type.as_str() {
            "csv" => csv::extract(&file_path),
            "docx" => docx::extract(&file_path),
            "epub" => epub::extract(&file_path),
            "mobi" => mobi::extract(&file_path),
            "md" => txt::extract(&file_path),
            "pdf" => pdf::extract(&file_path, pool, preferences).await,
            "pptx" => pptx::extract(&file_path),
            "txt" => txt::extract(&file_path),
            "xlsx" => xlsx::extract(&file_path),
            "jpg" => image::extract(&file_path, pool, preferences).await,
            "jpeg" => image::extract(&file_path, pool, preferences).await,
            "png" => image::extract(&file_path, pool, preferences).await,
            "svg" => image::extract(&file_path, pool, preferences).await,
            _ => Err("File type not supported".into()),
        }
    }
}