/// 删除条目对应的图片文件。路径守卫统一走 storage_service::resolve_image_path
/// (与读取/粘贴同源,见 SECURITY-AUDIT 三条消费路径守卫约定),
/// 越界或文件不存在时静默跳过。
pub fn remove_image_file(app_handle: &tauri::AppHandle, file_path: &str) {
    if let Some(canonical) =
        crate::services::storage_service::resolve_image_path(app_handle, file_path)
    {
        let _ = std::fs::remove_file(&canonical);
    }
}

/// 删除条目对应的富文本格式文件(html/rtf)。守卫与图片同源,只是
/// 换成 formats 目录(resolve_format_path);越界或不存在时静默跳过。
pub fn remove_format_file(app_handle: &tauri::AppHandle, file_path: &str) {
    if let Some(canonical) =
        crate::services::storage_service::resolve_format_path(app_handle, file_path)
    {
        let _ = std::fs::remove_file(&canonical);
    }
}

/// 删除目录中未被任何条目列引用的文件(通用孤儿清扫,供 images/formats 共用)。
/// referenced 为各条目路径列的 DISTINCT 并集(已规范化);仅删除文件。
async fn remove_unreferenced_in_dir(
    dir: &std::path::Path,
    referenced: &std::collections::HashSet<std::path::PathBuf>,
) {
    let entries = match std::fs::read_dir(dir) {
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

/// 删除 images 目录中未被任何条目引用的图片文件(用于清空历史:
/// 收藏条目及其图片被保留,其余文件含历史孤儿一并清除)。
/// 路径比较先规范化,避免分隔符/相对形式差异导致误删;仅删除文件。
/// formats/ 目录的 html/rtf 孤儿文件同样纳入清扫(引用比对来源为
/// file_path + html_path + rtf_path 三列的并集)。
pub async fn remove_unreferenced_images(
    app_handle: &tauri::AppHandle,
    db: &sqlx::sqlite::SqlitePool,
) {
    let images_dir = crate::services::storage_service::images_dir(app_handle);

    // 仍被引用(收藏)的文件集合:三个路径列取 DISTINCT 并集。
    // 引用集合对两个目录共用 —— formats 文件名与图片 hash 同源,
    // 互不重叠,合并比对不会误删
    let mut referenced: std::collections::HashSet<std::path::PathBuf> = Default::default();
    for column in ["file_path", "html_path", "rtf_path"] {
        let rows = match sqlx::query_as::<_, (String,)>(&format!(
            "SELECT DISTINCT {column} FROM items WHERE {column} IS NOT NULL"
        ))
        .fetch_all(db)
        .await
        {
            Ok(rows) => rows,
            Err(_) => return, // 查询失败时宁可不清,避免误删
        };
        for (path,) in rows {
            if let Ok(c) = std::fs::canonicalize(&path) {
                referenced.insert(c);
            }
        }
    }

    remove_unreferenced_in_dir(&images_dir, &referenced).await;
    let formats_dir = crate::services::storage_service::formats_dir(app_handle);
    remove_unreferenced_in_dir(&formats_dir, &referenced).await;
}
