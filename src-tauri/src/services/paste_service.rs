use crate::models::clipboard_type::ClipboardType;
use enigo::{Direction, Enigo, Key, Keyboard};
use std::thread;
use std::time::Duration;

/// 单击条目的目标行为
pub enum DeliverMode {
    /// 写入剪贴板后模拟 Ctrl+V,粘贴到之前聚焦的输入框
    Paste,
    /// 仅写入剪贴板,不模拟按键
    CopyOnly,
}

/// 模拟 Ctrl+V 粘贴
pub fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&enigo::Settings::default()).map_err(|e| e.to_string())?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| e.to_string())?;
    enigo
        .key(Key::Unicode('v'), Direction::Press)
        .map_err(|e| e.to_string())?;
    enigo
        .key(Key::Unicode('v'), Direction::Release)
        .map_err(|e| e.to_string())?;
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| e.to_string())?;

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

/// 将图片写入剪切板
fn write_image_to_clipboard(file_path: &str) -> Result<(), String> {
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
                let path = file_path.as_deref().unwrap_or(&content);
                write_image_to_clipboard(path)?;
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
