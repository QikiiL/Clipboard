use tauri::Manager;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
};
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::Threading::GetCurrentProcess;
use windows_core::Interface;

/// WebView2 内存优化:低内存目标模式。
/// 官方开关,浏览器/GPU 进程会更积极地回收内存。这是唯一保留的 WebView2 层优化:
/// TrySuspend / SetIsVisible 在实测中会导致唤出后前端交互失效或白屏,已全部移除。
pub fn apply_memory_optimizations(app_handle: &tauri::AppHandle) {
    if let Some(w) = app_handle.get_webview_window("main") {
        let _ = w.with_webview(|webview| unsafe {
            let controller = webview.controller();
            if let Ok(core) = controller.CoreWebView2() {
                if let Ok(v19) = core.cast::<ICoreWebView2_19>() {
                    let _ = v19
                        .SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW);
                }
            }
        });
    }
}

/// 窗口隐藏(=销毁)后的内存处理:WebView2 子进程已随窗口销毁退出,
/// 这里仅清空自身主进程的工作集(普通进程,按需换回,安全)。
pub fn set_webview_visible(_app_handle: &tauri::AppHandle, visible: bool) {
    if !visible {
        unsafe_self_trim();
    }
}

/// EmptyWorkingSet(当前进程):工作集页面转入待机列表,物理内存立即下降。
fn unsafe_self_trim() {
    unsafe {
        let handle = GetCurrentProcess();
        let _ = EmptyWorkingSet(handle);
    }
}
