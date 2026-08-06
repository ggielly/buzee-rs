/// Restart the application gracefully: spawn the current executable and exit the
/// current process after `wait_time` seconds. Used after preferences that need a
/// restart take effect.
pub fn graceful_restart(wait_time: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(wait_time));
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
            std::process::exit(0);
        }
    });
}

/// Exit the process without restarting (used when there is nothing to relaunch).
pub fn exit_after(wait_time: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(wait_time));
        std::process::exit(0);
    });
}