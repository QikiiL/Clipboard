#![allow(dead_code)]

use super::clipboard_type::ClipboardType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    #[serde(rename = "type")]
    pub item_type: ClipboardType,
    pub content: String,
    pub content_hash: String,
    pub file_path: Option<String>,
    pub preview: String,
    pub is_favorite: bool,
    pub group_id: Option<i64>,
    pub created_at: String,
    pub last_used_at: String,
}
