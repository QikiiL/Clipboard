use sqlx::sqlite::SqlitePool;
use tauri::{Emitter, Manager};

#[tauri::command]
pub async fn paste_item(app_handle: tauri::AppHandle, id: i64) -> Result<(), String> {
    let monitor = app_handle.state::<crate::services::clipboard_monitor::ClipboardMonitor>();
    let db = app_handle.state::<SqlitePool>();

    let row = sqlx::query_as::<_, (String, i32, Option<String>)>(
        "SELECT content, type, file_path FROM items WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&*db)
    .await
    .map_err(|e| e.to_string())?;

    let (content, item_type, file_path) = row.ok_or_else(|| "Item not found".to_string())?;

    monitor.set_suppress(true).await;

    let result = crate::services::paste_service::paste_content(
        &content,
        item_type,
        file_path.as_deref(),
    )
    .await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    monitor.set_suppress(false).await;

    if result.is_ok() {
        let _ = sqlx::query(
            "UPDATE items SET copy_count = copy_count + 1, last_used_at = datetime('now') WHERE id = ?",
        )
        .bind(id)
        .execute(&*db)
        .await;
        let _ = app_handle.emit(
            "clipboard-changed",
            serde_json::json!({"action": "updated", "id": id}),
        );
    }

    result
}

#[tauri::command]
pub async fn delete_item(app_handle: tauri::AppHandle, id: i64) -> Result<(), String> {
    let db = app_handle.state::<SqlitePool>();

    sqlx::query("DELETE FROM items WHERE id = ?")
        .bind(id)
        .execute(&*db)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app_handle.emit("item-deleted", id);
    Ok(())
}

#[tauri::command]
pub async fn toggle_favorite(app_handle: tauri::AppHandle, id: i64) -> Result<(), String> {
    let db = app_handle.state::<SqlitePool>();

    sqlx::query("UPDATE items SET is_favorite = NOT is_favorite WHERE id = ?")
        .bind(id)
        .execute(&*db)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app_handle.emit(
        "clipboard-changed",
        serde_json::json!({"action": "updated", "id": id}),
    );
    Ok(())
}

#[tauri::command]
pub async fn pause_monitoring(app_handle: tauri::AppHandle, paused: bool) -> Result<(), String> {
    let monitor = app_handle.state::<crate::services::clipboard_monitor::ClipboardMonitor>();
    monitor.set_paused(paused).await;
    Ok(())
}
