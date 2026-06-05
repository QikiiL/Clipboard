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
