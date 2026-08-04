// Extract text from a .epub file
use epub::doc::EpubDoc;
use std::error::Error;

pub fn extract(file: &String, _app: &tauri::AppHandle) -> Result<String, Box<dyn Error>> {
    let doc = EpubDoc::new(file);
    let mut doc = doc.unwrap();
    let mut text = String::new();
    while doc.go_next() {
        let current = doc.get_current_str();
        match current {
            Some((v, _m)) => {
                text.push_str(&v);
            }
            None => println!("Could not get value"),
        }
    }
    Ok(text)
}
