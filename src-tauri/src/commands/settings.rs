use crate::models::settings::AppSettings;

#[tauri::command]
pub fn load_settings(app_handle: tauri::AppHandle) -> Result<AppSettings, String> {
    Ok(crate::services::settings_service::load_settings(&app_handle))
}

#[tauri::command]
pub fn save_settings(app_handle: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    crate::services::settings_service::save_settings(&app_handle, &settings)
}
