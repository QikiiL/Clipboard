use crate::models::settings::AppSettings;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const STORE_KEY: &str = "app_settings";

/// settings.json 固定放在程序目录的 config/ 下(绝对路径传入,
/// 不走插件默认的 %APPDATA% 解析)
fn store_path() -> std::path::PathBuf {
    crate::app_config_dir().join(STORE_FILE)
}

pub fn load_settings(app_handle: &tauri::AppHandle) -> AppSettings {
    let store = app_handle.store(store_path());
    match store {
        Ok(store) => {
            if let Some(value) = store.get(STORE_KEY) {
                serde_json::from_value(value.clone()).unwrap_or_default()
            } else {
                AppSettings::default()
            }
        }
        Err(e) => {
            eprintln!("Failed to load settings store: {}", e);
            AppSettings::default()
        }
    }
}

pub fn save_settings(app_handle: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    // 取改动前的值:自动启动只在开关真正变化时同步,避免每次保存都触碰注册表
    let previous = load_settings(app_handle);

    let store = app_handle
        .store(store_path())
        .map_err(|e| e.to_string())?;
    let value = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    store.set(STORE_KEY, value);
    store.save().map_err(|e| e.to_string())?;

    // 同步自动启动状态（最佳努力，不影响设置保存）
    if settings.start_with_windows != previous.start_with_windows {
        use tauri_plugin_autostart::ManagerExt;
        if settings.start_with_windows {
            if let Err(e) = app_handle.autolaunch().enable() {
                eprintln!("Failed to enable autolaunch: {}", e);
            }
        } else if let Err(e) = app_handle.autolaunch().disable() {
            // 自启项本就不存在(os error 2)说明已是关闭状态,不算失败
            if !e.to_string().contains("os error 2") {
                eprintln!("Failed to disable autolaunch: {}", e);
            }
        }
    }

    Ok(())
}
