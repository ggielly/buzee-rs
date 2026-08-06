use std::error::Error;

pub fn extract(file: &String) -> Result<String, Box<dyn Error>> {
    let file = std::fs::read_to_string(file)?;
    let lines: Vec<String> = file.lines().map(|line| line.to_string()).collect();
    let text = lines.join("\n");
    let text: Vec<char> = text.chars().filter(|c| !c.is_numeric()).collect();
    Ok(text.into_iter().collect())
}