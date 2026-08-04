use dotext::*;
use std::error::Error;
use std::io::Read;

pub fn extract(file: &String, _app: &tauri::AppHandle) -> Result<String, Box<dyn Error>> {
    let mut file_buffer = Pptx::open(file)?;
    let mut text = String::new();
    file_buffer.read_to_string(&mut text)?;
    Ok(text)
}
