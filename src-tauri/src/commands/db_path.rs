/// Returns the absolute SQLite database URL used by the backend.
/// The frontend calls this to ensure it connects to the same database.
/// 位置可能为用户自定义(见 storage_service),必须走统一解析。
#[tauri::command]
pub fn get_db_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    let data_dir = crate::services::storage_service::current_data_dir(&app_handle);
    let db_path = data_dir.join("clipboard.db");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));
    Ok(db_url)
}
