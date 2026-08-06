use pdf_extract::extract_text;
use std::error::Error;
use std::path::Path;

use crate::domain::types::{DbPool, UserPreferencesState};
use crate::infrastructure::text_extraction::win_ocr::{should_fallback_to_ocr, OCR_FALLBACK_MIN_CHARS};
use std::sync::{Arc, RwLock};

#[cfg(feature = "ocr")]
use crate::infrastructure::housekeeping::get_app_directory;
#[cfg(all(target_os = "macos", feature = "ocr"))]
use crate::infrastructure::text_extraction::txt;
#[cfg(all(target_os = "windows", feature = "ocr"))]
use crate::infrastructure::user_prefs::get_pdf_max_ocr_pages;

const PDF_OCR_TIMEOUT_SECS: u64 = 120;

/// Locate the OCR sidecar binary bundled next to the executable, or fall back to
/// resolving it from PATH.
#[cfg(all(target_os = "macos", feature = "ocr"))]
fn textra_binary() -> std::process::Command {
    let mut command = std::process::Command::new("textra");
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sidecar = dir.join("textra");
            if sidecar.exists() {
                command = std::process::Command::new(sidecar);
            }
        }
    }
    command
}

pub async fn extract(
    file: &String,
    pool: &DbPool,
    preferences: &Arc<RwLock<UserPreferencesState>>,
) -> Result<String, Box<dyn Error>> {
    log::info!("Extracting text from: {}", file);
    let text_based_content = if file.to_lowercase().contains(".pdf") {
        text_based_extraction(file).unwrap_or_default()
    } else {
        String::new()
    };

    if !should_fallback_to_ocr(&text_based_content, OCR_FALLBACK_MIN_CHARS) {
        return Ok(text_based_content);
    }

    #[cfg(feature = "ocr")]
    {
        log::info!("Running OCR based text extraction");
        let app_directory = get_app_directory();

        #[cfg(target_os = "macos")]
        {
            let output_path = format!("{}/temp_output.txt", app_directory);
            let mut sidecar_command = textra_binary();
            let child = sidecar_command
                .args([file, "-o", output_path.as_str()])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn();
            match child {
                Ok(mut child) => {
                    let status = child.wait();
                    if status.is_err() {
                        return Err(Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "Failed to wait for textra sidecar",
                        )));
                    }
                }
                Err(e) => {
                    log::error!("Failed to spawn textra sidecar: {}", e);
                    return Err(Box::new(e));
                }
            }

            let temp_file_path = format!("{}/temp_output.txt", app_directory);
            let text = txt::extract(&temp_file_path)?;
            return Ok(text);
        }

        #[cfg(target_os = "windows")]
        {
            let _ = app_directory;
            use crate::infrastructure::database::establish_connection;
            use crate::infrastructure::text_extraction::ocr_cache;
            use crate::infrastructure::text_extraction::win_ocr::{self, ImageOcr};

            let mut conn = establish_connection(pool);
            let max_pages = get_pdf_max_ocr_pages(preferences) as u32;

            let file_hash =
                ocr_cache::compute_file_hash(std::path::Path::new(file)).unwrap_or_default();
            if let Some(cached) = ocr_cache::get_cached_ocr_pdf(&file_hash, &mut conn, max_pages) {
                return Ok(cached);
            }

            let cached_pages = ocr_cache::get_cached_pdf_pages(file, &mut conn);

            let path_buf = std::path::PathBuf::from(file);
            let (pages, pages_attempted) = tokio::time::timeout(
                std::time::Duration::from_secs(PDF_OCR_TIMEOUT_SECS),
                tokio::task::spawn_blocking(move || {
                    let ocr_engine = win_ocr::WindowsOcr;
                    ocr_engine.recognize_pdf(&path_buf, None, max_pages, &cached_pages)
                }),
            )
            .await
            .map_err(|_elapsed| {
                log::error!(
                    "PDF OCR timed out after {}s: {}",
                    PDF_OCR_TIMEOUT_SECS,
                    file
                );
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "PDF OCR timed out",
                )) as Box<dyn Error>
            })?
            .map_err(|error| Box::new(error) as Box<dyn Error>)??;

            let text = pages
                .iter()
                .map(|page| page.text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            let text = win_ocr::normalize_ocr_text(&text);

            if text.trim().is_empty() {
                return Err("OcrUnavailableForPdf".into());
            }

            ocr_cache::store_cached_pdf_pages(file, &pages, pages_attempted, &mut conn);
            if pages.len() as u32 == pages_attempted {
                ocr_cache::store_ocr_result(&file_hash, &text, max_pages as i32, None, &mut conn);
            }

            return Ok(text);
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = app_directory;
            Err("OCR sidecars are not supported on this platform".into())
        }
    }

    #[cfg(not(feature = "ocr"))]
    {
        Err("OCR is disabled in this build; the PDF has no extractable text layer".into())
    }
}

use std::panic::{catch_unwind, AssertUnwindSafe};

pub fn text_based_extraction(file: &String) -> Result<String, Box<dyn Error>> {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(AssertUnwindSafe(|| extract_text(Path::new(file))));
    std::panic::set_hook(previous_hook);

    match result {
        Ok(Ok(content)) => Ok(content),
        Ok(Err(error)) => Err(error.into()),
        Err(_) => {
            log::warn!(
                "pdf-extract panicked while extracting text from {}; falling back to OCR",
                file
            );
            Err("pdf text extraction panicked".into())
        }
    }
}