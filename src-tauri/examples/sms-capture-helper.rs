//! 短信验证码 helper 进程(无窗口、不提权、asInvoker)。
//!
//! 为什么需要它(发布级 bug 的修复):主程序带 requireAdministrator 清单,
//! 而稀疏包身份只授予「Shell 以 AUMID 激活」启动的进程,ActivateApplication
//! 无法拉起提权 exe(0x80070032)。所以提权的主进程永远拿不到包身份,
//! 无法订阅 `UserNotificationListener::NotificationChanged`(无身份必报
//! 0x80070490)。
//!
//! 本进程由主程序以包 AUMID 激活(identity::activate_helper),出生即带
//! 稀疏包身份 → 订阅 NotificationChanged 成功 → 过滤手机连接 toast →
//! 提取验证码 → 直接写剪贴板。主程序的剪贴板监控线程会在 500ms 内自动
//! 把验证码记入历史,无需任何 IPC。
//!
//! 生命周期:
//! - 父进程看门狗:每 5 秒检查 --parent 传入的主程序 PID,父进程退出则本进程退出
//! - 订阅失败:等 5 秒重试,最多 3 次,仍失败则退出(主程序下次启动会重新激活)
//!
//! 日志:%TEMP%\clipboard-sms-helper.log(追加,超过 256KB 截断重写)

#![windows_subsystem = "windows"]

use std::collections::HashSet;
use std::io::Write;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::Management::UserNotificationListener;
use windows::UI::Notifications::NotificationKinds;

// 探针同款:examples 是独立编译单元,identity.rs 仅依赖 windows crate、
// 无 crate 内部引用,用 #[path] 引主 crate 的同源实现。
#[path = "../src/services/identity.rs"]
mod identity;

/// Phone Link 的 AUMID 前缀(小写比较)。与主程序 sms_code_service.rs 同源
const PHONE_LINK_AUMID_PREFIX: &str = "microsoft.yourphone";

/// 单实例互斥体的 Win32 声明:按项目约定用裸 `#[link]`(CreateMutexW 在
/// windows crate 里被 Win32_Security feature 门控,不为一个调用引入整个模块)
#[link(name = "kernel32")]
extern "system" {
    fn CreateMutexW(
        lp_mutex_attributes: *const core::ffi::c_void,
        b_initial_owner: i32,
        lp_name: *const u16,
    ) -> *mut core::ffi::c_void;
    fn GetLastError() -> u32;
}

/// ERROR_ALREADY_EXISTS:CreateMutexW 命中同名互斥体时 GetLastError 返回它
const ERROR_ALREADY_EXISTS: u32 = 183;

/// 已见通知的 (aumid, id) 集合。轮询线程独占访问即可,不必上锁
type SeenSet = HashSet<(String, u32)>;

fn main() {
    // --parent <pid>:主程序激活时传入自己的进程 ID(看门狗用)
    let parent_pid: u32 = std::env::args()
        .position(|a| a == "--parent")
        .and_then(|i| std::env::args().nth(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    log(&format!(
        "sms-helper 启动 (pid={}, parent={parent_pid})",
        std::process::id()
    ));

    // 单实例护栏:应用快速重启时,旧 helper 最多要 5 秒才被看门狗发现父进程已死,
    // 这段窗口里两个 helper 会同时订阅、把同一个验证码重复写两遍剪贴板。
    // 命名互斥体(Local\ = 当前登录会话)保证同一时刻只有一个 helper;
    // 进程退出时内核自动释放,不会像锁文件那样留下残留。
    let mutex_name: Vec<u16> = "Local\\clipboard-sms-helper\0".encode_utf16().collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 1, mutex_name.as_ptr()) };
    if handle.is_null() {
        log("单实例互斥体创建失败(忽略,继续运行)");
    } else if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        log("已有 helper 实例在运行,本实例退出(避免重复捕获)");
        return;
    }

    // COM 以 STA 初始化:RequestAccessAsync 官方要求 UI 线程(STA)调用,
    // MTA 线程上调用会直接返回 Denied(identity_probe.rs 实测结论)
    unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() {
            log(&format!("CoInitializeEx(STA) 返回 {hr:?}(可能已初始化,继续)"));
        }
    }

    // 先请求监听授权:打包应用的通知监听授权是「按包」记录的,
    // 不先 RequestAccessAsync 直接订阅会报 0x80070490(见 identity_probe.rs)
    if let Err(e) = request_access() {
        log(&format!("请求通知监听授权失败(继续尝试订阅): {e}"));
    }

    let listener = match UserNotificationListener::Current() {
        Ok(l) => l,
        Err(e) => {
            log(&format!("UserNotificationListener 不可用,退出: {e}"));
            return;
        }
    };

    // 播种:启动时先把通知中心里已有的旧验证码 toast 标记为已见、不写剪贴板,
    // 避免 helper 每次重启都把旧码重复复制、覆盖用户当前剪贴板内容。
    // 播种后的 seen 集合随参数传入 event_loop,保证去重连续性
    let mut seen: SeenSet = HashSet::new();
    match poll_once(&listener, &mut seen, false) {
        Ok(n) => log(&format!("播种完成:已见 {n} 条历史通知(不捕获)")),
        Err(e) => log(&format!("播种轮询失败(不影响后续捕获): {e}")),
    }

    // 订阅 NotificationChanged,失败重试:最多 3 次,间隔 5 秒。
    // 以包 AUMID 激活的进程自带身份,首轮就应成功;失败说明环境异常
    // (如稀疏包被卸载),重试无果则退出,主程序下次启动会重新激活
    let mut wake_rx = None;
    for attempt in 1..=3 {
        match subscribe_event(&listener) {
            Ok(rx) => {
                log("NotificationChanged 订阅成功,事件驱动已启用");
                wake_rx = Some(rx);
                break;
            }
            Err(e) => {
                log(&format!("NotificationChanged 订阅失败(第 {attempt}/3 次): {e}"));
                if attempt < 3 {
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        }
    }
    let wake_rx = match wake_rx {
        Some(rx) => rx,
        None => {
            log("连续 3 次订阅失败,退出(主程序下次启动会重新激活)");
            return;
        }
    };

    event_loop(&listener, wake_rx, parent_pid, seen);
}

/// 事件驱动 + 兜底轮询主循环:
/// - NotificationChanged 唤醒立即轮询(亚秒级)
/// - 1 秒无事件兜底轮询一次,防止事件丢失导致漏捕
/// - 每 5 秒检查父进程存活(看门狗)
fn event_loop(
    listener: &UserNotificationListener,
    wake_rx: std::sync::mpsc::Receiver<()>,
    parent_pid: u32,
    mut seen: SeenSet,
) {
    let mut last_watchdog = std::time::Instant::now();
    let started = std::time::Instant::now();

    loop {
        // 父进程看门狗:主程序退出 → helper 跟着退出,不留孤儿进程
        if parent_pid != 0 && last_watchdog.elapsed() >= Duration::from_secs(5) {
            last_watchdog = std::time::Instant::now();
            if !parent_alive(parent_pid) {
                log(&format!("父进程 (pid={parent_pid}) 已退出,helper 退出"));
                return;
            }
        }
        // 无 --parent(手工/调试启动)时看门狗不工作:60 分钟后自动退出,
        // 避免遗留孤儿进程在应用关闭后仍持续写剪贴板
        if parent_pid == 0 && started.elapsed() >= Duration::from_secs(3600) {
            log("无父进程监护已运行 60 分钟,自动退出(防孤儿驻留)");
            return;
        }

        match wake_rx.recv_timeout(Duration::from_millis(1000)) {
            Ok(()) => {
                // 突发合并:一条短信可能连弹多条 toast,先让它们到齐,
                // 再排空通道里积压的信号,合并成一次 poll(与主程序同源)
                std::thread::sleep(Duration::from_millis(250));
                while wake_rx.try_recv().is_ok() {}
                log("事件唤醒,轮询通知中心");
            }
            // 兜底轮询:超时也往下走一轮
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            // wake_tx 随 listener 常驻不会 drop;真发生时按兜底节奏继续纯轮询
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {}
        }

        // 尊重开关:主程序 settings.json 里 sms_code_enabled 为 false 则跳过
        // 捕获(文件缺失/解析失败按 true 处理,helper 侧不做二次降级)
        if !sms_enabled_from_config() {
            continue;
        }

        match poll_once(listener, &mut seen, true) {
            Ok(hits) => {
                if hits > 0 {
                    log(&format!("本轮捕获 {hits} 条验证码"));
                }
            }
            Err(e) => log(&format!("轮询失败(权限或系统服务): {e}")),
        }
    }
}

/// 一轮轮询:过滤 Phone Link toast → 提取验证码 → 写剪贴板。
/// `capture=false` 时只播种已见集合、不写剪贴板(用于启动时把通知中心里
/// 已有的旧验证码 toast 标记为已见,避免 helper 每次重启都重复复制旧码、
/// 覆盖用户当前剪贴板)。
/// 逻辑与主程序 sms_code_service::poll_once 同源(去掉前端/系统通知部分,
/// helper 无 UI),去重键 (aumid, id) + 修剪已见集合的语义保持一致
fn poll_once(
    listener: &UserNotificationListener,
    seen: &mut SeenSet,
    capture: bool,
) -> windows::core::Result<usize> {
    let notifications = listener
        .GetNotificationsAsync(NotificationKinds::Toast)?
        .get()?;

    let mut hits = 0usize;
    let mut active: SeenSet = HashSet::new();

    for note in notifications {
        let aumid = match note.AppInfo().and_then(|a| a.AppUserModelId()) {
            Ok(id) => id.to_string_lossy().to_lowercase(),
            Err(_) => continue,
        };
        if !aumid.starts_with(PHONE_LINK_AUMID_PREFIX) {
            continue;
        }
        let id = match note.Id() {
            Ok(id) => id,
            Err(_) => continue,
        };
        let key = (aumid.clone(), id);
        active.insert(key.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);

        if capture {
            if let Some((sender, body)) = extract_toast_texts(&note) {
                if let Some(code) = extract_code(&body) {
                    if write_clipboard_text(&code) {
                        log(&format!("已捕获验证码 {code}(来自 {sender})并写入剪贴板"));
                    } else {
                        log(&format!("写剪贴板失败(3 次重试后仍被占用),跳过 {code}"));
                    }
                    hits += 1;
                }
            }
        }
    }

    // 修剪:只保留通知中心里仍存在的条目(通知被清除后同 id 复用不会误判为已见)
    seen.retain(|k| active.contains(k));
    Ok(hits)
}

/// 订阅一次 NotificationChanged,返回唤醒信号接收端。
/// 回调只发信号不做实际工作(剪贴板由主循环处理),绝不能 panic
fn subscribe_event(
    listener: &UserNotificationListener,
) -> Result<std::sync::mpsc::Receiver<()>, String> {
    let (wake_tx, wake_rx) = std::sync::mpsc::channel::<()>();
    listener
        .NotificationChanged(&TypedEventHandler::new(move |_l, _a| {
            let _ = wake_tx.send(());
            Ok(())
        }))
        .map(|_| ())
        .map_err(|e| format!("{e} (HRESULT {:#010X})", e.code().0 as u32))?;
    Ok(wake_rx)
}

/// 请求通知监听授权(打包应用按包记录授权,必须先建立条目,见探针注释)
fn request_access() -> Result<(), String> {
    let listener = UserNotificationListener::Current()
        .map_err(|e| format!("UserNotificationListener::Current 失败: {e}"))?;
    let status = listener
        .RequestAccessAsync()
        .map_err(|e| format!("RequestAccessAsync 调用失败: {e}"))?
        .get()
        .map_err(|e| format!("RequestAccessAsync 等待失败: {e}"))?;
    log(&format!("RequestAccessAsync => {status:?}"));
    Ok(())
}

/// 读主程序的设置开关:<exe目录>\config\settings.json。
/// settings.json 是 tauri-plugin-store 格式,AppSettings 挂在顶层键
/// `app_settings` 下;兼容「顶层字段」与「嵌套在 app_settings 里」两种位置。
/// 文件缺失/解析失败/字段缺省一律按 true 处理(宁可多看一眼通知,不漏捕)
fn sms_enabled_from_config() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return true;
    };
    let Some(dir) = exe.parent() else {
        return true;
    };
    let path = dir.join("config").join("settings.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return true;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return true;
    };
    let flag = |x: &serde_json::Value| x.get("sms_code_enabled").and_then(|b| b.as_bool());
    match flag(&v) {
        Some(b) => b,
        None => v
            .get("app_settings")
            .and_then(flag)
            .unwrap_or(true),
    }
}

/// 父进程是否存活:OpenProcess(最小查询权限) + GetExitCodeProcess。
/// 打不开(句柄失效/权限不足)按已退出处理;STILL_ACTIVE(259) = 存活
fn parent_alive(pid: u32) -> bool {
    // windows 0.61 未导出 STILL_ACTIVE(伪退出码),按 MSDN 值本地定义
    const STILL_ACTIVE: u32 = 259;
    unsafe {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code: u32 = 0;
        let alive = GetExitCodeProcess(h, &mut code).is_ok() && code == STILL_ACTIVE;
        let _ = CloseHandle(h);
        alive
    }
}

/// 写剪贴板(带 3 次重试)。与主程序 sms_code_service::write_clipboard_text
/// 同源:主程序监控线程每 500ms 也在开关剪贴板,偶发占用冲突
fn write_clipboard_text(text: &str) -> bool {
    for _ in 0..3 {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if cb.set_text(text.to_string()).is_ok() {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(60));
    }
    false
}

// ---------------------------------------------------------------------------
// 日志:追加写 %TEMP%\clipboard-sms-helper.log,超过 256KB 截断重写
// ---------------------------------------------------------------------------

const LOG_NAME: &str = "clipboard-sms-helper.log";
const LOG_LIMIT: u64 = 256 * 1024;

fn log(msg: &str) {
    let path = std::env::temp_dir().join(LOG_NAME);
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > LOG_LIMIT {
            let _ = std::fs::write(&path, ""); // 循环限长:截断重写
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "[{}] {}", timestamp(), msg);
    }
}

/// UTC 时间戳(不引 chrono,手写 days-from-civil 逆变换)
fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Howard Hinnant civil_from_days
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mth <= 2);
    format!("{y:04}-{mth:02}-{d:02} {h:02}:{m:02}:{s:02} UTC")
}

// ---------------------------------------------------------------------------
// 验证码提取(纯函数)。以下全部与主程序 sms_code_service.rs 同源复制,
// examples 是独立编译单元,不能 include 主 crate 模块;两处须同步维护
// ---------------------------------------------------------------------------

/// 关键词(小写匹配)。没有关键词的纯数字短信(账单/流量/物流)直接放弃
const CODE_KEYWORDS: &[&str] = &[
    "验证码", "校验码", "动态码", "动态密码", "识别码", "verification", "code", "otp",
];

/// 负向关键词:紧邻这些词的数字是卡号/账号尾号,不是验证码
const NEGATIVE_PREFIXES: &[&str] = &["尾号", "卡号", "末四位", "账号", "单号", "工号"];

/// 从短信正文中提取验证码。返回 None 表示不是验证码短信或拿不准。
/// 原则:宁缺毋滥(提取错误会覆盖用户剪贴板)
fn extract_code(body: &str) -> Option<String> {
    let lower = body.to_lowercase();
    if !CODE_KEYWORDS.iter().any(|k| lower.contains(k)) {
        return None;
    }

    // 归一化分组数字:「163-882」「9 1 2 8」→「163882」「9128」
    let mut normalized = body.to_string();
    for _ in 0..3 {
        let next = merge_grouped_digits(&normalized);
        if next == normalized {
            break;
        }
        normalized = next;
    }

    let chars: Vec<char> = normalized.chars().collect();
    let mut best: Option<(i32, String)> = None;
    let mut i = 0usize;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let digits: String = chars[start..i].iter().collect();
        let len = digits.len();
        if !(4..=8).contains(&len) {
            continue;
        }
        // 8 位的 YYYYMMDD 日期不是验证码
        if len == 8 && looks_like_date(&digits) {
            continue;
        }
        // 前文 3 个字符内的负向关键词 → 那是尾号不是验证码
        let prefix: String = chars[start.saturating_sub(3)..start].iter().collect();
        if NEGATIVE_PREFIXES.iter().any(|p| prefix.contains(p)) {
            continue;
        }
        if let Some(score) = score_candidate(&normalized, start, len) {
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((score, digits));
            }
        }
    }
    best.filter(|(score, _)| *score >= 5).map(|(_, code)| code)
}

/// 计算候选数字串的得分。返回 None = 一票否决
fn score_candidate(text: &str, digit_start: usize, len: usize) -> Option<i32> {
    let mut score = match len {
        4..=6 => 3,
        7..=8 => 1,
        _ => return None,
    };

    let lower = text.to_lowercase();
    let mut min_dist = usize::MAX;
    for keyword in CODE_KEYWORDS {
        let mut search_from = 0usize;
        while let Some(found) = lower[search_from..].find(keyword) {
            let byte_pos = search_from + found;
            let char_pos = lower[..byte_pos].chars().count();
            let dist = char_pos.abs_diff(digit_start);
            if dist < min_dist {
                min_dist = dist;
            }
            search_from = byte_pos + keyword.len();
        }
    }
    if min_dist == usize::MAX {
        return None;
    }

    score += match min_dist {
        0..=15 => 4,
        16..=30 => 2,
        _ => 0,
    };
    Some(score)
}

/// 合并「数字+单个空格/短横线+数字」的分组:返回新字符串
fn merge_grouped_digits(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit()
            && i + 2 < chars.len()
            && (chars[i + 1] == ' ' || chars[i + 1] == '-')
            && chars[i + 2].is_ascii_digit()
        {
            out.push(c);
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// 判断 8 位数字是否形如 YYYYMMDD 日期
fn looks_like_date(digits: &str) -> bool {
    let b = digits.as_bytes();
    if b.len() != 8 || !b.iter().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let year: u16 = digits[0..4].parse().unwrap_or(0);
    let month: u8 = digits[4..6].parse().unwrap_or(0);
    let day: u8 = digits[6..8].parse().unwrap_or(0);
    (1900..=2100).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// 从 toast 通知提取 (发送者, 正文)。text elements 约定:第 0 个是标题
fn extract_toast_texts(
    note: &windows::UI::Notifications::UserNotification,
) -> Option<(String, String)> {
    let notification = note.Notification().ok()?;
    let visual = notification.Visual().ok()?;
    let bindings = visual.Bindings().ok()?;

    let mut parts: Vec<String> = Vec::new();
    for binding in bindings {
        // 单个 binding 取不到文本只跳过它,不能因此丢掉整条通知
        let Ok(elements) = binding.GetTextElements() else {
            continue;
        };
        for element in elements {
            if let Ok(text) = element.Text() {
                let text = text.to_string_lossy().trim().to_string();
                // 多 binding 的重复文本去重,避免验证码被拼进正文两次
                if !text.is_empty() && !parts.contains(&text) {
                    parts.push(text);
                }
            }
        }
    }
    if parts.is_empty() {
        return None;
    }
    let sender = parts.remove(0);
    Some((sender, parts.join(" ")))
}
