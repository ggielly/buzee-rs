#[cfg(target_os = "windows")]
pub fn reveal_in_folder(file_path: &str) -> Result<(), String> {
    let do_steps = || -> Result<(), std::io::Error> {
        std::process::Command::new("explorer")
            .args(["/select,", file_path])
            .spawn()
            .map_err(|e| e)
            .map(|_| ())
    };

    if let Err(_err) = do_steps() {
        let path = std::path::PathBuf::from(file_path);
        if let Some(dir) = path.parent() {
            let _ = open::that_detached(dir);
        } else {
            log::warn!(
                "Could not determine the parent folder for {}",
                path.display()
            );
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn reveal_in_folder(file_path: &str) -> Result<(), String> {
    let path_buf = std::path::PathBuf::from(&file_path);
    let do_steps = || -> Result<(), std::io::Error> {
        if path_buf.is_dir() {
            std::process::Command::new("open")
                .args([&file_path])
                .spawn()
                .map_err(|e| e)
                .map(|_| ())
        } else {
            std::process::Command::new("open")
                .args(["-R", &file_path])
                .spawn()
                .map_err(|e| e)
                .map(|_| ())
        }
    };

    if let Err(_err) = do_steps() {
        if let Some(dir) = path_buf.parent() {
            let _ = open::that_detached(dir);
        } else {
            log::warn!(
                "Could not determine the parent folder for {}",
                path_buf.display()
            );
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn reveal_in_folder(file_path: &str) -> Result<(), String> {
    let path = std::path::PathBuf::from(file_path);
    if let Some(dir) = path.parent() {
        let _ = open::that_detached(dir);
    }
    Ok(())
}