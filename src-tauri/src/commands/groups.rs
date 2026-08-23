use crate::models::clipboard_group::ClipboardGroup;
use sqlx::sqlite::SqlitePool;
use tauri::{Emitter, Manager};

type GroupRow = (i64, String, String, i32, String);

fn to_group(row: GroupRow) -> ClipboardGroup {
    ClipboardGroup {
        id: row.0,
        name: row.1,
        icon: row.2,
        sort_order: row.3,
        created_at: row.4,
    }
}

#[tauri::command]
pub async fn create_group(
    app_handle: tauri::AppHandle,
    name: String,
    icon: Option<String>,
) -> Result<ClipboardGroup, String> {
    let db = app_handle.state::<SqlitePool>();
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("分组名称不能为空".to_string());
    }
    let icon = icon.unwrap_or_else(|| "📁".to_string());

    let row = sqlx::query_as::<_, GroupRow>(
        "INSERT INTO groups (name, icon) VALUES (?, ?) RETURNING id, name, icon, sort_order, created_at",
    )
    .bind(&name)
    .bind(&icon)
    .fetch_one(&*db)
    .await
    .map_err(|e| e.to_string())?;

    let group = to_group(row);
    let _ = app_handle.emit("groups-changed", ());
    Ok(group)
}

#[tauri::command]
pub async fn update_group(
    app_handle: tauri::AppHandle,
    id: i64,
    name: String,
    icon: String,
) -> Result<(), String> {
    let db = app_handle.state::<SqlitePool>();
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("分组名称不能为空".to_string());
    }

    sqlx::query("UPDATE groups SET name = ?, icon = ? WHERE id = ?")
        .bind(&name)
        .bind(&icon)
        .bind(id)
        .execute(&*db)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app_handle.emit("groups-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn delete_group(app_handle: tauri::AppHandle, id: i64) -> Result<(), String> {
    let db = app_handle.state::<SqlitePool>();

    // 先把该分组下的条目移回未分组,再删除分组
    sqlx::query("UPDATE items SET group_id = NULL WHERE group_id = ?")
        .bind(id)
        .execute(&*db)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM groups WHERE id = ?")
        .bind(id)
        .execute(&*db)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app_handle.emit("groups-changed", ());
    // 触发列表刷新(条目的 group_id 可能已变化)
    let _ = app_handle.emit(
        "clipboard-changed",
        serde_json::json!({"action": "updated"}),
    );
    Ok(())
}

#[tauri::command]
pub async fn set_item_group(
    app_handle: tauri::AppHandle,
    item_id: i64,
    group_id: Option<i64>,
) -> Result<(), String> {
    let db = app_handle.state::<SqlitePool>();

    sqlx::query("UPDATE items SET group_id = ? WHERE id = ?")
        .bind(group_id)
        .bind(item_id)
        .execute(&*db)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app_handle.emit(
        "clipboard-changed",
        serde_json::json!({"action": "updated", "id": item_id}),
    );
    Ok(())
}
