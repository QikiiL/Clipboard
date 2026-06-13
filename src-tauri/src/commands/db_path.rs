use tauri::Manager;

/// Returns the absolute SQLite database URL used by the backend.
/// The frontend calls this to ensure it connects to the same database.
#[tauri::command]
pub fn get_db_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    let app_config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(&app_config_dir);
    let db_path = app_config_dir.join("clipboard.db");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));
    Ok(db_url)
}
