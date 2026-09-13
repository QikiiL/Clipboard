//! 连续粘贴(框选模式)命令。
//!
//! 流程:前端把框选条目按界面从上到下的顺序传入 → 后端建 FIFO 队列并注册
//! 步进热键 → 首条立即投递(窗口随之销毁、焦点交还目标应用)→ 此后每按
//! 一次步进热键(或面板按钮)出队一条,直到队列取空。
//!
//! 注意:start 在投递首条时会销毁窗口,invoke 的响应可能到不了前端,
//! 进度一律以 `continuous-paste` 事件 + 重建后的 status 查询为准。

use tauri::{AppHandle, Manager};

use crate::services::continuous_paste::{self, ContinuousPasteState};

/// 开始前先确认条目都还存在:此刻窗口还活着,报错用户看得见;
/// 一旦开始投递窗口就会销毁,那时再报错就无处展示了
async fn precheck_items(app: &AppHandle, ids: &[i64]) -> Result<(), String> {
    let db = app.state::<sqlx::SqlitePool>();
    for &id in ids {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM items WHERE id = ?")
            .bind(id)
            .fetch_one(&*db)
            .await
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Err(format!("条目 {id} 已不存在,请刷新列表后重试"));
        }
    }
    Ok(())
}

/// 开始连续粘贴(FIFO)。只武装,不立即粘贴:武装物理触发钩子后
/// 隐藏窗口、把焦点交还目标应用,第一条等用户按下触发热键(默认 Ctrl+V)
#[tauri::command]
pub async fn start_continuous_paste(
    app: tauri::AppHandle,
    ids: Vec<i64>,
) -> Result<continuous_paste::QueueStatus, String> {
    if ids.is_empty() {
        return Err("请先框选要连续粘贴的条目".into());
    }
    precheck_items(&app, &ids).await?;

    let state = app.state::<ContinuousPasteState>();
    state.begin(ids);
    // 先武装钩子再隐藏窗口:过渡期间按下的触发热键也能被捕获
    let hook_installed = continuous_paste::install_step_hook(&app);
    if !hook_installed {
        eprintln!("连续粘贴触发钩子武装失败,只能通过面板按钮逐条粘贴");
    }
    // 隐藏窗口、焦点交还目标应用,但第一条不投 —— 等用户的第一次按键。
    // 窗口在本调用内被销毁,返回值大概率送不到前端,进度走事件
    if app
        .get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false)
    {
        crate::services::paste_service::release_focus_to_target(&app).await;
    }
    Ok(state.status_snapshot())
}

/// 开始自动连贴:后端循环按固定间隔逐条投递,直到队列清空或用户停止
#[tauri::command]
pub async fn start_continuous_paste_auto(
    app: tauri::AppHandle,
    ids: Vec<i64>,
    interval_ms: Option<u64>,
) -> Result<(), String> {
    if ids.is_empty() {
        return Err("请先框选要连续粘贴的条目".into());
    }
    precheck_items(&app, &ids).await?;

    let state = app.state::<ContinuousPasteState>();
    if state.is_auto_running() {
        return Err("自动连贴已在进行中".into());
    }
    state.begin(ids);
    tauri::async_runtime::spawn(continuous_paste::run_auto(
        app.clone(),
        interval_ms.unwrap_or(800),
    ));
    Ok(())
}

/// 手动步进:队列头出队一条并粘贴到当前焦点输入框
#[tauri::command]
pub async fn continuous_paste_next(
    app: tauri::AppHandle,
) -> Result<continuous_paste::QueueStatus, String> {
    let state = app.state::<ContinuousPasteState>();
    if state.is_auto_running() || state.is_empty() {
        return Ok(state.status_snapshot());
    }
    continuous_paste::run_once(&app).await?;
    Ok(state.status_snapshot())
}

/// 仅停止自动连贴,保留剩余队列(用户可改回手动步进继续)
#[tauri::command]
pub fn stop_continuous_paste_auto(app: tauri::AppHandle) -> continuous_paste::QueueStatus {
    let state = app.state::<ContinuousPasteState>();
    state.request_auto_stop();
    state.status_snapshot()
}

/// 查询队列状态(窗口重建后前端据此恢复 UI)
#[tauri::command]
pub fn get_continuous_paste_status(app: tauri::AppHandle) -> continuous_paste::QueueStatus {
    app.state::<ContinuousPasteState>().status_snapshot()
}

/// 取消连续粘贴:停掉自动循环、卸掉触发钩子并清空队列(含已粘贴记录)
#[tauri::command]
pub fn cancel_continuous_paste(app: tauri::AppHandle) -> continuous_paste::QueueStatus {
    let state = app.state::<ContinuousPasteState>();
    state.request_auto_stop();
    crate::utils::paste_hook::uninstall();
    state.clear();
    continuous_paste::emit_status(&app, "cancelled", None);
    state.status_snapshot()
}
