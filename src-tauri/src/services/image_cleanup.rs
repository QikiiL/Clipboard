/// 删除条目对应的图片文件,仅当文件位于应用的 images 目录内时才执行,
/// 避免误删任意路径。文件不存在时静默跳过。
pub fn remove_image_file(app_handle: &tauri::AppHandle, file_path: &str) {
    let images_dir = crate::services::storage_service::images_dir(app_handle);
    let canonical_images = match std::fs::canonicalize(&images_dir) {
        Ok(p) => p,
        Err(_) => return,
    };
    let canonical = match std::fs::canonicalize(file_path) {
        Ok(p) => p,
        Err(_) => return,
    };
    if canonical.starts_with(&canonical_images) {
        let _ = std::fs::remove_file(&canonical);
    }
}

/// 删除 images 目录中未被任何条目引用的图片文件(用于清空历史:
/// 收藏条目及其图片被保留,其余文件含历史孤儿一并清除)。
/// 路径比较先规范化,避免分隔符/相对形式差异导致误删;仅删除文件。
pub async fn remove_unreferenced_images(
    app_handle: &tauri::AppHandle,
    db: &sqlx::sqlite::SqlitePool,
) {
    let images_dir = crate::services::storage_service::images_dir(app_handle);

    // 仍被引用(收藏)的图片集合
    let rows = match sqlx::query_as::<_, (String,)>(
        "SELECT DISTINCT file_path FROM items WHERE file_path IS NOT NULL",
    )
    .fetch_all(db)
    .await
    {
        Ok(rows) => rows,
        Err(_) => return, // 查询失败时宁可不清,避免误删
    };
    let referenced: std::collections::HashSet<std::path::PathBuf> = rows
        .into_iter()
        .filter_map(|(path,)| std::fs::canonicalize(&path).ok())
        .collect();

    let entries = match std::fs::read_dir(&images_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match std::fs::canonicalize(&path) {
            Ok(canonical) => {
                if !referenced.contains(&canonical) {
                    let _ = std::fs::remove_file(&path);
                }
            }
            Err(_) => continue, // 无法规范化的文件跳过,不删
        }
    }
}
