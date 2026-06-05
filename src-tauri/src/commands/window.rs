use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
pub fn show_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        if !window.is_visible().unwrap_or(false) {
            window.show().map_err(|e| e.to_string())?;
        }
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn register_hotkey(
    app_handle: tauri::AppHandle,
    modifier: String,
    key: String,
) -> Result<(), String> {
    app_handle
        .global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;

    let mut mods = Modifiers::empty();
    if modifier.contains("Ctrl") {
        mods |= Modifiers::CONTROL;
    }
    if modifier.contains("Shift") {
        mods |= Modifiers::SHIFT;
    }
    if modifier.contains("Alt") {
        mods |= Modifiers::ALT;
    }
    if modifier.contains("Super") {
        mods |= Modifiers::SUPER;
    }

    let code = match key.to_uppercase().as_str() {
        "A" => Code::KeyA,
        "B" => Code::KeyB,
        "C" => Code::KeyC,
        "D" => Code::KeyD,
        "E" => Code::KeyE,
        "F" => Code::KeyF,
        "G" => Code::KeyG,
        "H" => Code::KeyH,
        "I" => Code::KeyI,
        "J" => Code::KeyJ,
        "K" => Code::KeyK,
        "L" => Code::KeyL,
        "M" => Code::KeyM,
        "N" => Code::KeyN,
        "O" => Code::KeyO,
        "P" => Code::KeyP,
        "Q" => Code::KeyQ,
        "R" => Code::KeyR,
        "S" => Code::KeyS,
        "T" => Code::KeyT,
        "U" => Code::KeyU,
        "V" => Code::KeyV,
        "W" => Code::KeyW,
        "X" => Code::KeyX,
        "Y" => Code::KeyY,
        "Z" => Code::KeyZ,
        _ => return Err(format!("Unsupported key: {}", key)),
    };

    let shortcut = Shortcut::new(Some(mods), code);
    app_handle
        .global_shortcut()
        .on_shortcut(shortcut, move |app_handle, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Some(window) = app_handle.get_webview_window("main") {
                    if !window.is_visible().unwrap_or(false) {
                        let _ = window.show();
                    }
                    let _ = window.set_focus();
                }
            }
        })
        .map_err(|e| e.to_string())?;

    Ok(())
}
