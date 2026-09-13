use base64::Engine;
use sqlx::sqlite::SqlitePool;
use tauri::{Emitter, Manager};

#[tauri::command]
pub async fn activate_item(
    app_handle: tauri::AppHandle,
    id: i64,
    force_paste: Option<bool>,
) -> Result<(), String> {
    // 粘贴还是仅复制:优先取前端的强制指定,否则按唤出窗口时检测到的输入状态
    let do_paste = force_paste.unwrap_or_else(|| {
        app_handle
            .state::<crate::utils::input_focus::PasteMode>()
            .get()
    });

    // 窗口销毁/焦点归还/监听抑制/模拟按键/更新使用时间,全部在共享核心里
    // (与连续粘贴同一条投递路径,行为不会分叉)
    match crate::services::paste_service::deliver_item_by_id(
        &app_handle, id, true, do_paste, 500,
    )
    .await?
    {
        crate::services::paste_service::Delivery::Delivered => Ok(()),
        crate::services::paste_service::Delivery::Missing => Err("Item not found".to_string()),
    }
}

#[tauri::command]
pub async fn delete_item(app_handle: tauri::AppHandle, id: i64) -> Result<(), String> {
    let db = app_handle.state::<SqlitePool>();

    // 先取出图片路径,删行后清理对应文件
    let row = sqlx::query_as::<_, (Option<i32>, Option<String>)>(
        "SELECT type, file_path FROM items WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&*db)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM items WHERE id = ?")
        .bind(id)
        .execute(&*db)
        .await
        .map_err(|e| e.to_string())?;

    if let Some((item_type, file_path)) = row {
        if item_type == Some(crate::models::clipboard_type::ClipboardType::Image as i32) {
            if let Some(path) = file_path {
                crate::services::image_cleanup::remove_image_file(&app_handle, &path);
            }
        }
    }

    let _ = app_handle.emit("item-deleted", id);
    Ok(())
}

/// 清空剪贴板历史(保留收藏):删除所有未收藏条目,清理不再被引用的图片文件
/// (含孤儿文件),并尽力 VACUUM 压缩数据库。
/// days = None 或 <=0 清除全部;>0 只清除 last_used_at 早于 N 天前的条目
/// (与保留天数自动清理的时间列保持一致)。
#[tauri::command]
pub async fn clear_history(
    app_handle: tauri::AppHandle,
    days: Option<i64>,
) -> Result<(), String> {
    let db = app_handle.state::<SqlitePool>();

    let result = match days.filter(|d| *d > 0) {
        Some(days) => {
            sqlx::query(
                "DELETE FROM items WHERE is_favorite = 0 \
                 AND last_used_at < datetime('now', '-' || ? || ' days')",
            )
            .bind(days)
            .execute(&*db)
            .await
            .map_err(|e| e.to_string())?
        }
        None => sqlx::query("DELETE FROM items WHERE is_favorite = 0")
            .execute(&*db)
            .await
            .map_err(|e| e.to_string())?,
    };

    if result.rows_affected() > 0 {
        // 收藏条目的图片保留,其余(含孤儿文件)清除
        crate::services::image_cleanup::remove_unreferenced_images(&app_handle, &*db).await;

        // 尽力回收数据库空间;若恰有并发连接占用导致失败,不影响清空结果
        if let Err(e) = sqlx::query("VACUUM").execute(&*db).await {
            eprintln!("VACUUM after clear_history failed: {}", e);
        }
    }

    let _ = app_handle.emit("clipboard-changed", serde_json::json!({"action": "cleared"}));
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

#[tauri::command]
pub async fn get_image_base64(
    app_handle: tauri::AppHandle,
    file_path: String,
) -> Result<String, String> {
    // 1. SECURITY: 图片路径必须落在应用 images 目录内。
    // 统一走 storage_service::resolve_image_path 守卫(读取/删除/粘贴三条
    // 路径同源),此前这里内联复制了一份同等逻辑,守卫一旦改动容易漏改
    let canonical =
        crate::services::storage_service::resolve_image_path(&app_handle, &file_path)
            .ok_or_else(|| "图片不存在或不在应用图片目录内,已拒绝读取".to_string())?;

    // 2. File size limit: reject files over 5MB
    const MAX_SIZE: u64 = 5 * 1024 * 1024;
    let metadata = tokio::fs::metadata(&canonical)
        .await
        .map_err(|e| format!("Failed to read file metadata: {}", e))?;
    if metadata.len() > MAX_SIZE {
        return Err(format!(
            "File too large ({}MB exceeds 5MB limit)",
            metadata.len() / (1024 * 1024)
        ));
    }

    let bytes = tokio::fs::read(&canonical)
        .await
        .map_err(|e| format!("Failed to read file '{}': {}", file_path, e))?;

    // 3. Detect MIME type from magic bytes
    let mime = detect_image_mime(&bytes);

    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);

    Ok(format!("data:{};base64,{}", mime, encoded))
}

/// 与采集端(ClipboardMonitor)完全一致的预览截断:前 100 个**字符**。
/// 用 chars() 而非按字节切片 —— 中文/emoji 下按字节切会切出半个字符。
pub(crate) fn build_preview(content: &str) -> String {
    content.chars().take(100).collect()
}

/// 编辑条目内容:同步更新 content 与 preview,并重算 content_hash,
/// 保证后续「内容去重」语义与采集端一致(否则编辑后同一内容会被当成新条目重复插入)。
/// 仅由前端的文本预览/编辑弹窗调用;图片条目前端不提供入口。
#[tauri::command]
pub async fn update_item_content(
    app_handle: tauri::AppHandle,
    id: i64,
    content: String,
) -> Result<(), String> {
    let db = app_handle.state::<SqlitePool>();
    let preview = build_preview(&content);
    let hash = crate::utils::hash::compute_hash(&content);

    let result = sqlx::query(
        "UPDATE items SET content = ?, preview = ?, content_hash = ? WHERE id = ?",
    )
    .bind(&content)
    .bind(&preview)
    .bind(&hash)
    .bind(id)
    .execute(&*db)
    .await
    .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        return Err("Item not found".to_string());
    }

    // 让列表按新内容重查(前端监听 clipboard-changed 后整表刷新)
    let _ = app_handle.emit(
        "clipboard-changed",
        serde_json::json!({"action": "updated", "id": id}),
    );
    Ok(())
}

fn detect_image_mime(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 4 && bytes[..4] == [0x89, 0x50, 0x4E, 0x47] {
        "image/png"
    } else if bytes.len() >= 3 && bytes[..3] == [0xFF, 0xD8, 0xFF] {
        "image/jpeg"
    } else if bytes.len() >= 4 && bytes[..4] == *b"GIF8" {
        "image/gif"
    } else if bytes.len() >= 12 && bytes[..4] == *b"RIFF" && bytes[8..12] == *b"WEBP" {
        "image/webp"
    } else {
        "image/png"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_truncates_on_char_boundary() {
        let preview = build_preview(&"验".repeat(150));
        assert_eq!(preview.chars().count(), 100);
        assert_eq!(preview, "验".repeat(100));
    }

    #[test]
    fn preview_keeps_short_content() {
        assert_eq!(build_preview("hello"), "hello");
    }
}
