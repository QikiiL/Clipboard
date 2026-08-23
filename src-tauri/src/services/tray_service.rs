use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

pub fn create_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let pause_item = MenuItem::with_id(app, "pause", "暂停监听", true, None::<&str>)?;
    let minimize_item = MenuItem::with_id(app, "minimize", "最小化到托盘", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_item, &pause_item, &minimize_item, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .icon(
            app.default_window_icon()
                .cloned()
                .unwrap_or_else(|| tauri::image::Image::new_owned(vec![0, 0, 0, 0], 1, 1)),
        )
        .menu(&menu)
        .tooltip("剪贴板管理器")
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
                app.exit(0);
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
