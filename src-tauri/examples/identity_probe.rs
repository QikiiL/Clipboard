//! 临时探针:验证稀疏包身份能否让 `UserNotificationListener::NotificationChanged`
//! 订阅成功。
//!
//! 不带 requireAdministrator 清单——稀疏包注册是每用户(CurrentUser)的,
//! 普通用户即可跑。运行前请把 `clipboard-identity.msix` / `clipboard-identity.cer`
//! 复制到与本品同目录(即 `target/debug/`),再用 `cargo run --bin identity_probe`。
//!
//! 预期:
//! - 首轮订阅应失败(0x80070490,无包身份)
//! - 自动注册稀疏包身份后,第二轮订阅成功 = 端到端打通
//! - 再跑一次探针,首轮订阅就直接成功(身份已在)
//!
//! ⚠️ 已知卡点(2026-09 实测):若签名证书只导入 CurrentUser 存储,注册
//! 报 0x87E80034(内层 0x800B0109)——AppX 部署服务只认 LocalMachine 的
//! 信任,需管理员执行 `certutil -addstore TrustedPeople clipboard-identity.cer`
//! 后再跑本探针。

use std::io::Write as _;

use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::Management::UserNotificationListener;

/// 双写:stderr + 日志文件(Shell 激活启动时控制台一闪而过,靠日志读结果)
fn out(msg: &str) {
    eprintln!("{msg}");
    let log = std::path::PathBuf::from("identity_probe.log");
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let path = match exe_dir {
        Some(d) => d.join(&log),
        None => log,
    };
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{msg}");
    }
}

// 探针是独立 crate,且按约束不动 lib.rs;identity.rs 仅依赖 windows crate、
// 无 crate 内部引用,故直接内联复用其实现(与 lib 内 services::identity 同源)。
#[path = "../src/services/identity.rs"]
mod identity;

/// 尝试订阅一次 NotificationChanged。成功返回 Ok,失败返回 HRESULT 字符串
fn try_subscribe() -> Result<(), String> {
    let listener = UserNotificationListener::Current()
        .map_err(|e| format!("UserNotificationListener::Current 失败: {e}"))?;
    listener
        .NotificationChanged(&TypedEventHandler::new(|_l, _a| Ok(())))
        .map(|_| ())
        .map_err(|e| format!("{e} (HRESULT {:#010X})", e.code().0 as u32))
}

/// 请求通知监听授权。打包应用的通知监听授权是「按包」记录的:
/// 稀疏包注册后,通知平台里还没有本包的监听者条目,不先 RequestAccessAsync
/// 直接订阅会报 0x80070490(通知平台「找不到」该包的监听者)
fn request_access() -> Result<(), String> {
    let listener = UserNotificationListener::Current()
        .map_err(|e| format!("UserNotificationListener::Current 失败: {e}"))?;
    let status = listener
        .RequestAccessAsync()
        .map_err(|e| format!("RequestAccessAsync 调用失败: {e}"))?
        .get()
        .map_err(|e| format!("RequestAccessAsync 等待失败: {e}"))?;
    out("&[probe] RequestAccessAsync => {status:?}");
    Ok(())
}

fn main() {
    // 诊断:当前进程是否真的具备包身份(sparse 注册后,新进程应当具备)
    match windows::ApplicationModel::AppInfo::Current() {
        Ok(info) => {
            let id = info.Id().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let pfn = info
                .PackageFamilyName()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            out("&[probe] 进程包身份: 有 (id={id}, pfn={pfn})");
        }
        Err(e) => out("&[probe] 进程包身份: 无 ({e})"),
    }
    // 诊断:GetAccessStatus(不弹提示,只读当前状态)
    if let Ok(l) = UserNotificationListener::Current() {
        if let Ok(s) = l.GetAccessStatus() {
            out("&[probe] GetAccessStatus => {s:?}");
        }
    }

    // COM 以 STA 初始化:RequestAccessAsync 官方要求 UI 线程调用,
    // MTA 线程上调用会直接返回 Denied(拿不到授权也无法订阅)。
    // 探针用控制台主线程模拟 STA UI 线程。
    unsafe {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        eprintln!(
            "[probe] CoInitializeEx(STA) => {hr:?}{}",
            if hr.is_err() { "(可能已初始化,继续)" } else { "" }
        );
    }

    // 先请求监听授权(打包应用按包记录授权,必须先建立条目)
    if let Err(e) = request_access() {
        out("&[probe] 请求授权失败: {e}");
    }

    out("&[probe] === 第一轮订阅(无包身份应失败 0x80070490) ===");
    match try_subscribe() {
        Ok(()) => {
            out("&[probe] 第一轮订阅就成功:本进程已具备包身份。");
        }
        Err(e) => {
            out("&[probe] 第一轮订阅失败: {e}");
            out("&[probe] 调用 ensure_identity_registered() 尝试注册稀疏包身份...");
            match identity::ensure_identity_registered() {
                Ok(()) => out("&[probe] 稀疏包身份注册成功,进行第二轮订阅..."),
                Err(err) => {
                    out("&[probe] 稀疏包身份注册失败: {err}");
                    out("&[probe] 无法继续验证,退出。");
                    std::process::exit(2);
                }
            }
            // 注册后重新请求授权(包身份已生效,授权记录需要刷新)
            if let Err(e) = request_access() {
                out("&[probe] 注册后请求授权失败: {e}");
            }
            out("&[probe] === 第二轮订阅(注册后预期成功) ===");
            match try_subscribe() {
                Ok(()) => eprintln!(
                    "[probe] 第二轮订阅成功!事件驱动已可用(进程身份在启动时确定,重启后更稳定)。"
                ),
                Err(e) => eprintln!(
                    "[probe] 第二轮订阅仍失败(进程身份在启动时确定,重启应用后生效): {e}"
                ),
            }
        }
    }
}
