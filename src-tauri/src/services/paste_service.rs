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

/// 将图片写入剪切板并粘贴
pub fn copy_and_paste_image(file_path: &str) -> Result<(), String> {
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

    thread::sleep(Duration::from_millis(50));

    simulate_paste()
}

/// 粘贴指定内容
pub async fn paste_content(content: &str, item_type: i32, file_path: Option<&str>) -> Result<(), String> {
    let content = content.to_string();
    let file_path = file_path.map(|s| s.to_string());
    tokio::task::spawn_blocking(move || {
        match item_type {
            0 | 1 => copy_and_paste_text(&content),
            2 => {
                let path = file_path.as_deref().unwrap_or(&content);
                copy_and_paste_image(path)
            }
            _ => copy_and_paste_text(&content),
        }
    }).await.map_err(|e| e.to_string())?
}
