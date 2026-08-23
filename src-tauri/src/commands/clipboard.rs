use base64::Engine;
use sqlx::sqlite::SqlitePool;
use tauri::{Emitter, Manager};

#[tauri::command]
pub async fn activate_item(
    app_handle: tauri::AppHandle,
    id: i64,
    force_paste: Option<bool>,
) -> Result<(), String> {
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

    // 粘贴还是仅复制:优先取前端的强制指定,否则按唤出窗口时检测到的输入状态
    let do_paste = force_paste.unwrap_or_else(|| {
        app_handle
            .state::<crate::utils::input_focus::PasteMode>()
            .get()
    });
    let mode = if do_paste {
        crate::services::paste_service::DeliverMode::Paste
    } else {
        crate::services::paste_service::DeliverMode::CopyOnly
    };

    // 先销毁窗口(隐藏即销毁,WebView2 进程树随之退出),
    // 再主动把前台焦点还给唤出前的目标窗口并等待其就绪;
    // 若不等待,模拟的 Ctrl+V 会在焦点切换完成前发出而落空
    crate::utils::window_manager::destroy_main_window(&app_handle);
    let target_hwnd = app_handle
        .state::<crate::utils::input_focus::PasteMode>()
        .target();
    if target_hwnd != 0 {
        let _ = tauri::async_runtime::spawn_blocking(move || {
            crate::utils::input_focus::restore_target_focus(target_hwnd)
        })
        .await;
    } else {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    monitor.set_suppress(true).await;

    let result = crate::services::paste_service::deliver_content(
        &content,
        item_type,
        file_path.as_deref(),
        mode,
    )
    .await;

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    monitor.set_suppress(false).await;

    if result.is_ok() {
        if let Err(e) = sqlx::query(
            "UPDATE items SET last_used_at = datetime('now') WHERE id = ?",
        )
        .bind(id)
        .execute(&*db)
        .await
        {
            eprintln!("Failed to update copy count: {}", e);
        }
        if let Err(e) = app_handle.emit(
            "clipboard-changed",
            serde_json::json!({"action": "updated", "id": id}),
        ) {
            eprintln!("Failed to emit clipboard-changed event: {}", e);
        }
    }

    result
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
/// (含孤儿文件),并尽力 VACUUM 压缩数据库
#[tauri::command]
pub async fn clear_history(app_handle: tauri::AppHandle) -> Result<(), String> {
    let db = app_handle.state::<SqlitePool>();

    sqlx::query("DELETE FROM items WHERE is_favorite = 0")
        .execute(&*db)
        .await
        .map_err(|e| e.to_string())?;

    // 收藏条目的图片保留,其余(含孤儿文件)清除
    crate::services::image_cleanup::remove_unreferenced_images(&app_handle, &*db).await;

    // 尽力回收数据库空间;若恰有并发连接占用导致失败,不影响清空结果
    if let Err(e) = sqlx::query("VACUUM").execute(&*db).await {
        eprintln!("VACUUM after clear_history failed: {}", e);
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
    // 1. SECURITY: Resolve the images directory and validate path is inside it
    let images_dir = crate::services::storage_service::images_dir(&app_handle);

    // Ensure images dir exists before canonicalizing
    let _ = std::fs::create_dir_all(&images_dir);

    let canonical_images = std::fs::canonicalize(&images_dir)
        .map_err(|e| format!("Failed to resolve images directory: {}", e))?;

    let canonical = std::fs::canonicalize(&file_path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => "Image file not found".to_string(),
        _ => format!("Failed to resolve file path: {}", e),
    })?;

    if !canonical.starts_with(&canonical_images) {
        return Err("Access denied: path is outside the images directory".to_string());
    }

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
