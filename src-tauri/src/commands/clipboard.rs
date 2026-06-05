use tauri::Manager;

#[tauri::command]
pub async fn paste_item(app_handle: tauri::AppHandle, id: i64) -> Result<(), String> {
    let monitor = app_handle.state::<crate::services::clipboard_monitor::ClipboardMonitor>();

    let db = tauri_plugin_sql::DbPool::get(&app_handle, "sqlite:clipboard.db")
        .await
        .map_err(|e| e.to_string())?;

    let rows = db
        .select(
            "SELECT content, type, file_path FROM items WHERE id = ?",
            vec![serde_json::Value::Number(id.into())],
        )
        .await
        .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return Err("Item not found".to_string());
    }

    let row = &rows[0];
    let content = row.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let item_type = row.get("type").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let file_path = row.get("file_path").and_then(|v| v.as_str());

    monitor.set_suppress(true).await;

    let result = crate::services::paste_service::paste_content(content, item_type, file_path).await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    monitor.set_suppress(false).await;

    if result.is_ok() {
        let _ = db
            .execute(
                "UPDATE items SET copy_count = copy_count + 1, last_used_at = datetime('now') WHERE id = ?",
                vec![serde_json::Value::Number(id.into())],
            )
            .await;
        let _ = app_handle.emit("clipboard-changed", serde_json::json!({
            "action": "updated",
            "id": id,
        }));
    }

    result
}

#[tauri::command]
pub async fn delete_item(app_handle: tauri::AppHandle, id: i64) -> Result<(), String> {
    let db = tauri_plugin_sql::DbPool::get(&app_handle, "sqlite:clipboard.db")
        .await
        .map_err(|e| e.to_string())?;

    db.execute(
        "DELETE FROM items WHERE id = ?",
        vec![serde_json::Value::Number(id.into())],
    )
    .await
    .map_err(|e| e.to_string())?;

    let _ = app_handle.emit("item-deleted", id);
    Ok(())
}

#[tauri::command]
pub async fn toggle_favorite(app_handle: tauri::AppHandle, id: i64) -> Result<(), String> {
    let db = tauri_plugin_sql::DbPool::get(&app_handle, "sqlite:clipboard.db")
        .await
        .map_err(|e| e.to_string())?;

    db.execute(
        "UPDATE items SET is_favorite = NOT is_favorite WHERE id = ?",
        vec![serde_json::Value::Number(id.into())],
    )
    .await
    .map_err(|e| e.to_string())?;

    let _ = app_handle.emit("clipboard-changed", serde_json::json!({
        "action": "updated",
        "id": id,
    }));
    Ok(())
}

#[tauri::command]
pub async fn pause_monitoring(app_handle: tauri::AppHandle, paused: bool) -> Result<(), String> {
    let monitor = app_handle.state::<crate::services::clipboard_monitor::ClipboardMonitor>();
    monitor.set_paused(paused).await;
    Ok(())
}
