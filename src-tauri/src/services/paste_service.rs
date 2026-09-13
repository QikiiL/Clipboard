use crate::models::clipboard_type::ClipboardType;
use enigo::{Direction, Enigo, Key, Keyboard};
use sqlx::sqlite::SqlitePool;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager};

/// 单击条目的目标行为
pub enum DeliverMode {
    /// 写入剪贴板后模拟 Ctrl+V,粘贴到之前聚焦的输入框
    Paste,
    /// 仅写入剪贴板,不模拟按键
    CopyOnly,
}

/// 模拟 Ctrl+V 粘贴。
/// 镜像用户的物理 Ctrl 状态:连续快速触发时用户常"按住 Ctrl 连点 V",
/// 此时应用侧修饰键已经是按下态 —— 只补一个 V 即可,且结束时绝不发
/// Ctrl-up,否则会把用户手里还没松开的 Ctrl 提前"放掉",
/// 后续的裸 V 就会被目标应用打成字母 v
pub fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&enigo::Settings::default()).map_err(|e| e.to_string())?;
    let ctrl_held_by_user = crate::utils::paste_hook::is_ctrl_physically_down();

    if !ctrl_held_by_user {
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| e.to_string())?;
    }
    enigo
        .key(Key::Unicode('v'), Direction::Press)
        .map_err(|e| e.to_string())?;
    enigo
        .key(Key::Unicode('v'), Direction::Release)
        .map_err(|e| e.to_string())?;
    if !ctrl_held_by_user {
        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 将文本写入剪切板;若剪贴板内容已与之一致则跳过写入(返回 false),
/// 此时直接模拟 Ctrl+V 即可,避免重复复制。
fn write_text_if_changed(text: &str) -> Result<bool, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    if let Ok(current) = cb.get_text() {
        if current == text {
            return Ok(false);
        }
    }
    cb.set_text(text.to_string())
        .map_err(|e| e.to_string())?;
    Ok(true)
}

/// 将图片写入剪切板。file_path 必须是经 storage_service::resolve_image_path
/// 校验过的规范化绝对路径
fn write_image_to_clipboard(file_path: &Path) -> Result<(), String> {
    let img = image::open(file_path).map_err(|e| format!("Failed to decode image: {}", e))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let img_data = arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: std::borrow::Cow::Owned(rgba.into_raw()),
    };
    arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .set_image(img_data)
        .map_err(|e| format!("Failed to set image on clipboard: {}", e))?;
    Ok(())
}

/// 将文件列表写入剪切板(CF_HDROP)
fn write_files_to_clipboard(paths: Vec<String>) -> Result<(), String> {
    // clipboard_win 的 set_clipboard 按值传参,与 FileList 的 Setter<[T]> 实现
    // 不兼容,因此持 guard 后直接调用 raw API
    let _clip = clipboard_win::Clipboard::new_attempts(10)
        .map_err(|e| format!("Failed to open clipboard: {}", e))?;
    clipboard_win::raw::set_file_list_with(&paths, clipboard_win::options::DoClear)
        .map_err(|e| format!("Failed to set file list on clipboard: {}", e))?;
    Ok(())
}

/// 投递指定条目内容:按模式写入剪贴板,Paste 模式额外模拟 Ctrl+V。
/// 调用前需先隐藏本应用窗口,确保按键落到目标应用上。
pub async fn deliver_content(
    app_handle: tauri::AppHandle,
    content: &str,
    item_type: i32,
    file_path: Option<&str>,
    mode: DeliverMode,
) -> Result<(), String> {
    let content = content.to_string();
    let file_path = file_path.map(|s| s.to_string());
    let do_paste = matches!(mode, DeliverMode::Paste);
    tokio::task::spawn_blocking(move || {
        let wrote = match item_type {
            t if t == ClipboardType::Text as i32 || t == ClipboardType::Link as i32 => {
                write_text_if_changed(&content)?
            }
            t if t == ClipboardType::Image as i32 => {
                let raw = file_path.as_deref().unwrap_or(&content);
                // 图片路径来自数据库,必须先校验落在 images 目录内才打开,
                // 与读取(get_image_base64)、删除(remove_image_file)一致;
                // 校验失败直接报错,不能降级当文本处理,否则守卫形同虚设
                let path = crate::services::storage_service::resolve_image_path(&app_handle, raw)
                    .ok_or_else(|| "图片路径无效或不在应用图片目录内,已拒绝粘贴".to_string())?;
                write_image_to_clipboard(&path)?;
                true
            }
            t if t == ClipboardType::File as i32 => {
                // 文件条目的 content 是换行分隔的路径列表
                let paths: Vec<String> = content
                    .lines()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty())
                    .collect();
                if paths.is_empty() {
                    write_text_if_changed(&content)?
                } else {
                    write_files_to_clipboard(paths)?;
                    true
                }
            }
            _ => write_text_if_changed(&content)?,
        };

        if do_paste {
            // 写入剪贴板后等待其生效;跳过写入时也给焦点切换留一点余量
            thread::sleep(Duration::from_millis(if wrote { 50 } else { 30 }));
            simulate_paste()?;
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 投递结果:Delivered = 已写入剪贴板并(按模式)模拟按键;
/// Missing = 条目已不存在(连续粘贴用它跳过被删除的条目)
pub enum Delivery {
    Delivered,
    Missing,
}

/// 销毁窗口并把前台焦点还给唤出前记录的目标窗口(不投递任何内容)。
/// 单击条目投递的"先隐藏"分支与连续粘贴「只武装、不立即粘贴」的
/// 启动路径共用
pub async fn release_focus_to_target(app_handle: &tauri::AppHandle) {
    // 先销毁窗口(隐藏即销毁,WebView2 进程树随之退出),
    // 再主动把前台焦点还给唤出前的目标窗口并等待其真正就绪;
    // 若不等待,后续模拟的 Ctrl+V 会在焦点切换完成前发出而落空
    crate::utils::window_manager::destroy_main_window(app_handle);
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
}

/// 按 id 读取条目并投递。单击条目(activate_item)与连续粘贴共用此核心,
/// 二者的差异只有焦点处理:
/// - hide_first = true(面板内点击):先销毁窗口,把前台焦点还给唤出前
///   记录的目标窗口并等待其就绪,否则模拟的 Ctrl+V 会在焦点切换完成前落空;
/// - hide_first = false(全局触发的连续粘贴):前台已经是目标应用,
///   不动焦点 —— 粘贴总是落进用户当前光标所在的输入框。
/// suppress_ms = 粘贴完成后抑制剪贴板监听的时长(盖过写入瞬间即可;
/// 单击路径 500ms,队列步进用更短的值保持节奏)
pub async fn deliver_item_by_id(
    app_handle: &tauri::AppHandle,
    id: i64,
    hide_first: bool,
    do_paste: bool,
    suppress_ms: u64,
) -> Result<Delivery, String> {
    let db = app_handle.state::<SqlitePool>();
    let row = sqlx::query_as::<_, (String, i32, Option<String>)>(
        "SELECT content, type, file_path FROM items WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&*db)
    .await
    .map_err(|e| e.to_string())?;

    let (content, item_type, file_path) = match row {
        Some(row) => row,
        None => return Ok(Delivery::Missing),
    };

    if hide_first {
        release_focus_to_target(app_handle).await;
    } else {
        // 焦点已在目标应用;留一点余量,避免上一轮模拟按键尚在收尾
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    }

    let monitor = app_handle.state::<crate::services::clipboard_monitor::ClipboardMonitor>();
    monitor.set_suppress(true).await;

    let mode = if do_paste {
        DeliverMode::Paste
    } else {
        DeliverMode::CopyOnly
    };
    let result =
        deliver_content(app_handle.clone(), &content, item_type, file_path.as_deref(), mode).await;

    tokio::time::sleep(std::time::Duration::from_millis(suppress_ms)).await;
    monitor.set_suppress(false).await;

    if result.is_ok() {
        if let Err(e) = sqlx::query("UPDATE items SET last_used_at = datetime('now') WHERE id = ?")
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

    result.map(|()| Delivery::Delivered)
}
