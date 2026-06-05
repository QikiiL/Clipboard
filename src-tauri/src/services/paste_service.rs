use enigo::{Enigo, Key, Direction, Keyboard};
use std::thread;
use std::time::Duration;

/// 模拟 Ctrl+V 粘贴
pub fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&enigo::Settings::default()).map_err(|e| e.to_string())?;

    enigo.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
    enigo.key(Key::Unicode('v'), Direction::Press).map_err(|e| e.to_string())?;
    enigo.key(Key::Unicode('v'), Direction::Release).map_err(|e| e.to_string())?;
    enigo.key(Key::Control, Direction::Release).map_err(|e| e.to_string())?;

    Ok(())
}

/// 将文本写入剪切板并粘贴
pub fn copy_and_paste_text(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .set_text(text.to_string())
        .map_err(|e| e.to_string())?;

    thread::sleep(Duration::from_millis(50));

    simulate_paste()
}

/// 粘贴指定内容
pub async fn paste_content(content: &str, item_type: i32, file_path: Option<&str>) -> Result<(), String> {
    match item_type {
        0 | 1 => {
            // Text or Link
            copy_and_paste_text(content)?;
        }
        2 => {
            // Image - 复制文件路径到剪切板
            let path = file_path.unwrap_or(content);
            copy_and_paste_text(path)?;
        }
        _ => {
            copy_and_paste_text(content)?;
        }
    }
    Ok(())
}
