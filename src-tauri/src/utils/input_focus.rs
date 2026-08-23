use std::sync::Mutex;
use std::time::Duration;

#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> isize;
    fn SetForegroundWindow(hwnd: isize) -> i32;
}

/// 捕获当前前台窗口句柄。必须在本应用窗口显示之前调用,
/// 此刻前台还是目标应用,粘贴按键要发还给这个窗口。
pub fn capture_foreground_hwnd() -> isize {
    unsafe { GetForegroundWindow() }
}

/// 隐藏本应用窗口后,主动把前台焦点还给目标窗口,并等待其真正就绪。
/// SetForegroundWindow 的调用方需为前台进程或最近接收输入的进程——
/// 用户刚在我们的窗口里点击过,满足条件。
/// 返回目标窗口是否已成为前台。
pub fn restore_target_focus(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    unsafe {
        for _ in 0..25 {
            if GetForegroundWindow() == hwnd {
                return true;
            }
            SetForegroundWindow(hwnd);
            std::thread::sleep(Duration::from_millis(20));
        }
        GetForegroundWindow() == hwnd
    }
}

/// 窗口是否为当前前台窗口。tao/tauri 的 is_focused 标志基于 WM_ACTIVATE /
/// WM_SETFOCUS 消息异步更新,窗口刚创建或刚显示时会滞后;
/// 这里直接用原生 GetForegroundWindow 比对,作为判定兜底。
pub fn is_foreground_window(hwnd: isize) -> bool {
    hwnd != 0 && unsafe { GetForegroundWindow() } == hwnd
}

/// 单击条目的目标行为(true = 粘贴到之前聚焦的输入框)与目标窗口句柄。
/// 默认粘贴;"仅复制"由前端状态栏手动切换,会话内保持。
pub struct PasteMode {
    input: Mutex<bool>,
    target: Mutex<isize>,
}

impl PasteMode {
    pub fn new() -> Self {
        Self {
            input: Mutex::new(true),
            target: Mutex::new(0),
        }
    }

    pub fn get(&self) -> bool {
        *self.input.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set(&self, value: bool) {
        *self.input.lock().unwrap_or_else(|e| e.into_inner()) = value;
    }

    pub fn set_target(&self, hwnd: isize) {
        *self.target.lock().unwrap_or_else(|e| e.into_inner()) = hwnd;
    }

    pub fn target(&self) -> isize {
        *self.target.lock().unwrap_or_else(|e| e.into_inner())
    }
}
