mod commands;
mod models;
mod services;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:clipboard.db", get_migrations())
                .build(),
        )
        .setup(|app| {
            services::tray_service::create_tray(app)?;

            // Register clipboard monitor
            let monitor = services::clipboard_monitor::ClipboardMonitor::new();
            app.manage(monitor.clone());

            let app_handle = app.handle().clone();
            let monitor_clone = monitor.clone();
            use tauri_plugin_clipboard_manager::ClipboardContent;
            tauri_plugin_clipboard_manager::ClipboardManagerExt::clipboard_manager(&app_handle)
                .watch(move |content: ClipboardContent| {
                    let app_handle = app_handle.clone();
                    let monitor = monitor_clone.clone();
                    tokio::spawn(async move {
                        match content {
                            ClipboardContent::Text(text) => {
                                let item_type = if text.starts_with("http://") || text.starts_with("https://") {
                                    1
                                } else {
                                    0
                                };
                                monitor.handle_clipboard_change(app_handle, text, item_type, None).await;
                            }
                            ClipboardContent::Image { bytes, .. } => {
                                let hash = crate::utils::hash::compute_hash_bytes(&bytes);
                                let images_dir = app_handle.path().app_data_dir()
                                    .unwrap_or_default()
                                    .join("images");
                                let _ = std::fs::create_dir_all(&images_dir);
                                let file_path = images_dir.join(format!("{}.png", hash));
                                let _ = std::fs::write(&file_path, &bytes);
                                monitor.handle_clipboard_change(
                                    app_handle,
                                    "[图片]".to_string(),
                                    2,
                                    Some(file_path.to_string_lossy().to_string()),
                                ).await;
                            }
                            ClipboardContent::Html(html) => {
                                monitor.handle_clipboard_change(app_handle, html, 0, None).await;
                            }
                            ClipboardContent::Rtf(rtf) => {
                                monitor.handle_clipboard_change(app_handle, rtf, 0, None).await;
                            }
                        }
                    });
                })
                .expect("Failed to watch clipboard");

            // Register global hotkey (Ctrl+Shift+V)
            let app_handle = app.handle().clone();
            use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, Modifiers, Code, ShortcutState};
            let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV);
            app_handle.global_shortcut().register(
                shortcut,
                move |_app_handle, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(window) = _app_handle.get_webview_window("main") {
                            if !window.is_visible().unwrap_or(false) {
                                let _ = window.show();
                            }
                            let _ = window.set_focus();
                        }
                    }
                },
            )?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::clipboard::paste_item,
            commands::clipboard::delete_item,
            commands::clipboard::toggle_favorite,
            commands::clipboard::pause_monitoring,
            commands::settings::save_settings,
            commands::settings::load_settings,
            commands::window::show_window,
            commands::window::register_hotkey,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn get_migrations() -> Vec<tauri_plugin_sql::Migration> {
    vec![
        tauri_plugin_sql::Migration {
            version: 1,
            description: "001_create_tables",
            sql: include_str!("../migrations/001_create_tables.sql"),
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        tauri_plugin_sql::Migration {
            version: 2,
            description: "002_add_indexes",
            sql: include_str!("../migrations/002_add_indexes.sql"),
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
    ]
}
