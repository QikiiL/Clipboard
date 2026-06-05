use crate::utils::hash::compute_hash;
use crate::utils::debounce::Debouncer;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub struct ClipboardMonitor {
    suppress: Arc<Mutex<bool>>,
    paused: Arc<Mutex<bool>>,
    debouncer: Debouncer,
}

impl ClipboardMonitor {
    pub fn new() -> Self {
        Self {
            suppress: Arc::new(Mutex::new(false)),
            paused: Arc::new(Mutex::new(false)),
            debouncer: Debouncer::new(100),
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

    pub async fn handle_clipboard_change(
        &self,
        app_handle: tauri::AppHandle,
        content: String,
        item_type: i32,
        file_path: Option<String>,
    ) {
        if *self.suppress.lock().await || *self.paused.lock().await {
            return;
        }

        let suppress = Arc::clone(&self.suppress);
        let paused = Arc::clone(&self.paused);

        self.debouncer.debounce(async move {
            if *suppress.lock().await || *paused.lock().await {
                return;
            }

            let hash = compute_hash(&content);
            let preview = if content.len() > 100 {
                content[..100].to_string()
            } else {
                content.clone()
            };

            let db = match tauri_plugin_sql::DbPool::get(&app_handle, "sqlite:clipboard.db").await {
                Ok(db) => db,
                Err(e) => {
                    eprintln!("Failed to get DB: {}", e);
                    return;
                }
            };

            let existing = db
                .select(
                    "SELECT id, copy_count FROM items WHERE content_hash = ? LIMIT 1",
                    vec![serde_json::Value::String(hash.clone())],
                )
                .await;

            match existing {
                Ok(rows) if !rows.is_empty() => {
                    let id = rows[0].get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                    let _ = db
                        .execute(
                            "UPDATE items SET copy_count = copy_count + 1, last_used_at = datetime('now') WHERE id = ?",
                            vec![serde_json::Value::Number(id.into())],
                        )
                        .await;
                    let _ = app_handle.emit("clipboard-changed", serde_json::json!({
                        "action": "updated",
                        "id": id,
                    }));
                }
                _ => {
                    let _ = db
                        .execute(
                            "INSERT INTO items (type, content, content_hash, file_path, preview) VALUES (?, ?, ?, ?, ?)",
                            vec![
                                serde_json::Value::Number(item_type.into()),
                                serde_json::Value::String(content),
                                serde_json::Value::String(hash),
                                file_path.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                                serde_json::Value::String(preview),
                            ],
                        )
                        .await;
                    let _ = app_handle.emit("clipboard-changed", serde_json::json!({
                        "action": "new",
                    }));
                }
            }
        }).await;
    }
}

impl Clone for ClipboardMonitor {
    fn clone(&self) -> Self {
        Self {
            suppress: Arc::clone(&self.suppress),
            paused: Arc::clone(&self.paused),
            debouncer: self.debouncer.clone(),
        }
    }
}
