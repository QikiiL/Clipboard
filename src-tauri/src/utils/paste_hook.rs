//! 低级键盘钩子(WH_KEYBOARD_LL):连续粘贴期间拦截物理触发热键(默认 Ctrl+V)。
//!
//! 为什么不用 RegisterHotKey 全局热键:全局热键会拦截系统里**一切**目标组合键
//! ——包括我们自己为投递而模拟的 Ctrl+V(只能靠"投递期间注销热键"绕开,
//! 而热键真空期内用户的快速连按又会裸落到目标应用:轻则重复粘贴,重则因为
//! 模拟的 Ctrl-up 放掉了用户还按着的 Ctrl,把后续按键打成字母 v)。
//! 低级钩子能区分注入事件(LLKHF_INJECTED):物理按下 → 吞掉并步进;
//! 我们模拟的 → 原样放行。快速连按全部被捕获、排队消化,不丢不重。
//!
//! 线程模型:SetWindowsHookExW 装在独立线程上,LL 钩子要求该线程有消息泵;
//! 卸载用 PostThreadMessageW(WM_QUIT) 让泵退出后 UnhookWindowsHookEx。
//! 钩子回调必须极快:只读按键结构 + 查修饰键状态 + 唤起异步步进,立即返回。

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use tauri::AppHandle;

const WH_KEYBOARD_LL: i32 = 13;
const WM_KEYDOWN: usize = 0x0100;
const WM_SYSKEYDOWN: usize = 0x0104;
const WM_QUIT: u32 = 0x0012;
const LLKHF_INJECTED: u32 = 0x10;

/// 触发键去抖:按住不放的系统自动重复(~30ms/次)不得连续触发
const DEBOUNCE_MS: u32 = 180;
/// 待办步进上限:防疯狂连按把计数器顶上天(正常永远到不了)
pub const MAX_PENDING: usize = 500;

type HOOKPROC = Option<unsafe extern "system" fn(i32, usize, isize) -> isize>;

#[repr(C)]
struct KbdLlHookStruct {
    vk_code: u32,
    scan_code: u32,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[repr(C)]
struct Msg {
    hwnd: isize,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt_x: i32,
    pt_y: i32,
    l_private: i32,
}

impl Msg {
    fn zeroed() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[link(name = "user32")]
extern "system" {
    fn SetWindowsHookExW(id_hook: i32, lpfn: HOOKPROC, hmod: isize, thread_id: u32) -> isize;
    fn UnhookWindowsHookEx(hhk: isize) -> bool;
    fn CallNextHookEx(hhk: isize, ncode: i32, wparam: usize, lparam: isize) -> isize;
    fn GetMessageW(lpmsg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
    fn PostThreadMessageW(thread_id: u32, msg: u32, wparam: usize, lparam: isize) -> bool;
    fn GetCurrentThreadId() -> u32;
    fn GetAsyncKeyState(v_key: i32) -> i16;
    fn GetTickCount() -> u32;
    fn GetModuleHandleW(lp_module_name: *const u16) -> isize;
}

// 修饰键位掩码(GetAsyncKeyState 的 VK:Ctrl=0x11 Shift=0x10 Alt=0x12 Win=0x5B/0x5C)
pub const MOD_CTRL: u32 = 1;
pub const MOD_SHIFT: u32 = 2;
pub const MOD_ALT: u32 = 4;
pub const MOD_WIN: u32 = 8;

static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static CFG_MODS: AtomicU32 = AtomicU32::new(MOD_CTRL);
static CFG_KEY_VK: AtomicU32 = AtomicU32::new(0);
static LAST_TRIGGER_TICK: AtomicU32 = AtomicU32::new(0);
static TRIGGER_APP: OnceLock<AppHandle> = OnceLock::new();

/// 解析设置里的「修饰键+按键」为钩子用的位掩码与 VK 码。
/// 不允许裸键(没有修饰键):那会吞掉所有正常打字
pub fn parse_trigger(modifier: &str, key: &str) -> Option<(u32, u32)> {
    let lower = modifier.to_lowercase();
    let mut mods = 0u32;
    if lower.contains("ctrl") {
        mods |= MOD_CTRL;
    }
    if lower.contains("shift") {
        mods |= MOD_SHIFT;
    }
    if lower.contains("alt") {
        mods |= MOD_ALT;
    }
    if lower.contains("super") || lower.contains("win") {
        mods |= MOD_WIN;
    }
    if mods == 0 {
        return None;
    }
    let key = key.trim().to_uppercase();
    let chars = key.chars().collect::<Vec<_>>();
    if chars.len() != 1 {
        return None;
    }
    let ch = chars[0];
    let vk = match ch {
        'A'..='Z' => 0x41 + (ch as u32 - 'A' as u32),
        '0'..='9' => 0x30 + (ch as u32 - '0' as u32),
        _ => return None,
    };
    Some((mods, vk))
}

/// 武装钩子(已安装则仅更新触发组合)。阻塞等待安装结果,最多 2 秒。
/// LL 钩子没有"组合键被占用"的概念,失败基本只剩系统资源异常
pub fn install(app: AppHandle, mods: u32, key_vk: u32) -> bool {
    let _ = TRIGGER_APP.set(app);
    CFG_MODS.store(mods, Ordering::SeqCst);
    CFG_KEY_VK.store(key_vk, Ordering::SeqCst);
    if INSTALLED.load(Ordering::SeqCst) {
        return true;
    }

    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    std::thread::spawn(move || {
        let hmod = unsafe { GetModuleHandleW(std::ptr::null()) };
        let hhook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hmod, 0) };
        HOOK_THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
        HOOK_HANDLE.store(hhook, Ordering::SeqCst);
        let ok = hhook != 0;
        INSTALLED.store(ok, Ordering::SeqCst);
        let _ = tx.send(ok);
        if !ok {
            eprintln!("安装连续粘贴键盘钩子失败");
            return;
        }
        // 消息泵:LL 钩子回调在该线程收到按键事件,GetMessage 阻塞等待;
        // 收到 WM_QUIT(uninstall)后退出循环并卸载钩子
        let mut msg = Msg::zeroed();
        while unsafe { GetMessageW(&mut msg, 0, 0, 0) } > 0 {}
        unsafe { UnhookWindowsHookEx(hhook) };
        HOOK_HANDLE.store(0, Ordering::SeqCst);
        INSTALLED.store(false, Ordering::SeqCst);
    });
    rx.recv_timeout(Duration::from_secs(2)).unwrap_or(false)
}

/// 卸载钩子(幂等)。向钩子线程投递 WM_QUIT,由线程自己完成卸载
pub fn uninstall() {
    if !INSTALLED.swap(false, Ordering::SeqCst) {
        return;
    }
    let thread_id = HOOK_THREAD_ID.load(Ordering::SeqCst);
    if thread_id != 0 {
        unsafe { PostThreadMessageW(thread_id, WM_QUIT, 0, 0) };
    }
}

pub fn is_installed() -> bool {
    INSTALLED.load(Ordering::SeqCst)
}

/// 要求的修饰键必须全部按下,未要求的必须全部未按下:
/// 触发 Ctrl+V 时不抢 Ctrl+Shift+V(唤出面板热键)之类的组合
fn modifiers_match() -> bool {
    let want = CFG_MODS.load(Ordering::SeqCst);
    let checks: [(u32, i32, i32); 4] = [
        (MOD_CTRL, 0x11, 0),
        (MOD_SHIFT, 0x10, 0),
        (MOD_ALT, 0x12, 0),
        (MOD_WIN, 0x5B, 0x5C),
    ];
    for (bit, vk_a, vk_b) in checks {
        let down = (unsafe { GetAsyncKeyState(vk_a) } as u16) & 0x8000 != 0
            || (vk_b != 0 && (unsafe { GetAsyncKeyState(vk_b) } as u16) & 0x8000 != 0);
        if ((want & bit) != 0) != down {
            return false;
        }
    }
    true
}

unsafe extern "system" fn hook_proc(ncode: i32, wparam: usize, lparam: isize) -> isize {
    if ncode >= 0 && (wparam == WM_KEYDOWN || wparam == WM_SYSKEYDOWN) {
        let ll = &*(lparam as *const KbdLlHookStruct);
        // 只拦物理按键:我们自己模拟的粘贴(注入事件)必须原样放行,
        // 否则目标应用永远收不到 Ctrl+V
        if ll.flags & LLKHF_INJECTED == 0
            && ll.vk_code == CFG_KEY_VK.load(Ordering::SeqCst)
            && modifiers_match()
        {
            let now = unsafe { GetTickCount() };
            let last = LAST_TRIGGER_TICK.swap(now, Ordering::SeqCst);
            let debounced =
                last != 0 && now.wrapping_sub(last) < DEBOUNCE_MS;
            if !debounced {
                if let Some(app) = TRIGGER_APP.get() {
                    crate::services::continuous_paste::on_physical_trigger(app);
                }
            }
            // 无论是否去抖,物理触发键都吞掉:按住不放的自动重复不许打进输入框
            return 1;
        }
    }
    unsafe { CallNextHookEx(0, ncode, wparam, lparam) }
}

/// 模拟按键前检查用户是否正物理按着 Ctrl
pub fn is_ctrl_physically_down() -> bool {
    let state = unsafe { GetAsyncKeyState(0x11) } as u16;
    state & 0x8000 != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ctrl_v() {
        assert_eq!(parse_trigger("Ctrl", "V"), Some((MOD_CTRL, 0x56)));
        assert_eq!(parse_trigger("Ctrl+Alt", "V"), Some((MOD_CTRL | MOD_ALT, 0x56)));
        assert_eq!(parse_trigger("Ctrl+Shift", "P"), Some((MOD_CTRL | MOD_SHIFT, 0x50)));
        assert_eq!(parse_trigger("", "V"), None, "裸键不允许:会吞掉正常打字");
        assert_eq!(parse_trigger("Ctrl", "F5"), None, "仅支持字母/数字键");
    }
}
