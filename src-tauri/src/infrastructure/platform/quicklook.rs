/// Open QuickLook (macOS) or Peek (Windows) for a file. Best-effort: failures
/// are logged and the caller continues.
pub fn open_preview(file_path: &str) -> Result<(), String> {
    log::info!("Opening QuickLook for {}", file_path);
    let file_path = file_path.to_string();

    #[cfg(target_os = "macos")]
    std::thread::spawn(move || {
        let _ = std::process::Command::new("qlmanage")
            .arg("-p")
            .arg(&file_path)
            .output();
    });

    #[cfg(target_os = "windows")]
    std::thread::spawn(move || {
        let Some(home_directory) = dirs::home_dir() else {
            log::warn!("Could not resolve home directory for QuickLook peek");
            return;
        };
        let home_directory = home_directory.to_string_lossy().to_string();
        let quicklook_path = format!(
            "{}\\AppData\\Local\\Programs\\QuickLook\\QuickLook.exe",
            &home_directory
        );
        let _ = std::process::Command::new(quicklook_path).arg(&file_path).output();
    });

    Ok(())
}