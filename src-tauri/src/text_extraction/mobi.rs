// Extract text from a .epub file
use mobi::Mobi;
use std::error::Error;

pub fn extract(file: &String, _app: &tauri::AppHandle) -> Result<String, Box<dyn Error>> {
    let mobi_file = Mobi::from_path(file)?;
    let text = mobi_file.content_as_string()?;
    Ok(text)
}
