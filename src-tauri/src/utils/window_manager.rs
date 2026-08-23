use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::models::settings::CloseBehavior;
use crate::services::settings_service::load_settings;

/// 上次窗口创建开始的时间戳(毫秒),0=空闲。不用布尔标志:登录瞬间等
/// 桌面未就绪的场景下创建可能卡死不返回,布尔会永久封死后续唤出;
/// 时间戳超时(CREATE_TIMEOUT_MS)后自动放行重试
static CREATING_AT: AtomicU64 = AtomicU64::new(0);
const CREATE_TIMEOUT_MS: u64 = 30_000;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 统一的“唤出”入口:窗口存在则显示聚焦,不存在(已被销毁)则后台线程重建。
/// 热键、托盘、show_window 命令都走这里。
pub fn show_or_create(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        ensure_on_screen(app, &window);
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    spawn_create(app);
}

/// 自愈:窗口保存的坐标可能已不在任何显示器上(登录早期枚举不全、
/// 拔了显示器、改了分辨率),越界时拉回主显示器居中,否则用户会以为
/// "唤不出窗口"。仅对可见判定,不改变正常位置。
fn ensure_on_screen(app: &AppHandle, window: &tauri::WebviewWindow) {
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let Ok(monitors) = app.available_monitors() else {
        return;
    };
    if monitors.is_empty() {
        return;
    }
    let on_screen = monitors.iter().any(|m| {
        let mp = m.position();
        let ms = m.size();
        pos.x + size.width as i32 > mp.x
            && pos.x < mp.x + ms.width as i32
            && pos.y + size.height as i32 > mp.y
            && pos.y < mp.y + ms.height as i32
    });
    if !on_screen {
        let Some(primary) = app
            .primary_monitor()
            .ok()
            .flatten()
            .or_else(|| monitors.first().cloned())
        else {
            return;
        };
        let mp = primary.position();
        let ms = primary.size();
        let x = mp.x + (ms.width as i32 - size.width as i32).max(0) / 2;
        let y = mp.y + (ms.height as i32 - size.height as i32).max(0) / 2;
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

/// 在独立线程中重建窗口:避免占用事件循环线程,
/// 也避开“从主线程事件回调里同步 build 可能死锁”的问题。
/// 防重入用时间戳:进行中且未超时才跳过,卡死后 30 秒自动解锁重试。
fn spawn_create(app: &AppHandle) {
    let now = now_ms();
    let prev = CREATING_AT.swap(now, Ordering::SeqCst);
    if prev != 0 && now.saturating_sub(prev) < CREATE_TIMEOUT_MS {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = create_main_window(&app, true) {
            eprintln!("Failed to recreate main window: {}", e);
        }
        CREATING_AT.store(0, Ordering::SeqCst);
    });
}

/// 创建主窗口。tauri.conf.json 不再声明窗口,启动与唤出共用此函数。
/// 窗口先隐藏,页面加载完成后才显示(show_when_ready),避免唤出瞬间的白屏闪烁。
/// 返回值仅给启动路径用:setup 阶段创建失败应直接终止应用。
pub fn create_main_window(app: &AppHandle, show_when_ready: bool) -> tauri::Result<()> {
    let state = load_window_state();

    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("剪贴板管理器 (Clipboard)")
        // 默认尺寸取自用户实测偏好(461×569 物理 @125% 缩放);
        // 老用户由 window-state.json 精确恢复,此值仅首次启动时生效
        .inner_size(370.0, 455.0)
        .resizable(true)
        .decorations(false)
        .shadow(true)
        .visible(false)
        .on_page_load(move |window, payload| {
            if show_when_ready
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                let _ = window.show();
                let _ = window.set_focus();
                ensure_focus(window);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        // icon() 消耗 builder;仅在不支持窗口图标的平台才可能失败,Windows 不会
        builder = builder.icon(icon.clone()).expect("set window icon");
    }
    let window = builder.build()?;

    // 恢复上次的窗口几何位置:销毁重建不像 hide/show 那样由系统免费保留位置
    if let (Some(x), Some(y)) = (state.x, state.y) {
        if position_on_screen(app, x, y) {
            let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
        }
    }
    match (state.w, state.h) {
        // 恢复用户上次的尺寸;首次使用(无记录)默认 461×569 物理像素,
        // 与显示缩放无关,所有机器首开都是同一尺寸
        (Some(w), Some(h)) => {
            let _ = window.set_size(tauri::PhysicalSize::new(w, h));
        }
        _ => {
            let _ = window.set_size(tauri::PhysicalSize::new(461, 569));
        }
    }
    if state.maximized {
        let _ = window.maximize();
    }
    if load_settings(app).pinned {
        let _ = window.set_always_on_top(true);
    }

    // 几何状态由移动/缩放事件实时持久化到独立文件(不进 AppSettings:
    // 前端设置面板即改即存会整体回传设置对象,曾把几何字段清空)
    register_geometry_persistence(&window);
    // 以下是每次重建都必须重新挂上的窗口级接线
    register_close_handler(&window);
    crate::utils::webview_control::apply_memory_optimizations(app);

    Ok(())
}

/// 窗口几何状态:独立于 AppSettings 的持久化文件,
/// 由 Moved/Resized 事件实时写入,窗口销毁重建后精确恢复。
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct WindowState {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub maximized: bool,
}

fn window_state_path() -> std::path::PathBuf {
    crate::app_config_dir().join("window-state.json")
}

pub fn load_window_state() -> WindowState {
    std::fs::read_to_string(window_state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_window_state(s: &WindowState) {
    let dir = crate::app_config_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_string_pretty(s) {
        let _ = std::fs::write(window_state_path(), json);
    }
}

/// 监听移动/缩放:非最大化时记下几何,最大化只记标志(保留上次的正常尺寸)。
/// 文件极小,拖拽过程中的连续写入可接受。
fn register_geometry_persistence(window: &tauri::WebviewWindow) {
    let win = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Moved(pos) => {
            let mut s = load_window_state();
            if !win.is_maximized().unwrap_or(false) {
                s.x = Some(pos.x);
                s.y = Some(pos.y);
            }
            save_window_state(&s);
        }
        tauri::WindowEvent::Resized(size) => {
            let mut s = load_window_state();
            let maximized = win.is_maximized().unwrap_or(false);
            s.maximized = maximized;
            if !maximized {
                s.w = Some(size.width);
                s.h = Some(size.height);
            }
            save_window_state(&s);
        }
        _ => {}
    });
}

/// CloseRequested 拦截,按 close_behavior 分流:询问 / 最小化(销毁)/ 直接退出
fn register_close_handler(window: &tauri::WebviewWindow) {
    let win = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            let app_handle = win.app_handle();
            let settings = load_settings(app_handle);
            match settings.close_behavior {
                CloseBehavior::Minimize => {
                    api.prevent_close();
                    destroy_main_window(app_handle);
                }
                CloseBehavior::Close => {
                    // 主进程不随最后一个窗口销毁而退出(见 lib.rs 的 prevent_exit),
                    // “直接退出”必须显式调 exit
                    app_handle.exit(0);
                }
                CloseBehavior::Ask => {
                    api.prevent_close();
                    let _ = app_handle.emit("ask-close-behavior", ());
                }
            }
        }
    });
}

/// 销毁主窗口:WebView2 控件与整棵 msedgewebview2.exe 进程树随之退出,内存归还系统。
/// 几何状态已由 Moved/Resized 事件实时持久化,这里无需再保存。
pub fn destroy_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.destroy();
    // 主进程工作集裁剪,物理内存立即下降
    crate::utils::webview_control::set_webview_visible(app, false);
}

/// 页面加载完成后的焦点保障:紧跟 show() 调用的 set_focus() 可能被
/// tao 的可见性标志门槛或系统前台锁定静默吞掉,窗口"可见但非前台"——
/// 下一次热键会被 toggle 判定为未聚焦而只走显示分支,表现为
/// "第一次按键仅切焦点、第二次才隐藏"。这里异步轮询,未真正取得
/// 前台焦点就重试 set_focus,最多约 1.5 秒;窗口期间被销毁则安静退出。
fn ensure_focus(window: tauri::WebviewWindow) {
    std::thread::spawn(move || {
        let hwnd = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
        for _ in 0..15 {
            if window.is_focused().unwrap_or(false)
                || crate::utils::input_focus::is_foreground_window(hwnd)
            {
                return;
            }
            let _ = window.set_focus();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
}

/// 保存的窗口坐标是否落在任一显示器范围内(拔掉显示器后窗口不会“消失”在屏外)
fn position_on_screen(app: &AppHandle, x: i32, y: i32) -> bool {
    match app.available_monitors() {
        Ok(monitors) => monitors.iter().any(|m| {
            let pos = m.position();
            let size = m.size();
            x >= pos.x
                && y >= pos.y
                && x < pos.x + size.width as i32
                && y < pos.y + size.height as i32
        }),
        Err(_) => true, // 查询失败时不拦截,交给系统兜底
    }
}
