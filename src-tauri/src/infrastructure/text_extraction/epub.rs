use epub::doc::EpubDoc;
use std::error::Error;

pub fn extract(file: &String) -> Result<String, Box<dyn Error>> {
    let doc = match EpubDoc::new(file) {
        Ok(doc) => doc,
        Err(err) => {
            log::warn!("Could not open epub {:?}: {}", file, err);
            return Ok(String::new());
        }
    };
    let mut doc = doc;
    let mut text = String::new();
    while doc.go_next() {
        let current = doc.get_current_str();
        match current {
            Some((v, _m)) => {
                text.push_str(&v);
            }
            None => log::info!("Could not get value"),
        }
    }
    Ok(text)
}