mod commands;
mod models;
mod services;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app, _shortcut, event| {
            use tauri::Manager;
            use tauri_plugin_global_shortcut::ShortcutState;
            if event.state == ShortcutState::Pressed {
                if let Some(window) = app.get_webview_window("main") {
                    if !window.is_visible().unwrap_or(false) {
                        let _ = window.show();
                    }
                    let _ = window.set_focus();
                }
            }
        }).build())
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
            use tauri::Manager;

            services::tray_service::create_tray(app)?;

            // Create our own sqlx pool for direct DB access
            let app_config_dir = app
                .path()
                .app_config_dir()
                .expect("No app config dir found");
            let _ = std::fs::create_dir_all(&app_config_dir);
            let db_path = app_config_dir.join("clipboard.db");
            let db_url = format!("sqlite:{}", db_path.to_string_lossy());

            let db = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    sqlx::sqlite::SqlitePool::connect(&db_url)
                        .await
                        .expect("Failed to connect to SQLite")
                })
            });

            // Run migrations manually
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    for migration in get_migrations() {
                        sqlx::query(migration.sql)
                            .execute(&db)
                            .await
                            .expect("Failed to run migration");
                    }
                })
            });

            app.manage(db.clone());

            // Register clipboard monitor with polling
            let monitor = services::clipboard_monitor::ClipboardMonitor::new();
            app.manage(monitor.clone());
            services::clipboard_monitor::ClipboardMonitor::start_polling(
                app.handle().clone(),
                db,
                monitor,
            );

            // Register global hotkey (Ctrl+Shift+V)
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
            let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV);
            app.global_shortcut().register(shortcut)?;

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
