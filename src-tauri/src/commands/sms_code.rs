use serde::Serialize;

// 打开 Windows 通知隐私设置页。用裸 ShellExecuteW 声明(项目约定:
// 优先 #[link] 裸声明,不为单个调用给 windows crate 加 feature);
// opener 插件面向 http(s)/文件,ms-settings: 协议不走它
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: isize,
        verb: *const u16,
        file: *const u16,
        params: *const u16,
        dir: *const u16,
        showcmd: i32,
    ) -> isize;
}

#[tauri::command]
pub fn open_notification_settings() -> Result<(), String> {
    use windows::core::w;
    unsafe {
        // SW_SHOWNORMAL(1);句柄传 0(无属主窗口),verb 传 null(默认 open)
        let result = ShellExecuteW(
            0,
            std::ptr::null(),
            w!("ms-settings:privacy-notifications").0,
            std::ptr::null(),
            std::ptr::null(),
            1,
        );
        // 返回值 <= 32 表示失败(含 SE_ERR_ACCESSDENIED 等)
        if result <= 32 {
            return Err(format!("打开系统设置失败(ShellExecuteW={})", result));
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct SmsCodeStatus {
    pub enabled: bool,
    /// allowed / denied / unspecified / unsupported
    pub access: String,
    /// 最近一次捕获的 Unix 时间戳(秒),0 = 本次会话未捕获
    pub last_capture: i64,
    pub capture_count: i64,
}

/// 查询短信验证码功能状态。GetAccessStatus 是纯本地同步调用,
/// 耗时可忽略,无需阻塞线程池
#[tauri::command]
pub fn sms_code_status() -> SmsCodeStatus {
    SmsCodeStatus {
        enabled: crate::services::sms_code_service::is_enabled(),
        access: crate::services::sms_code_service::access_status(),
        last_capture: crate::services::sms_code_service::last_capture(),
        capture_count: crate::services::sms_code_service::capture_count(),
    }
}

/// 请求通知监听权限。非打包应用通常不弹系统对话框,这个调用的作用是
/// 把本应用注册进「设置>隐私和安全性>通知」的列表;真正放行需要用户
/// 手动允许。RequestAccessAsync 是异步 WinRT 调用,放阻塞线程池,
/// 不卡主线程(应用窗口不冻结)
#[tauri::command]
pub async fn sms_code_request_access() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(crate::services::sms_code_service::request_access_blocking)
        .await
        .map_err(|e| format!("权限请求任务失败: {}", e))
}
