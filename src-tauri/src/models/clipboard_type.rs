#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ClipboardType {
    Text = 0,
    Link = 1,
    Image = 2,
    File = 3,
}

impl ClipboardType {
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Text,
            1 => Self::Link,
            2 => Self::Image,
            3 => Self::File,
            _ => Self::Text,
        }
    }

    pub fn to_i32(self) -> i32 {
        self as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_i32() {
        assert_eq!(ClipboardType::from_i32(0), ClipboardType::Text);
        assert_eq!(ClipboardType::from_i32(1), ClipboardType::Link);
        assert_eq!(ClipboardType::from_i32(2), ClipboardType::Image);
        assert_eq!(ClipboardType::from_i32(3), ClipboardType::File);
        assert_eq!(ClipboardType::from_i32(99), ClipboardType::Text);
    }

    #[test]
    fn test_to_i32() {
        assert_eq!(ClipboardType::Text.to_i32(), 0);
        assert_eq!(ClipboardType::Link.to_i32(), 1);
        assert_eq!(ClipboardType::Image.to_i32(), 2);
        assert_eq!(ClipboardType::File.to_i32(), 3);
    }
}
