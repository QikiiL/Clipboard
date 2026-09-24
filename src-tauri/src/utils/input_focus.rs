use std::sync::Mutex;
use std::time::Duration;

#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> isize;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn GetWindow(hwnd: isize, cmd: u32) -> isize;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsWindowEnabled(hwnd: isize) -> i32;
    fn GetWindowLongW(hwnd: isize, nindex: i32) -> i32;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(hwnd: isize, attr: u32, out: *mut u32, size: u32) -> i32;
}

const GW_HWNDNEXT: u32 = 2;
const DWMWA_CLOAKED: u32 = 14;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOPMOST: i32 = 0x0000_0008;

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

/// 找出本窗口若被销毁时 Windows 会隐式激活的窗口:Z 序上本窗口下方第一个
/// 可激活的**普通层**窗口。找不到返回 0。
/// 「粘贴后保持打开」在唤出前没有可靠目标记录(应用启动即建窗、托盘打开)
/// 或记录的目标已失效时,用它作为粘贴落点,对齐销毁路径的隐式激活结果。
/// 必须跳过两类窗口,否则焦点会切到一个粘不进东西的目标上:
/// - 置顶窗口(WS_EX_TOPMOST):面板自身默认置顶,悬在置顶层,紧挨其下的
///   是任务栏等置顶壳窗口——粘贴目标(用户正在用的应用)在普通层;
/// - UWP 幽灵窗口:IsWindowVisible 为真但被 DWM 遮蔽(cloaked)。
pub fn find_window_below(our_hwnd: isize) -> isize {
    unsafe {
        let mut hwnd = GetWindow(our_hwnd, GW_HWNDNEXT);
        while hwnd != 0 {
            let topmost = GetWindowLongW(hwnd, GWL_EXSTYLE) & WS_EX_TOPMOST != 0;
            if !topmost {
                let mut cloaked: u32 = 0;
                let is_cloaked = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked, 4) == 0
                    && cloaked != 0;
                if !is_cloaked && IsWindowVisible(hwnd) != 0 && IsWindowEnabled(hwnd) != 0 {
                    return hwnd;
                }
            }
            hwnd = GetWindow(hwnd, GW_HWNDNEXT);
        }
        0
    }
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
