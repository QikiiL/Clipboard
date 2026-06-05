use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardGroup {
    pub id: i64,
    pub name: String,
    pub icon: String,
    pub sort_order: i32,
    pub created_at: String,
}
