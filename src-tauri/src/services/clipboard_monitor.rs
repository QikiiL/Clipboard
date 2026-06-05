use crate::utils::hash::{compute_hash, compute_hash_bytes};
use arboard::Clipboard as ArboardClipboard;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

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
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                interval.tick().await;
                if *monitor.suppress.lock().await || *monitor.paused.lock().await {
                    continue;
                }

                // Read clipboard on a blocking thread to avoid arboard deadlocks
                let clipboard_result =
                    tokio::task::spawn_blocking(|| -> Option<(String, i32, Option<String>, Option<Vec<u8>>)> {
                        let mut cb = ArboardClipboard::new().ok()?;
                        // Try image first
                        if let Ok(img) = cb.get_image() {
                            let bytes = img.bytes.to_vec();
                            let hash = compute_hash_bytes(&bytes);
                            return Some(("[图片]".to_string(), 2, Some(hash), Some(bytes)));
                        }
                        // Try text
                        if let Ok(text) = cb.get_text() {
                            if !text.is_empty() {
                                let item_type =
                                    if text.starts_with("http://") || text.starts_with("https://") {
                                        1
                                    } else {
                                        0
                                    };
                                return Some((text, item_type, None, None));
                            }
                        }
                        None
                    })
                    .await
                    .unwrap_or(None);

                let Some((content, item_type, image_hash, image_bytes)) = clipboard_result else {
                    continue;
                };

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

                let preview = if content.chars().count() > 100 {
                    content.chars().take(100).collect::<String>()
                } else {
                    content.clone()
                };

                // Save image bytes to file if it's an image
                let file_path: Option<String> = if item_type == 2 {
                    if let Some(bytes) = image_bytes {
                        let images_dir = app_handle
                            .path()
                            .app_data_dir()
                            .unwrap_or_default()
                            .join("images");
                        if let Err(e) = std::fs::create_dir_all(&images_dir) {
                            eprintln!("Failed to create images directory: {}", e);
                        }
                        let file_name = format!("{}.png", &hash[..16]);
                        let path = images_dir.join(&file_name);
                        if std::fs::write(&path, &bytes).is_ok() {
                            Some(path.to_string_lossy().to_string())
                        } else {
                            eprintln!("Failed to save image to {:?}", path);
                            None
                        }
                    } else {
                        None
                    }
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
                            "UPDATE items SET copy_count = copy_count + 1, last_used_at = datetime('now') WHERE id = ?",
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
                            let _ = app_handle.emit(
                                "clipboard-changed",
                                serde_json::json!({"action": "new"}),
                            );
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
