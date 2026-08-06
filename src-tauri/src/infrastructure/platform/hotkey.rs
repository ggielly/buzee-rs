/// Parse a "Mod+Key" shortcut string into a `global_hotkey::HotKey`. The key
/// component may carry a `Key`/`Digit` prefix (as stored by the app); this
/// function strips it and maps it onto the crate's key enum.
pub fn parse_hotkey(shortcut: &str) -> Result<global_hotkey::hotkey::HotKey, String> {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};

    let mut splits: Vec<&str> = shortcut.split('+').collect();
    if splits.is_empty() {
        return Err(format!("Invalid shortcut: {}", shortcut));
    }

    let key_part = splits.pop().unwrap_or("");

    let mut modifiers = Modifiers::empty();
    for modifier in &splits {
        match modifier.to_uppercase().as_str() {
            "ALT" => modifiers |= Modifiers::ALT,
            "CTRL" | "CONTROL" => modifiers |= Modifiers::CONTROL,
            "SHIFT" => modifiers |= Modifiers::SHIFT,
            "SUPER" | "CMD" | "META" | "COMMAND" => modifiers |= Modifiers::SUPER,
            _ => {
                log::warn!("Unknown modifier in shortcut: {}", modifier);
            }
        }
    }

    // Normalize the key: strip a Key/Digit prefix used by the DB representation.
    let normalized = if key_part.contains("Key") {
        key_part.split("Key").last().unwrap_or(key_part).to_string()
    } else if key_part.contains("Digit") {
        key_part.split("Digit").last().unwrap_or(key_part).to_string()
    } else {
        key_part.to_string()
    };

    let code = match normalized.to_lowercase().as_str() {
        "space" => Code::Space,
        "backspace" => Code::Backspace,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "escape" | "esc" => Code::Escape,
        "up" => Code::ArrowUp,
        "down" => Code::ArrowDown,
        "left" => Code::ArrowLeft,
        "right" => Code::ArrowRight,
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,
        other => {
            if other.len() == 1 {
                let c = other.chars().next().unwrap();
                if c.is_ascii_alphabetic() {
                    Code::KeyA
                } else if c.is_ascii_digit() {
                    Code::Digit0
                } else {
                    return Err(format!("Unsupported key in shortcut: {}", shortcut));
                }
            } else {
                return Err(format!("Unsupported key in shortcut: {}", shortcut));
            }
        }
    };

    Ok(HotKey::new(Some(modifiers), code))
}

/// Register a global hotkey best-effort. A failure is logged and ignored so the
/// app still starts when the hotkey is already claimed by the OS or another app.
pub fn register_best_effort(
    registry: &global_hotkey::GlobalHotKeyManager,
    shortcut: &str,
) -> Result<global_hotkey::hotkey::HotKey, String> {
    let hotkey = parse_hotkey(shortcut)?;
    if let Err(e) = registry.register(hotkey) {
        log::error!("Failed to register the global shortcut ({}); continuing without it.", e);
        return Err(e.to_string());
    }
    log::info!("Registered global shortcut: {}", shortcut);
    Ok(hotkey)
}