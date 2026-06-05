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
                    let pool = sqlx::sqlite::SqlitePool::connect(&db_url)
                        .await
                        .expect("Failed to connect to SQLite");

                    // Enable WAL mode to prevent lock contention with tauri_plugin_sql
                    sqlx::query("PRAGMA journal_mode=WAL")
                        .execute(&pool)
                        .await
                        .expect("Failed to enable WAL mode");

                    // Set busy timeout so writers wait instead of failing immediately
                    sqlx::query("PRAGMA busy_timeout=5000")
                        .execute(&pool)
                        .await
                        .expect("Failed to set busy timeout");

                    pool
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

            // 启动定时清理任务（每小时）
            let app_handle_cleanup = app.handle().clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
                loop {
                    interval.tick().await;
                    cleanup_expired_items(&app_handle_cleanup).await;
                }
            });

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

async fn cleanup_expired_items(app_handle: &tauri::AppHandle) {
    use tauri::Manager;
    let settings = services::settings_service::load_settings(app_handle);

    // Get the sqlx pool from state
    let pool = match app_handle.try_state::<sqlx::SqlitePool>() {
        Some(pool) => pool.inner().clone(),
        None => return,
    };

    // 删除过期记录（retention_days > 0 时）
    if settings.retention_days > 0 {
        let _ = sqlx::query(
            "DELETE FROM items WHERE is_favorite = 0 AND last_used_at < datetime('now', '-' || ? || ' days')"
        )
        .bind(settings.retention_days)
        .execute(&pool)
        .await;
    }

    // 删除超量记录（max_item_count > 0 时，保留最新的）
    if settings.max_item_count > 0 {
        let _ = sqlx::query(
            "DELETE FROM items WHERE id NOT IN (SELECT id FROM items ORDER BY last_used_at DESC LIMIT ?) AND is_favorite = 0"
        )
        .bind(settings.max_item_count)
        .execute(&pool)
        .await;
    }
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
