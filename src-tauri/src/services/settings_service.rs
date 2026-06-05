use crate::models::settings::AppSettings;
use tauri_plugin_store::StoreExt;

const STORE_KEY: &str = "app_settings";

pub fn load_settings(app_handle: &tauri::AppHandle) -> AppSettings {
    let store = app_handle.store("settings.json");
    match store {
        Ok(store) => {
            if let Some(value) = store.get(STORE_KEY) {
                serde_json::from_value(value.clone()).unwrap_or_default()
            } else {
                AppSettings::default()
            }
        }
        Err(_) => AppSettings::default(),
    }
}

pub fn save_settings(app_handle: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    let store = app_handle.store("settings.json").map_err(|e| e.to_string())?;
    let value = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    store.set(STORE_KEY, value);
    store.save().map_err(|e| e.to_string())?;

    // 同步自动启动状态
    use tauri_plugin_autostart::ManagerExt;
    if settings.start_with_windows {
        app_handle.autolaunch().enable().map_err(|e| e.to_string())?;
    } else {
        app_handle.autolaunch().disable().map_err(|e| e.to_string())?;
    }

    Ok(())
}
