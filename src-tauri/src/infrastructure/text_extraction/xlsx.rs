use dotext::*;
use std::error::Error;
use std::io::Read;

pub fn extract(file: &String) -> Result<String, Box<dyn Error>> {
    let mut file_buffer = Xlsx::open(file)?;
    let mut text = String::new();
    file_buffer.read_to_string(&mut text)?;
    text = text.chars().filter(|c| !c.is_numeric()).collect();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut unique_words = Vec::new();
    for word in words {
        if !unique_words.contains(&word) {
            unique_words.push(word);
        }
    }
    Ok(text)
}