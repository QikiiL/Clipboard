use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub start_with_windows: bool,
    pub retention_days: i32,
    pub max_item_count: i32,
    pub hotkey_modifier: String,
    pub hotkey_key: String,
    pub paused: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            retention_days: 30,
            max_item_count: 500,
            hotkey_modifier: "Ctrl+Shift".to_string(),
            hotkey_key: "V".to_string(),
            paused: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.retention_days, 30);
        assert_eq!(settings.max_item_count, 500);
        assert!(!settings.paused);
    }
}
