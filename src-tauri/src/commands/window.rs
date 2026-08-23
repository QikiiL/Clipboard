use crate::models::settings::CloseBehavior;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use winreg::enums::*;
use winreg::RegKey;

/// Tracks the currently registered custom shortcut so it can be unregistered
/// when the user changes to a different hotkey.
pub struct LastHotkey(pub Mutex<Option<Shortcut>>);

#[tauri::command]
pub fn show_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    clear_paste_target(&app_handle);
    // 窗口可能已被销毁(隐藏即销毁),由 helper 统一处理存在/重建两种情况
    crate::utils::window_manager::show_or_create(&app_handle);
    Ok(())
}

/// 窗口经托盘/命令(而非热键)打开时,没有可靠的目标窗口,清空粘贴目标;
/// 粘贴/复制模式本身保持用户上次的手动选择(默认粘贴)
pub fn clear_paste_target(app_handle: &tauri::AppHandle) {
    if let Some(mode) = app_handle.try_state::<crate::utils::input_focus::PasteMode>() {
        mode.set_target(0);
    }
}

/// 置顶:悬浮于所有应用之上,并持久化到设置
#[tauri::command]
pub fn set_always_on_top(app_handle: tauri::AppHandle, pinned: bool) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .set_always_on_top(pinned)
            .map_err(|e| e.to_string())?;
    }
    let mut settings = crate::services::settings_service::load_settings(&app_handle);
    settings.pinned = pinned;
    crate::services::settings_service::save_settings(&app_handle, &settings)
}

/// 查询单击条目的当前行为(true=粘贴到输入框,false=仅复制)
#[tauri::command]
pub fn get_paste_mode(app_handle: tauri::AppHandle) -> Result<bool, String> {
    Ok(app_handle
        .state::<crate::utils::input_focus::PasteMode>()
        .get())
}

/// 手动切换单击行为(浏览器等应用检测不到输入符时的兜底)
#[tauri::command]
pub fn set_paste_mode(app_handle: tauri::AppHandle, input_mode: bool) -> Result<(), String> {
    use tauri::Emitter;
    let mode = app_handle.state::<crate::utils::input_focus::PasteMode>();
    mode.set(input_mode);
    let _ = app_handle.emit("paste-mode-changed", input_mode);
    Ok(())
}

#[tauri::command]
pub fn register_hotkey(
    app_handle: tauri::AppHandle,
    modifier: String,
    key: String,
) -> Result<(), String> {
    let last_hotkey = app_handle.state::<LastHotkey>();
    let mut last = last_hotkey.0.lock().unwrap_or_else(|e| e.into_inner());

    // Unregister the old shortcut FIRST, before registering the new one.
    // This is necessary because on_shortcut() fails if the shortcut is already registered
    // (e.g. by the plugin-level handler that runs at startup).
    if let Some(prev) = last.take() {
        let _ = app_handle.global_shortcut().unregister(prev);
    }

    let shortcut = crate::utils::hotkey::build_shortcut(&modifier, &key)
        .ok_or_else(|| format!("Unsupported key: {}", key))?;
    app_handle
        .global_shortcut()
        .on_shortcut(shortcut.clone(), move |app_handle, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                crate::utils::hotkey::toggle_window(app_handle);
            }
        })
        .map_err(|e| e.to_string())?;

    *last = Some(shortcut);

    Ok(())
}

#[tauri::command]
pub fn hide_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    // 隐藏即销毁:WebView2 进程树随之退出,内存归还系统
    crate::utils::window_manager::destroy_main_window(&app_handle);
    Ok(())
}

#[tauri::command]
pub fn get_close_behavior(app_handle: tauri::AppHandle) -> Result<CloseBehavior, String> {
    let settings = crate::services::settings_service::load_settings(&app_handle);
    Ok(settings.close_behavior)
}

/// 退出应用。主进程不随窗口销毁而退出(见 lib.rs 的 prevent_exit),
/// 必须显式 exit 触发完整清理流程。
#[tauri::command]
pub fn close_app(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[tauri::command]
pub fn set_close_behavior(
    app_handle: tauri::AppHandle,
    behavior: CloseBehavior,
) -> Result<(), String> {
    let mut settings = crate::services::settings_service::load_settings(&app_handle);
    settings.close_behavior = behavior;
    crate::services::settings_service::save_settings(&app_handle, &settings)
}

/// Enable Win+V integration: disable system clipboard history and register Win+V
#[tauri::command]
pub async fn enable_win_v_integration(app_handle: tauri::AppHandle) -> Result<(), String> {
    // 1. Disable system clipboard history via registry
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = r"SOFTWARE\Policies\Microsoft\Windows\System";
    let (key, _) = hklm
        .create_subkey(path)
        .map_err(|e| format!("写入注册表失败(需要以管理员身份运行本应用): {}", e))?;
    key.set_value("AllowClipboardHistory", &0u32)
        .map_err(|e| format!("写入注册表失败(需要以管理员身份运行本应用): {}", e))?;

    // 2. Try to restart Explorer to apply registry change immediately
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "explorer.exe"])
        .output();
    let _ = std::process::Command::new("explorer.exe").spawn();

    // 3. Persist the win_v_integration flag BEFORE registering hotkey
    let mut settings = crate::services::settings_service::load_settings(&app_handle);
    settings.win_v_integration = true;
    crate::services::settings_service::save_settings(&app_handle, &settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // 4. Register Win+V as the app hotkey.
    // Explorer 重启后系统释放 Win+V 的时机不定,首次注册常因占用失败(1409),带重试。
    // 注意:MutexGuard 不能跨 .await(否则 future 不是 Send),先注销旧热键并释放锁
    {
        let last_hotkey = app_handle.state::<LastHotkey>();
        let mut last = last_hotkey.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(prev) = last.take() {
            let _ = app_handle.global_shortcut().unregister(prev);
        }
    }

    let winv = Shortcut::new(Some(Modifiers::SUPER), Code::KeyV);
    let mut last_err = String::new();
    let mut registered = false;
    for attempt in 0..6 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
        match app_handle
            .global_shortcut()
            .on_shortcut(winv.clone(), move |app_handle, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    crate::utils::hotkey::toggle_window(app_handle);
                }
            }) {
            Ok(()) => {
                registered = true;
                break;
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    // 兜底:先用 Ctrl+Shift+V 保证可用,保留 win_v_integration 标志,下次启动继续尝试接管
    let final_shortcut = if registered {
        winv
    } else {
        let fallback = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV);
        let _ = app_handle
            .global_shortcut()
            .on_shortcut(fallback.clone(), move |app_handle, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    crate::utils::hotkey::toggle_window(app_handle);
                }
            });
        fallback
    };

    let last_hotkey = app_handle.state::<LastHotkey>();
    let mut last = last_hotkey.0.lock().unwrap_or_else(|e| e.into_inner());
    *last = Some(final_shortcut);

    if registered {
        return Ok(());
    }
    Err(format!(
        "Win+V 暂时无法注册({}),已先用 Ctrl+Shift+V。Win+V 可能仍被系统或其他软件占用,可稍后重新开启再试。",
        last_err
    ))
}

/// Disable Win+V integration: restore system clipboard history and original hotkey
#[tauri::command]
pub fn disable_win_v_integration(app_handle: tauri::AppHandle) -> Result<(), String> {
    // 1. Re-enable system clipboard history via registry
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = r"SOFTWARE\Policies\Microsoft\Windows\System";
    if let Ok((key, _)) = hklm.create_subkey(path) {
        let _ = key.delete_value("AllowClipboardHistory");
    }

    // 2. Restart Explorer to apply registry change
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "explorer.exe"])
        .output();
    let _ = std::process::Command::new("explorer.exe").spawn();

    // 3. Restore the user's original hotkey from settings
    let settings = crate::services::settings_service::load_settings(&app_handle);
    let shortcut = crate::utils::hotkey::build_shortcut(
        &settings.hotkey_modifier,
        &settings.hotkey_key,
    )
    .unwrap_or_else(|| Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV));

    let last_hotkey = app_handle.state::<LastHotkey>();
    let mut last = last_hotkey.0.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(prev) = last.take() {
        let _ = app_handle.global_shortcut().unregister(prev);
    }

    // 自定义热键注册失败时退回默认 Ctrl+Shift+V,保证始终有热键可用
    let final_shortcut = match app_handle
        .global_shortcut()
        .on_shortcut(shortcut.clone(), move |app_handle, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                crate::utils::hotkey::toggle_window(app_handle);
            }
        }) {
        Ok(()) => shortcut,
        Err(e) => {
            eprintln!("Failed to register custom hotkey after Win+V off: {}. Falling back to Ctrl+Shift+V.", e);
            let fallback = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV);
            let _ = app_handle
                .global_shortcut()
                .on_shortcut(fallback.clone(), move |app_handle, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        crate::utils::hotkey::toggle_window(app_handle);
                    }
                });
            fallback
        }
    };

    *last = Some(final_shortcut);

    // Persist the win_v_integration flag
    let mut settings = crate::services::settings_service::load_settings(&app_handle);
    settings.win_v_integration = false;
    crate::services::settings_service::save_settings(&app_handle, &settings)
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}
