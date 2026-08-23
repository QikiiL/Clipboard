mod commands;
mod models;
mod services;
mod utils;

/// 行业标准布局(XDG 惯例):配置与数据跟随程序目录,绿色便携。
/// `<exe所在目录>\config` = 设置(settings.json / 存储位置指针 / 窗口状态);
/// `<exe所在目录>\data`   = 数据默认位置(clipboard.db / images,可在设置中改)。
/// 取不到 exe 路径时兜底旧的 %APPDATA% 标识符目录。
fn legacy_appdata_dir() -> std::path::PathBuf {
    let app_identifier = "com.lyz.clipboard-manager-tauri";
    let config_dir = if cfg!(target_os = "windows") {
        std::env::var("APPDATA").unwrap_or_default()
    } else if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}/Library/Application Support", home)
    } else {
        let home = std::env::var("HOME").unwrap_or_default();
        match std::env::var("XDG_CONFIG_HOME") {
            Ok(path) => path,
            Err(_) => format!("{}/.config", home),
        }
    };
    std::path::PathBuf::from(config_dir).join(app_identifier)
}

fn exe_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(legacy_appdata_dir)
}

pub(crate) fn app_config_dir() -> std::path::PathBuf {
    exe_dir().join("config")
}

pub(crate) fn app_data_dir_default() -> std::path::PathBuf {
    exe_dir().join("data")
}

/// 首次以新布局启动时,从旧 %APPDATA% 位置继承配置与(默认位置的)数据。
/// 只在新 config 目录尚无 settings.json 时执行一次性迁移;
/// 若旧配置里有自定义存储位置指针,数据不动,仅指针随配置继承。
fn migrate_legacy_layout() {
    let config = app_config_dir();
    if config.join("settings.json").exists() {
        return; // 已迁移过
    }
    let legacy = legacy_appdata_dir();
    if !legacy.is_dir() {
        return;
    }
    let _ = std::fs::create_dir_all(&config);

    for name in ["settings.json", "storage.json", "window-state.json"] {
        let src = legacy.join(name);
        if src.is_file() {
            let _ = std::fs::copy(&src, config.join(name));
        }
    }

    // 无自定义存储位置指针时,旧默认位置的数据库与图片迁到新 data 目录
    let has_custom_pointer = config
        .join("storage.json")
        .is_file()
        && std::fs::read_to_string(config.join("storage.json"))
            .map(|s| s.contains("\"data_dir\": \"") && !s.contains("\"data_dir\": null"))
            .unwrap_or(false);
    if !has_custom_pointer && legacy.join("clipboard.db").is_file() {
        let data = app_data_dir_default();
        let _ = std::fs::create_dir_all(&data);
        for name in ["clipboard.db", "clipboard.db-wal", "clipboard.db-shm"] {
            let src = legacy.join(name);
            if src.is_file() {
                let _ = std::fs::copy(&src, data.join(name));
            }
        }
        let legacy_images = legacy.join("images");
        if legacy_images.is_dir() {
            let images = data.join("images");
            let _ = std::fs::create_dir_all(&images);
            for entry in std::fs::read_dir(&legacy_images).into_iter().flatten().flatten() {
                if entry.path().is_file() {
                    let _ = std::fs::copy(entry.path(), images.join(entry.file_name()));
                }
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 行业标准布局:config/ 与 data/ 跟随程序目录;
    // 首次启动从旧 %APPDATA% 位置继承配置(见 migrate_legacy_layout)。
    // 数据库与图片的实际位置由 storage_service 解析(可能被用户改到别的目录)
    migrate_legacy_layout();
    let default_config_dir = app_config_dir();
    let _ = std::fs::create_dir_all(&default_config_dir);
    // data 文件夹无条件确保存在:布局完整,也是“恢复默认”的落点
    let _ = std::fs::create_dir_all(app_data_dir_default());
    let data_dir = services::storage_service::resolve_data_dir();
    let _ = std::fs::create_dir_all(&data_dir);
    let db_path = data_dir.join("clipboard.db");
    let db_url_for_plugin = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations(&db_url_for_plugin, get_migrations())
                .build(),
        )
        .setup(move |app| {
            use tauri::Manager;

            services::tray_service::create_tray(app)?;

            // Use the pre-computed db_path for the sqlx pool (same path as plugin-sql).
            // 走文件路径而非 URL 字符串,避免中文目录/特殊前缀破坏连接串
            let db_file = db_path.clone();

            let db = tauri::async_runtime::block_on(async {
                // create_if_missing:全新安装(或存储位置刚切换)时库文件尚不存在,
                // 由后端创建;随后上方的手动迁移会建好表结构
                let options = sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&db_file)
                    .create_if_missing(true);
                let pool = sqlx::sqlite::SqlitePool::connect_with(options)
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

                // Run migrations manually
                for migration in get_migrations() {
                    sqlx::query(migration.sql)
                        .execute(&pool)
                        .await
                        .expect("Failed to run migration");
                }

                pool
            });

            app.manage(db.clone());

            // 数据目录(可能为用户自定义位置),供各处统一解析 db/images 路径
            app.manage(services::storage_service::StorageDir(data_dir.clone()));

            // 前端缩略图经 asset 协议加载;conf 里的作用域只覆盖默认位置,
            // 自定义存储目录必须在这里动态授权,否则迁移后图片全部显示占位符
            let images_dir = data_dir.join("images");
            let _ = std::fs::create_dir_all(&images_dir);
            if let Err(e) = app
                .asset_protocol_scope()
                .allow_directory(&images_dir, false)
            {
                eprintln!("Failed to extend asset protocol scope: {}", e);
            }

            // 单击条目的目标行为(默认粘贴,可在状态栏手动切换)与粘贴目标窗口
            app.manage(utils::input_focus::PasteMode::new());

            // Register clipboard monitor with polling.
            // Apply the persisted `paused` state so a user who paused monitoring
            // before quitting stays paused after restart.
            let saved_settings = services::settings_service::load_settings(app.handle());
            let monitor = services::clipboard_monitor::ClipboardMonitor::new();
            if saved_settings.paused {
                tauri::async_runtime::block_on(monitor.set_paused(true));
            }
            app.manage(monitor.clone());
            services::clipboard_monitor::ClipboardMonitor::start_polling(
                app.handle().clone(),
                db,
                monitor,
            );

            // Register global hotkey and track it for unregistration.
            // If the user has a custom hotkey, register that instead of the default
            // Ctrl+Shift+V.
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
            let mut startup_shortcut = if saved_settings.win_v_integration {
                Shortcut::new(Some(Modifiers::SUPER), Code::KeyV)
            } else {
                build_shortcut_from_settings(
                    &saved_settings.hotkey_modifier,
                    &saved_settings.hotkey_key,
                )
                .unwrap_or_else(|| {
                    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV)
                })
            };
            // Try to register the hotkey; if it fails (e.g. Super+V already taken by Windows),
            // retry a few times (system may release it late after boot), then fall back to
            // the default Ctrl+Shift+V.
            let register_fn = |shortcut: &Shortcut| -> Result<(), String> {
                app.global_shortcut()
                    .on_shortcut(shortcut.clone(), |app, _shortcut, event| {
                        use tauri_plugin_global_shortcut::ShortcutState;
                        if event.state == ShortcutState::Pressed {
                            utils::hotkey::toggle_window(app);
                        }
                    })
                    .map_err(|e| e.to_string())
            };
            let mut last_err = String::new();
            let mut registered = false;
            for attempt in 0..4 {
                if attempt > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                }
                match register_fn(&startup_shortcut) {
                    Ok(()) => {
                        registered = true;
                        break;
                    }
                    Err(e) => last_err = e,
                }
            }
            if !registered {
                eprintln!(
                    "Failed to register hotkey {:?}: {}. Falling back to Ctrl+Shift+V.",
                    startup_shortcut, last_err
                );
                let fallback = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV);
                register_fn(&fallback)?;
                let mut s = saved_settings.clone();
                s.win_v_integration = false;
                s.hotkey_modifier = "Ctrl+Shift".to_string();
                s.hotkey_key = "V".to_string();
                let _ = services::settings_service::save_settings(app.handle(), &s);
                startup_shortcut = fallback;
            }
            app.manage(commands::window::LastHotkey(std::sync::Mutex::new(Some(
                startup_shortcut,
            ))));

            // 启动定时清理任务（每小时）
            let app_handle_cleanup = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
                loop {
                    interval.tick().await;
                    cleanup_expired_items(&app_handle_cleanup).await;
                }
            });

            // 主窗口在此程序化创建(tauri.conf.json 不再声明窗口),
            // 与热键唤出时的重建共用同一条路径(几何恢复/置顶恢复/
            // CloseRequested 拦截/WebView2 低内存模式都挂在创建函数里)。
            // --minimized(开机自启)时保持隐藏,常驻托盘。
            let start_minimized = std::env::args().any(|arg| arg == "--minimized");
            utils::window_manager::create_main_window(app.handle(), !start_minimized)?;
            if start_minimized {
                utils::webview_control::set_webview_visible(app.handle(), false);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::clipboard::activate_item,
            commands::clipboard::delete_item,
            commands::clipboard::toggle_favorite,
            commands::clipboard::clear_history,
            commands::clipboard::pause_monitoring,
            commands::clipboard::get_image_base64,
            commands::settings::save_settings,
            commands::settings::load_settings,
            commands::window::show_window,
            commands::window::register_hotkey,
            commands::window::hide_window,
            commands::window::close_app,
            commands::window::get_close_behavior,
            commands::window::set_close_behavior,
            commands::window::enable_win_v_integration,
            commands::window::disable_win_v_integration,
            commands::window::set_always_on_top,
            commands::window::get_paste_mode,
            commands::window::set_paste_mode,
            commands::db_path::get_db_path,
            commands::groups::create_group,
            commands::groups::update_group,
            commands::groups::delete_group,
            commands::groups::set_item_group,
            services::storage_service::get_storage_info,
            services::storage_service::change_storage_location,
            services::storage_service::reset_storage_location,
            commands::update::check_update,
            commands::update::open_external_url,
            commands::update::write_clipboard_text,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    // 窗口已改为“隐藏即销毁、唤出即重建”:销毁最后一个窗口不能退出应用,
    // 托盘/热键/剪贴板监听常驻后台,窗口按需重建。
    // code = Some(_) 是显式 exit(托盘退出、关闭行为=直接退出),必须放行。
    app.run(|_app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}

async fn cleanup_expired_items(app_handle: &tauri::AppHandle) {
    use tauri::Manager;
    let settings = services::settings_service::load_settings(app_handle);

    // Get the sqlx pool from state
    let pool = match app_handle.try_state::<sqlx::SqlitePool>() {
        Some(pool) => pool.inner().clone(),
        None => return,
    };

    // Collect the conditions first, then delete rows and their image files.
    // 条件与 DELETE 一致,先查出待删行的图片路径,删除行后再清理文件
    let mut conditions: Vec<(String, i32)> = Vec::new();
    if settings.retention_days > 0 {
        conditions.push((
            "is_favorite = 0 AND last_used_at < datetime('now', '-' || ? || ' days')"
                .to_string(),
            settings.retention_days,
        ));
    }
    if settings.max_item_count > 0 {
        conditions.push((
            "id NOT IN (SELECT id FROM items ORDER BY last_used_at DESC LIMIT ?) AND is_favorite = 0"
                .to_string(),
            settings.max_item_count,
        ));
    }

    for (where_clause, value) in conditions {
        let file_paths: Vec<Option<String>> =
            match sqlx::query_scalar::<_, Option<String>>(&format!(
                "SELECT file_path FROM items WHERE {}",
                where_clause
            ))
            .bind(value)
            .fetch_all(&pool)
            .await
            {
                Ok(paths) => paths,
                Err(e) => {
                    eprintln!("Cleanup error (select): {}", e);
                    continue;
                }
            };

        if let Err(e) = sqlx::query(&format!("DELETE FROM items WHERE {}", where_clause))
            .bind(value)
            .execute(&pool)
            .await
        {
            eprintln!("Cleanup error (delete): {}", e);
            continue;
        }

        for path in file_paths.iter().flatten() {
            services::image_cleanup::remove_image_file(app_handle, path);
        }
    }
}

fn build_shortcut_from_settings(
    modifier: &str,
    key: &str,
) -> Option<tauri_plugin_global_shortcut::Shortcut> {
    utils::hotkey::build_shortcut(modifier, key)
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
