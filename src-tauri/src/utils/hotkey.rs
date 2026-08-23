use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

/// Toggle panel visibility. 热键切换的是“面板是否可见”,焦点状态不参与判定:
/// 面板可见(未最小化)时一按即隐藏——即使焦点在其他窗口;不可见/已最小化/
/// 已销毁时显示或重建。原“可见且聚焦才隐藏”的语义会在点击其他窗口后
/// 要求按两次热键(第一次仅切回焦点),不符合直觉,已弃用。
pub fn toggle_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        let is_minimized = window.is_minimized().unwrap_or(false);

        if is_visible && !is_minimized {
            crate::utils::window_manager::destroy_main_window(app);
        } else {
            show_with_paste_target(app);
            if is_minimized {
                let _ = window.unminimize();
            }
            let _ = window.show();
            let _ = window.set_focus();
        }
    } else {
        // 窗口已被销毁:重建需要约 1s,期间 CREATING 标志防重入
        show_with_paste_target(app);
        crate::utils::window_manager::show_or_create(app);
    }
}

/// 显示窗口前,前台还是目标应用:记录其窗口句柄,
/// 单击条目粘贴时把焦点还给它
fn show_with_paste_target(app: &tauri::AppHandle) {
    if let Some(mode) = app.try_state::<crate::utils::input_focus::PasteMode>() {
        mode.set_target(crate::utils::input_focus::capture_foreground_hwnd());
    }
}

/// Parse modifier string into Modifiers bitflags (case-insensitive).
pub fn parse_modifiers(modifier: &str) -> Modifiers {
    let lower = modifier.to_lowercase();
    let mut mods = Modifiers::empty();
    if lower.contains("ctrl") {
        mods |= Modifiers::CONTROL;
    }
    if lower.contains("shift") {
        mods |= Modifiers::SHIFT;
    }
    if lower.contains("alt") {
        mods |= Modifiers::ALT;
    }
    if lower.contains("super") {
        mods |= Modifiers::SUPER;
    }
    mods
}

/// Map key name to Code (case-insensitive). Returns None for unsupported keys.
pub fn map_key_code(key: &str) -> Option<Code> {
    let upper = key.to_uppercase();
    match upper.as_str() {
        "A" => Some(Code::KeyA),
        "B" => Some(Code::KeyB),
        "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD),
        "E" => Some(Code::KeyE),
        "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG),
        "H" => Some(Code::KeyH),
        "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ),
        "K" => Some(Code::KeyK),
        "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM),
        "N" => Some(Code::KeyN),
        "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP),
        "Q" => Some(Code::KeyQ),
        "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS),
        "T" => Some(Code::KeyT),
        "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV),
        "W" => Some(Code::KeyW),
        "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY),
        "Z" => Some(Code::KeyZ),
        "0" => Some(Code::Digit0),
        "1" => Some(Code::Digit1),
        "2" => Some(Code::Digit2),
        "3" => Some(Code::Digit3),
        "4" => Some(Code::Digit4),
        "5" => Some(Code::Digit5),
        "6" => Some(Code::Digit6),
        "7" => Some(Code::Digit7),
        "8" => Some(Code::Digit8),
        "9" => Some(Code::Digit9),
        _ => None,
    }
}

/// Build a Shortcut from modifier+key strings. Returns None if key is unsupported.
pub fn build_shortcut(modifier: &str, key: &str) -> Option<Shortcut> {
    let mods = parse_modifiers(modifier);
    let code = map_key_code(key)?;
    Some(Shortcut::new(Some(mods), code))
}
