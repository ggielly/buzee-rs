use std::{error::Error, fs::File, io::BufReader};

use crate::domain::types::{DbPool, UserPreferencesState};
use crate::infrastructure::text_extraction::win_ocr::{should_fallback_to_ocr, OCR_FALLBACK_MIN_CHARS};
use std::sync::{Arc, RwLock};

#[cfg(all(target_os = "macos", feature = "ocr"))]
use crate::infrastructure::housekeeping::get_app_directory;
#[cfg(all(target_os = "macos", feature = "ocr"))]
use crate::infrastructure::text_extraction::txt;

#[cfg(all(target_os = "windows", feature = "ocr"))]
const IMAGE_OCR_TIMEOUT_SECS: u64 = 60;

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
    let mut text_based_content = String::new();

    if file.to_lowercase().contains(".svg") {
        text_based_content = extract_text_from_svg(file).unwrap_or_else(|error| {
            log::error!("Failed to extract text from SVG {}: {}", file, error);
            String::new()
        });
    }

    if !should_fallback_to_ocr(&text_based_content, OCR_FALLBACK_MIN_CHARS) {
        return Ok(text_based_content);
    }

    #[cfg(feature = "ocr")]
    {
        #[cfg(target_os = "windows")]
        {
            use crate::infrastructure::database::establish_connection;
            use crate::infrastructure::text_extraction::ocr_cache;
            use crate::infrastructure::text_extraction::win_ocr::{self, ImageOcr, OcrError, WindowsOcr};

            let path = std::path::PathBuf::from(file);
            let extension = win_ocr::image_extension(&path).unwrap_or_default();
            if !win_ocr::is_supported_image(&extension) {
                log::info!("Image format not supported by Windows OCR: {}", file);
                return Err(Box::new(OcrError::UnsupportedFormat));
            }

            let mut conn = establish_connection(pool);

            let file_hash = ocr_cache::compute_file_hash(&path).unwrap_or_default();
            if let Some(cached) = ocr_cache::get_cached_ocr(&file_hash, &mut conn) {
                return Ok(cached);
            }

            let ocr = WindowsOcr;
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(IMAGE_OCR_TIMEOUT_SECS),
                tokio::task::spawn_blocking(move || ocr.recognize_image(&path, None)),
            )
            .await
            .map_err(|_elapsed| {
                log::error!(
                    "Image OCR timed out after {}s: {}",
                    IMAGE_OCR_TIMEOUT_SECS,
                    file
                );
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Image OCR timed out",
                )) as Box<dyn Error>
            })?
            .map_err(|error| {
                log::error!("OCR task panicked for {}: {}", file, error);
                Box::new(error) as Box<dyn Error>
            })??;
            if result.text.trim().is_empty() {
                log::info!("OCR produced no text for image: {}", file);
            }

            ocr_cache::store_ocr_result(
                &file_hash,
                &result.text,
                result.lines_detected as i32,
                result.language_tag.as_deref(),
                &mut conn,
            );

            return Ok(result.text);
        }

        #[cfg(target_os = "macos")]
        {
            let app_directory = get_app_directory();
            let output_path = format!("{}/temp_output.txt", app_directory);

            let mut sidecar_command = textra_binary();
            let child = sidecar_command
                .args([file, "-o", output_path.as_str()])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn();
            match child {
                Ok(mut child) => {
                    if child.wait().is_err() {
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

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err("OCR sidecars are not supported on this platform".into())
        }
    }

    #[cfg(not(feature = "ocr"))]
    {
        Err("OCR is disabled in this build; the image has no extractable text".into())
    }
}

fn extract_text_from_svg(file_path: &String) -> Result<String, Box<dyn Error>> {
    use xml::reader::{EventReader, XmlEvent};
    let file = File::open(file_path)?;
    let file = BufReader::new(file);

    let parser = EventReader::new(file);

    let mut inside_text = false;
    let mut extracted_text = String::new();

    for event in parser {
        match event {
            Ok(XmlEvent::StartElement { name, .. }) => {
                if name.local_name == "text" {
                    inside_text = true;
                }
            }
            Ok(XmlEvent::Characters(data)) => {
                if inside_text {
                    extracted_text.push_str(&data);
                }
            }
            Ok(XmlEvent::EndElement { name }) => {
                if name.local_name == "text" {
                    inside_text = false;
                }
            }
            Err(e) => {
                log::error!("Error parsing SVG {}: {}", file_path, e);
                break;
            }
            _ => {}
        }
    }

    Ok(extracted_text)
}