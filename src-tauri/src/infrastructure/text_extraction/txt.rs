use std::error::Error;
use std::fs::{metadata, read_to_string, File};
use std::io::{prelude::*, BufReader};

pub fn extract(filepath: &String) -> Result<String, Box<dyn Error>> {
    let filesize = metadata(filepath.clone())?.len();
    let text: String;
    if filesize > 1000000 {
        text = extract_large_file(&filepath)?;
    } else {
        let file_contents = read_to_string(filepath)?;
        let lines: Vec<String> = file_contents.lines().map(|line| line.to_string()).collect();
        text = lines.join("\n\n");
    }
    Ok(text)
}

pub fn extract_large_file(filepath: &String) -> Result<String, Box<dyn Error>> {
    let file_buffer = File::open(filepath)?;
    let reader = BufReader::new(file_buffer);
    let text = reader
        .lines()
        .filter_map(Result::ok)
        .collect::<Vec<String>>()
        .join("\n\n");
    Ok(text)
}