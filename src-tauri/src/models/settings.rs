use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CloseBehavior {
    Ask,
    Minimize,
    Close,
}

impl Default for CloseBehavior {
    fn default() -> Self {
        Self::Ask
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub start_with_windows: bool,
    pub retention_days: i32,
    pub max_item_count: i32,
    pub hotkey_modifier: String,
    pub hotkey_key: String,
    pub paused: bool,
    // 兼容旧版 settings.json:后加的字段缺省时不应导致整个结构体反序列化失败
    #[serde(default)]
    pub close_behavior: CloseBehavior,
    #[serde(default)]
    pub win_v_integration: bool,
    #[serde(default = "default_pinned")]
    pub pinned: bool,
}

/// 窗口默认置顶;serde 的字段级 default 若不指定函数,bool 恒为 false,
/// 老配置文件缺 pinned 字段时会退回 false,故显式指定
fn default_pinned() -> bool {
    true
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
            close_behavior: CloseBehavior::default(),
            win_v_integration: false,
            pinned: true,
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
        assert_eq!(settings.close_behavior, CloseBehavior::Ask);
        assert!(!settings.win_v_integration);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = AppSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.retention_days, settings.retention_days);
    }

    #[test]
    fn test_close_behavior_serde() {
        assert_eq!(
            serde_json::to_string(&CloseBehavior::Ask).unwrap(),
            "\"ask\""
        );
        assert_eq!(
            serde_json::to_string(&CloseBehavior::Minimize).unwrap(),
            "\"minimize\""
        );
        assert_eq!(
            serde_json::to_string(&CloseBehavior::Close).unwrap(),
            "\"close\""
        );
        assert_eq!(
            serde_json::from_str::<CloseBehavior>("\"ask\"").unwrap(),
            CloseBehavior::Ask
        );
        assert_eq!(
            serde_json::from_str::<CloseBehavior>("\"minimize\"").unwrap(),
            CloseBehavior::Minimize
        );
        assert_eq!(
            serde_json::from_str::<CloseBehavior>("\"close\"").unwrap(),
            CloseBehavior::Close
        );
    }
}
