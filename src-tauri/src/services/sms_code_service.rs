//! 短信验证码自动捕获。
//!
//! 实现方式(逆向调研 TeleLink 后采用的同款路径):
//! 手机收到短信 → 「链接至 Windows」转发 → Phone Link 弹 Windows toast →
//! 通知中心 → [本服务] UserNotificationListener 轮询 → 过滤出
//! Phone Link 的通知 → 提取验证码 → 写剪贴板。
//!
//! 安卓与 iPhone 走同一条路径(都汇到 Phone Link 的 toast),无需手机侧
//! 安装任何第三方 App,数据全程本地。
//!
//! 两个关键设计(照抄 TeleLink 的成熟经验):
//! 1. 按 AUMID 过滤(`microsoft.yourphone` 前缀)而非窗口标题——
//!    AUMID 语言无关,显示名会随系统语言变(Phone Link / 手机连接)
//! 2. 去重键 (aumid, notification_id),且每轮用「通知中心当前仍存在的
//!    id 集合」修剪已见集合——通知被清除后同 id 复用不会误判为已见

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Duration;

use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;
use windows::UI::Notifications::Management::UserNotificationListener;
use windows::UI::Notifications::Management::UserNotificationListenerAccessStatus;
use windows::UI::Notifications::NotificationKinds;

/// 功能总开关。轮询线程常驻(与剪贴板监控同生命周期),靠这个标志决定
/// 每轮是否真正去读通知——关闭时零 WinRT 调用,不占资源
static ENABLED: AtomicBool = AtomicBool::new(false);

/// 最近一次成功捕获的 Unix 时间戳(秒),给设置面板显示「最近捕获」,
/// 让用户能感知功能是否真的在工作(Phone Link 断连时不会报错,只会静默)
static LAST_CAPTURE: AtomicI64 = AtomicI64::new(0);

/// 捕获总次数(本次会话内),同上用于状态展示
static CAPTURE_COUNT: AtomicI64 = AtomicI64::new(0);

/// 已见通知的 (aumid, id) 集合。轮询线程独占访问即可,不必上锁
type SeenSet = HashSet<(String, u32)>;

/// Phone Link 的 AUMID 前缀(小写比较)。Phone Link 包名是
/// Microsoft.YourPhone_8wekyb3d8bbwe,toast 的 AUMID 形如
/// `Microsoft.YourPhone_8wekyb3d8bbwe!YourPhone`,前缀匹配即可覆盖
const PHONE_LINK_AUMID_PREFIX: &str = "microsoft.yourphone";

pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn last_capture() -> i64 {
    LAST_CAPTURE.load(Ordering::Relaxed)
}

pub fn capture_count() -> i64 {
    CAPTURE_COUNT.load(Ordering::Relaxed)
}

/// 当前通知监听权限状态。字符串直接给前端:
/// allowed / denied / unspecified / unsupported(系统不支持该 API)
pub fn access_status() -> String {
    match UserNotificationListener::Current() {
        Ok(listener) => match listener.GetAccessStatus() {
            Ok(UserNotificationListenerAccessStatus::Allowed) => "allowed".to_string(),
            Ok(UserNotificationListenerAccessStatus::Denied) => "denied".to_string(),
            _ => "unspecified".to_string(),
        },
        Err(_) => "unsupported".to_string(),
    }
}

/// 请求通知监听权限(阻塞调用,命令层须放 spawn_blocking)。
/// 非打包(non-MSIX)桌面应用调用它通常不弹系统对话框,作用是把本应用
/// 注册进「设置>隐私和安全性>通知」的应用列表;真正放行仍需用户在
/// 设置里手动允许(TeleLink 的 SETUP 文档记录了同样的坑)
pub fn request_access_blocking() -> String {
    match UserNotificationListener::Current() {
        Ok(listener) => match listener.RequestAccessAsync() {
            Ok(op) => match op.get() {
                Ok(UserNotificationListenerAccessStatus::Allowed) => "allowed".to_string(),
                Ok(UserNotificationListenerAccessStatus::Denied) => "denied".to_string(),
                _ => "unspecified".to_string(),
            },
            Err(_) => "unspecified".to_string(),
        },
        Err(_) => "unsupported".to_string(),
    }
}

/// 启动轮询线程。线程常驻,失败只记日志不崩溃——权限被用户在系统设置里
/// 掐掉、OS 拒绝服务等都按「本轮拿不到」处理,下一轮重试
pub fn spawn(app_handle: tauri::AppHandle) {
    let builder = std::thread::Builder::new().name("sms-code".into());
    let _ = builder.spawn(move || {
        let listener = match UserNotificationListener::Current() {
            Ok(l) => l,
            Err(e) => {
                // Windows 10 早期版本没有 UserNotificationListener,功能整体不可用
                eprintln!("[sms-code] UserNotificationListener 不可用: {}", e);
                return;
            }
        };

        let mut seen: SeenSet = HashSet::new();
        let mut idle_rounds: u32 = 0;

        loop {
            let mut delay = Duration::from_secs(2);
            if is_enabled() {
                match poll_once(&listener, &mut seen, &app_handle) {
                    Ok(hits) => {
                        if hits > 0 {
                            idle_rounds = 0;
                        } else {
                            idle_rounds += 1;
                        }
                    }
                    Err(e) => {
                        // 常见原因:权限被拒/被撤。退避到 5s,避免刷日志
                        eprintln!("[sms-code] 轮询失败(权限或系统服务): {}", e);
                        delay = Duration::from_secs(5);
                    }
                }
            } else {
                idle_rounds += 1;
            }
            // 空闲退避:连续无收获时拉长间隔,与 TeleLink 一致(2s → 4s)
            if idle_rounds > 3 {
                delay = delay.max(Duration::from_secs(4));
            }
            std::thread::sleep(delay);
        }
    });
}

/// 一轮轮询。返回本轮新处理的验证码条数。
/// 已见集合按 TeleLink 的方式修剪:通知从通知中心消失后,同 id 的
/// 新通知(Windows 会复用小整数 id)不应被误判为已见
fn poll_once(
    listener: &UserNotificationListener,
    seen: &mut SeenSet,
    app_handle: &tauri::AppHandle,
) -> windows::core::Result<usize> {
    let notifications = listener
        .GetNotificationsAsync(NotificationKinds::Toast)?
        .get()?;

    let mut hits = 0usize;
    let mut active: HashSet<(String, u32)> = HashSet::new();

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

        if let Some((sender, body)) = extract_toast_texts(&note) {
            if let Some(code) = extract_code(&body) {
                hits += 1;
                handle_code(app_handle, &code, &sender);
            }
        }
    }

    // 修剪:只保留通知中心里仍存在的条目
    seen.retain(|k| active.contains(k));
    Ok(hits)
}

/// 从 toast 通知提取 (发送者, 正文)。
/// text elements 约定:第 0 个是标题(发送者名或 App 名),其余是正文
fn extract_toast_texts(
    note: &windows::UI::Notifications::UserNotification,
) -> Option<(String, String)> {
    let notification = note.Notification().ok()?;
    let visual = notification.Visual().ok()?;
    let bindings = visual.Bindings().ok()?;

    let mut parts: Vec<String> = Vec::new();
    for binding in bindings {
        let elements = binding.GetTextElements().ok()?;
        for element in elements {
            if let Ok(text) = element.Text() {
                let text = text.to_string_lossy().trim().to_string();
                if !text.is_empty() {
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

/// 命中验证码:写剪贴板 + 前端提示 + 失焦系统通知。
///
/// 写剪贴板**不设 suppress**:剪贴板监控会把验证码当作普通内容自动记进
/// 历史(500ms 内),历史列表里直接能翻到——这是刻意设计的零成本集成
fn handle_code(app_handle: &tauri::AppHandle, code: &str, sender: &str) {
    if !write_clipboard_text(code) {
        eprintln!("[sms-code] 写剪贴板失败(重试后仍被占用),跳过 {}", code);
        return;
    }

    LAST_CAPTURE.store(now_unix(), Ordering::Relaxed);
    CAPTURE_COUNT.fetch_add(1, Ordering::Relaxed);

    // 前端居中提示(窗口存在且前端在监听时可见)
    let _ = app_handle.emit(
        "sms-code-copied",
        serde_json::json!({ "code": code, "sender": sender }),
    );

    // 主窗口未获焦(或已销毁)时发系统通知——用户多半正在别的窗口里
    // 等着粘贴验证码,通知比应用内 toast 更能到达视野。
    // 与排除提示同一守门逻辑,避免获焦时双重打扰
    use tauri::Manager;
    let focused = app_handle
        .get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    if !focused {
        let _ = app_handle
            .notification()
            .builder()
            .title("验证码已复制")
            .body(format!("来自 {} 的验证码 {} 已写入剪贴板,直接粘贴即可", sender, code))
            .show();
    }
}

/// 写剪贴板(带重试):监控线程每 500ms 也在开关剪贴板,偶发占用冲突
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

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 验证码提取(纯函数,重点单测)
// ---------------------------------------------------------------------------

/// 关键词(小写匹配)。没有关键词的纯数字短信(账单/流量/物流)直接放弃:
/// 把「支出100元」写进剪贴板覆盖用户内容是帮倒忙,宁可不提取
const CODE_KEYWORDS: &[&str] = &[
    "验证码", "校验码", "动态码", "动态密码", "识别码", "verification", "code", "otp",
];

/// 负向关键词:紧邻这些词的数字是卡号/账号尾号,不是验证码
const NEGATIVE_PREFIXES: &[&str] = &["尾号", "卡号", "末四位", "账号", "单号", "工号"];

/// 从短信正文中提取验证码。返回 None 表示不是验证码短信或拿不准。
///
/// 原则:宁缺毋滥。提取失败的代价只是「用户手动看一眼手机」,
/// 提取错误的代价是「错误内容覆盖了用户正在复制的剪贴板」——不对称
pub fn extract_code(body: &str) -> Option<String> {
    let lower = body.to_lowercase();
    if !CODE_KEYWORDS.iter().any(|k| lower.contains(k)) {
        return None;
    }

    // 归一化分组数字:「163-882」「9 1 2 8」→「163882」「9128」。
    // 只合并「数字 分隔符 数字」的相邻组;多轮替换处理连续分组
    let mut normalized = body.to_string();
    for _ in 0..3 {
        let next = merge_grouped_digits(&normalized);
        if next == normalized {
            break;
        }
        normalized = next;
    }

    // 候选:连续 4-8 位数字(跳过手机号 11 位、卡号 13-19 位等长数字)。
    // 手写字符扫描而非 regex,提取逻辑高频跑在轮询线程,保持轻量
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
            continue; // 11 位手机号 / 13-19 位卡号 / 3 位短号都不属于验证码
        }
        // 前文 3 个字符内的负向关键词 → 那是尾号不是验证码
        let prefix: String = chars[start.saturating_sub(3)..start].iter().collect();
        if NEGATIVE_PREFIXES.iter().any(|p| prefix.contains(p)) {
            continue;
        }
        let score = score_candidate(&normalized, start, len);
        if let Some(score) = score {
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((score, digits));
            }
        }
    }
    best.filter(|(score, _)| *score >= 5).map(|(_, code)| code)
}

/// 计算候选数字串的得分。返回 None = 一票否决。
/// 打分:长度 4-6 是主流验证码长度(+3),7-8 较少见(+1);
/// 与**最近**关键词出现的字符距离 <15 强信号(+4),<30 弱信号(+2)。
/// 阈值 5:要么近距+主流长度(3+4=7),要么远距但主流(3+2=5)
fn score_candidate(text: &str, digit_start: usize, len: usize) -> Option<i32> {
    let mut score = match len {
        4..=6 => 3,
        7..=8 => 1,
        _ => return None,
    };

    let lower = text.to_lowercase();
    // 找所有关键词的所有出现位置,取与候选数字距离最小的一个
    // (验证码可能在关键词之后,也可能在之前,如 "123456 is your code")
    let mut min_dist = usize::MAX;
    for keyword in CODE_KEYWORDS {
        let mut search_from = 0usize;
        while let Some(found) = lower[search_from..].find(keyword) {
            let byte_pos = search_from + found;
            // 字节下标 → 字符下标(评分全程用字符距离,中文正文必须)
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
            // 跳过分隔符,保留两侧数字
            out.push(c);
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chinese_code_sms() {
        assert_eq!(
            extract_code("【淘宝】验证码:483920,您正在登录,请勿泄露给他人"),
            Some("483920".to_string())
        );
    }

    #[test]
    fn test_english_grouped_code() {
        // WhatsApp 风格:分组数字要归一
        assert_eq!(
            extract_code("Your WhatsApp code is 163-882"),
            Some("163882".to_string())
        );
    }

    #[test]
    fn test_spaced_code() {
        assert_eq!(
            extract_code("【京东】动态密码:9 1 2 8,请勿告知他人"),
            Some("9128".to_string())
        );
    }

    #[test]
    fn test_no_keyword_no_extract() {
        // 无关键词:宁可漏,不可错
        assert_eq!(extract_code("【工商银行】您尾号3456的卡支出100元,余额5000元"), None);
    }

    #[test]
    fn test_negative_prefix_rejected() {
        // 关键词在场,但最近的数字是「尾号」——必须跳过它取真验证码
        assert_eq!(
            extract_code("【银行】动态密码:9128,尾号3456的卡入账100元"),
            Some("9128".to_string())
        );
    }

    #[test]
    fn test_phone_number_not_code() {
        // 11 位手机号不是验证码;400 电话合并后 10 位也被长度过滤
        assert_eq!(
            extract_code("您的验证码是1234,请在5分钟内输入。客服电话400-800-9000"),
            Some("1234".to_string())
        );
    }

    #[test]
    fn test_long_digits_ignored() {
        // 卡号形态(16 位)不提取
        assert_eq!(
            extract_code("验证码已发送至您手机,卡号4532015112830366关联的手机"),
            None
        );
    }

    #[test]
    fn test_code_after_keyword_far() {
        // 验证码与关键词距离较远但仍可识别
        assert_eq!(
            extract_code("Your Apple ID verification code is: 582913"),
            Some("582913".to_string())
        );
    }

    #[test]
    fn test_pure_digits_without_keyword() {
        assert_eq!(extract_code("123456"), None);
    }

    #[test]
    fn test_merge_grouped() {
        assert_eq!(merge_grouped_digits("163-882"), "163882");
        assert_eq!(merge_grouped_digits("9 1 2 8"), "9128");
        assert_eq!(merge_grouped_digits("400-800-9000"), "4008009000");
        assert_eq!(merge_grouped_digits("验证码 1234"), "验证码 1234");
    }

    #[test]
    fn test_order_number_negative_prefix() {
        // 「订单号」含负向词「单号」:8 位订单号不能被当成验证码
        assert_eq!(
            extract_code("尊敬的客户:您的订单号20260831已发货,验证码功能需开通"),
            None
        );
    }

    #[test]
    fn test_multi_group_spaces() {
        // 三段式空格分组:57 39 21 → 573921
        assert_eq!(
            extract_code("【支付宝】校验码:57 39 21,请勿泄露"),
            Some("573921".to_string())
        );
    }

    #[test]
    fn test_code_before_keyword() {
        // 验证码在关键词之前(Telegram/Google 风格)
        assert_eq!(
            extract_code("G-548921 is your Google verification code"),
            Some("548921".to_string())
        );
    }
}
