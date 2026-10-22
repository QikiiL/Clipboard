use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

/// 托盘图标:用专用的 tray-icon.png(小尺寸、透明圆角),而不是把窗口大图标
/// 缩到 16/32px(缩下去发糊、边缘有白底)。解码失败(资源异常)回落窗口默认
/// 图标,再兜底 1x1 透明图 —— 托盘没图标用户就找不到入口了。
fn tray_icon(app: &tauri::App) -> tauri::image::Image<'static> {
    const TRAY_ICON_PNG: &[u8] = include_bytes!("../../icons/tray-icon.png");
    if let Ok(image) = tauri::image::Image::from_bytes(TRAY_ICON_PNG) {
        return image;
    }
    eprintln!("Failed to decode tray-icon.png, falling back to window icon");
    // default_window_icon 借用自 app,拷成 owned 才能返回 'static
    match app.default_window_icon() {
        Some(icon) => {
            tauri::image::Image::new_owned(icon.rgba().to_vec(), icon.width(), icon.height())
        }
        None => tauri::image::Image::new_owned(vec![0, 0, 0, 0], 1, 1),
    }
}

pub fn create_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let pause_item = MenuItem::with_id(app, "pause", "暂停监听", true, None::<&str>)?;
    let minimize_item = MenuItem::with_id(app, "minimize", "最小化到托盘", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_item, &pause_item, &minimize_item, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .icon(tray_icon(app))
        .menu(&menu)
        .tooltip("剪雀 Sparrow")
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => {
                crate::commands::window::clear_paste_target(app);
                crate::utils::window_manager::show_or_create(app);
            }
            "pause" => {
                let monitor = app
                    .state::<crate::services::clipboard_monitor::ClipboardMonitor>()
                    .inner()
                    .clone();
                let app_handle = app.clone();
                tokio::spawn(async move {
                    monitor.toggle_paused().await;
                    let is_paused = monitor.is_paused().await;
                    // 持久化暂停状态,重启后保持一致(与设置面板行为对齐)
                    let mut settings =
                        crate::services::settings_service::load_settings(&app_handle);
                    settings.paused = is_paused;
                    let _ =
                        crate::services::settings_service::save_settings(&app_handle, &settings);
                    let _ = app_handle.emit("monitoring-paused", is_paused);
                });
            }
            "minimize" => {
                // 与热键隐藏一致:销毁窗口,内存归还系统
                crate::utils::window_manager::destroy_main_window(app);
            }
            "quit" => {
                // 统一退出入口:先销毁窗口再 exit,消除 WebView2 退出噪音
                crate::utils::window_manager::exit_app(app);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                crate::commands::window::clear_paste_target(app);
                crate::utils::window_manager::show_or_create(app);
            }
        })
        .build(app)?;

    Ok(())
}
