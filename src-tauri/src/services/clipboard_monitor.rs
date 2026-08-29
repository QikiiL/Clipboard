use crate::models::clipboard_type::ClipboardType;
use crate::services::exclusion_service::{ExclusionReason, ExclusionState};
use crate::utils::hash::{compute_hash, compute_hash_bytes};
use arboard::Clipboard as ArboardClipboard;
use image::RgbaImage;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

/// Fallback: read CF_DIB from the Windows clipboard directly and convert to RGBA.
/// Returns (rgba_bytes, width, height) or None.
fn read_clipboard_dib_fallback() -> Option<(Vec<u8>, usize, usize)> {
    use clipboard_win::formats::RawData;

    let _clip = clipboard_win::Clipboard::new().ok()?;

    // CF_DIB = 8
    const CF_DIB: u32 = 8;
    if !clipboard_win::is_format_avail(CF_DIB) {
        return None;
    }

    let dib_data: Vec<u8> = clipboard_win::get(RawData(CF_DIB)).ok()?;
    if dib_data.len() < 36 {
        return None;
    }

    // Read BITMAPINFOHEADER size from first 4 bytes
    let header_size =
        u32::from_le_bytes([dib_data[0], dib_data[1], dib_data[2], dib_data[3]]) as usize;

    // Read biBitCount (offset 14) and biClrUsed (offset 32) for color table size
    let bit_count = u16::from_le_bytes([dib_data[14], dib_data[15]]);
    let clr_used =
        u32::from_le_bytes([dib_data[32], dib_data[33], dib_data[34], dib_data[35]]) as usize;
    let color_table_size = if clr_used > 0 {
        clr_used * 4
    } else if bit_count <= 8 {
        (1usize << bit_count) * 4
    } else {
        0
    };

    // Prepend a 14-byte BMP file header
    let file_size = 14u32 + dib_data.len() as u32;
    let pixel_offset = 14u32 + header_size as u32 + color_table_size as u32;
    let mut bmp_data = Vec::with_capacity(file_size as usize);
    bmp_data.extend_from_slice(b"BM");
    bmp_data.extend_from_slice(&file_size.to_le_bytes());
    bmp_data.extend_from_slice(&[0u8; 4]); // reserved
    bmp_data.extend_from_slice(&pixel_offset.to_le_bytes());
    bmp_data.extend_from_slice(&dib_data);

    let img = image::load_from_memory(&bmp_data).ok()?;
    let rgba = img.to_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
    Some((rgba.into_raw(), width, height))
}

/// Check if a string looks like a file path (Windows drive letter, Unix path, or UNC path).
fn is_file_path(s: &str) -> bool {
    // Windows drive letter (C:\...)
    (s.len() >= 3
        && s.as_bytes()[1] == b':'
        && (s.as_bytes()[2] == b'\\' || s.as_bytes()[2] == b'/'))
    // Unix path
    || s.starts_with('/')
    // UNC path (\\server\share)
    || (s.starts_with("\\\\") && s.len() > 2)
}

/// Detect content type from text content.
fn detect_content_type(text: &str) -> i32 {
    if text.starts_with("http://") || text.starts_with("https://") {
        ClipboardType::Link as i32
    } else if is_file_path(text) {
        ClipboardType::File as i32
    } else {
        ClipboardType::Text as i32
    }
}

/// 一次剪贴板抓取的结果
struct CapturedContent {
    content: String,
    item_type: i32,
    image_hash: Option<String>,
    image_bytes: Option<Vec<u8>>,
    width: usize,
    height: usize,
    /// 放置这段内容的来源进程名(小写);判定不了时为 None。
    /// 必须在打开剪贴板之前取:剪贴板是独占资源,打开后 GetClipboardOwner 会失败
    source_process: Option<String>,
}

/// Read clipboard content on a blocking thread.
/// 返回 None 表示剪贴板为空或读取失败。
fn read_clipboard_content() -> Option<CapturedContent> {
    // 来源进程要在打开剪贴板之前取(打开后 GetClipboardOwner 拿不到 owner)。
    // 这是阻塞式 Win32 调用,而本函数本就跑在 spawn_blocking 里
    let source_process = crate::services::exclusion_service::source_process_name();

    // Try reading a file list (CF_HDROP) first — copied from Explorer.
    // The clipboard_win guard must be dropped before arboard calls below,
    // as both open the clipboard exclusively.
    {
        let _clip = clipboard_win::Clipboard::new().ok()?;
        const CF_HDROP: u32 = 15;
        if clipboard_win::is_format_avail(CF_HDROP) {
            let files: Vec<String> = clipboard_win::get(clipboard_win::formats::FileList).ok()?;
            if !files.is_empty() {
                return Some(CapturedContent {
                    content: files.join("\n"),
                    item_type: ClipboardType::File as i32,
                    image_hash: None,
                    image_bytes: None,
                    width: 0,
                    height: 0,
                    source_process,
                });
            }
        }
    }

    let mut cb = ArboardClipboard::new().ok()?;

    // Try reading an image first
    match cb.get_image() {
        Ok(img) => {
            let bytes = img.bytes.to_vec();
            let width = img.width;
            let height = img.height;
            let hash = compute_hash_bytes(&bytes);
            return Some(CapturedContent {
                content: "[图片]".to_string(),
                item_type: ClipboardType::Image as i32,
                image_hash: Some(hash),
                image_bytes: Some(bytes),
                width,
                height,
                source_process,
            });
        }
        Err(_) => {
            // Try DIB fallback (Windows-specific)
            if let Some((bytes, width, height)) = read_clipboard_dib_fallback() {
                let hash = compute_hash_bytes(&bytes);
                return Some(CapturedContent {
                    content: "[图片]".to_string(),
                    item_type: ClipboardType::Image as i32,
                    image_hash: Some(hash),
                    image_bytes: Some(bytes),
                    width,
                    height,
                    source_process,
                });
            }
        }
    }

    // Try reading text
    match cb.get_text() {
        Ok(text) => {
            if !text.is_empty() {
                let item_type = detect_content_type(&text);
                Some(CapturedContent {
                    content: text,
                    item_type,
                    image_hash: None,
                    image_bytes: None,
                    width: 0,
                    height: 0,
                    source_process,
                })
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

/// 排除原因的事件标识,供前端区分提示文案
fn reason_str(reason: Option<ExclusionReason>) -> &'static str {
    match reason {
        Some(ExclusionReason::App) => "app",
        Some(ExclusionReason::Pattern) => "pattern",
        Some(ExclusionReason::Sensitive) => "sensitive",
        None => "unknown",
    }
}

/// Save image bytes as a PNG file to the app's images directory.
fn save_image_to_disk(
    app_handle: &tauri::AppHandle,
    hash: &str,
    bytes: &[u8],
    width: usize,
    height: usize,
) -> Option<String> {
    let images_dir = crate::services::storage_service::images_dir(app_handle);
    if let Err(e) = std::fs::create_dir_all(&images_dir) {
        eprintln!("Failed to create images directory: {}", e);
    }
    let file_name = format!("{}.png", &hash[..hash.len().min(16)]);
    let path = images_dir.join(&file_name);
    if let Some(img_buf) = RgbaImage::from_raw(width as u32, height as u32, bytes.to_vec()) {
        // Convert to RGB to strip alpha channel and color profile metadata.
        // This prevents the "iCCP: cHRM chunk does not match sRGB" libpng warning.
        let dynamic_img = image::DynamicImage::ImageRgba8(img_buf);
        let rgb_img = dynamic_img.to_rgb8();
        if rgb_img.save(&path).is_ok() {
            Some(path.to_string_lossy().to_string())
        } else {
            eprintln!("Failed to save image to {:?}", path);
            None
        }
    } else {
        eprintln!(
            "Failed to create RgbaImage from raw bytes: dimensions={}x{}, byte_count={}",
            width,
            height,
            bytes.len()
        );
        None
    }
}

pub struct ClipboardMonitor {
    suppress: Arc<Mutex<bool>>,
    paused: Arc<Mutex<bool>>,
    last_hash: Arc<Mutex<String>>,
}

impl ClipboardMonitor {
    pub fn new() -> Self {
        Self {
            suppress: Arc::new(Mutex::new(false)),
            paused: Arc::new(Mutex::new(false)),
            last_hash: Arc::new(Mutex::new(String::new())),
        }
    }

    pub async fn set_suppress(&self, value: bool) {
        let mut suppress = self.suppress.lock().await;
        *suppress = value;
    }

    pub async fn set_paused(&self, value: bool) {
        let mut paused = self.paused.lock().await;
        *paused = value;
    }

    pub async fn is_paused(&self) -> bool {
        *self.paused.lock().await
    }

    pub async fn toggle_paused(&self) {
        let mut paused = self.paused.lock().await;
        *paused = !*paused;
    }

    /// Start polling the system clipboard for changes.
    pub fn start_polling(app_handle: tauri::AppHandle, db: SqlitePool, monitor: ClipboardMonitor) {
        let monitor = monitor.clone();
        tauri::async_runtime::spawn(async move {
            // 规则状态在循环外取一次:state 内部是 Arc,持有它不会阻塞设置刷新,
            // 也省掉每 500ms 一次的状态查找
            let exclusion = app_handle.try_state::<ExclusionState>();

            // 启动捕获由下方轮询循环完成:tokio interval 的首次 tick 立即到期,
            // 与后续读取共用同一套去重/入库/发事件逻辑(独一份,避免双份维护),
            // 且首次捕获到新条目时也会发事件,前端首屏加载后能及时刷新出来
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                interval.tick().await;
                if *monitor.suppress.lock().await || *monitor.paused.lock().await {
                    continue;
                }

                // Read clipboard on a blocking thread to avoid arboard deadlocks
                let clipboard_result = tokio::task::spawn_blocking(read_clipboard_content)
                    .await
                    .unwrap_or(None);

                let Some(captured) = clipboard_result else {
                    continue;
                };
                let CapturedContent {
                    content,
                    item_type,
                    image_hash,
                    image_bytes,
                    width: img_width,
                    height: img_height,
                    source_process,
                } = captured;

                let hash = if let Some(ih) = image_hash {
                    ih
                } else {
                    compute_hash(&content)
                };

                // Skip if same as last
                {
                    let last = monitor.last_hash.lock().await;
                    if *last == hash {
                        continue;
                    }
                }

                // Skip if suppress/paused changed during blocking read
                if *monitor.suppress.lock().await || *monitor.paused.lock().await {
                    continue;
                }

                // 排除规则:命中则不入库,密码、密钥之类不落盘
                if let Some(state) = &exclusion {
                    let check = state.check(&content, item_type, source_process.as_deref());
                    if check.excluded {
                        // 也要把 hash 记进 last_hash:被排除的内容会一直留在系统
                        // 剪贴板里,不记的话每 500ms 都要对同一份内容重跑一遍
                        // 全部正则。代价是此后用户删掉该规则、再复制同一份内容
                        // 时不会被记录(需先复制点别的把 last_hash 冲掉),
                        // 属极边缘场景,接受
                        *monitor.last_hash.lock().await = hash.clone();
                        let _ = app_handle.emit("item-excluded", reason_str(check.reason));
                        continue;
                    }
                }

                let preview: String = content.chars().take(100).collect();

                // Save image bytes to file if it's an image
                let file_path: Option<String> = if item_type == ClipboardType::Image as i32 {
                    image_bytes.as_deref().and_then(|bytes| {
                        save_image_to_disk(&app_handle, &hash, bytes, img_width, img_height)
                    })
                } else {
                    None
                };

                // Update last hash
                {
                    let mut last = monitor.last_hash.lock().await;
                    *last = hash.clone();
                }

                // Check for existing item
                let existing = sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM items WHERE content_hash = ? LIMIT 1",
                )
                .bind(&hash)
                .fetch_optional(&db)
                .await;

                match existing {
                    Ok(Some(id)) => {
                        let _ = sqlx::query(
                            "UPDATE items SET last_used_at = datetime('now') WHERE id = ?",
                        )
                        .bind(id)
                        .execute(&db)
                        .await;
                        // Double-check suppress wasn't set during our write
                        if *monitor.suppress.lock().await {
                            continue;
                        }
                        let _ = app_handle.emit(
                            "clipboard-changed",
                            serde_json::json!({"action": "updated", "id": id}),
                        );
                    }
                    _ => {
                        let result = sqlx::query(
                            "INSERT INTO items (type, content, content_hash, file_path, preview) VALUES (?, ?, ?, ?, ?)",
                        )
                        .bind(item_type)
                        .bind(&content)
                        .bind(&hash)
                        .bind(&file_path)
                        .bind(&preview)
                        .execute(&db)
                        .await;

                        if result.is_ok() {
                            // Double-check suppress wasn't set during our write
                            if *monitor.suppress.lock().await {
                                continue;
                            }
                            let _ = app_handle
                                .emit("clipboard-changed", serde_json::json!({"action": "new"}));
                        }
                    }
                }
            }
        });
    }
}

impl Clone for ClipboardMonitor {
    fn clone(&self) -> Self {
        Self {
            suppress: Arc::clone(&self.suppress),
            paused: Arc::clone(&self.paused),
            last_hash: Arc::clone(&self.last_hash),
        }
    }
}
