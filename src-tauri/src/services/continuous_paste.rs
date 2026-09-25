//! 连续粘贴(框选模式):由后端持有的 FIFO 粘贴队列。
//!
//! 为什么队列必须放在后端:主窗口是"隐藏即销毁、唤出即重建",窗口一销毁,
//! 前端全部状态(含 Zustand store)随之消失。用户在面板里点「开始连续粘贴」
//! 后窗口立即销毁、焦点交还目标应用,此后每按一次触发热键出队一条 ——
//! 只有活在整个应用生命周期里的 Rust 状态,才能跨窗口销毁/重建接管队列。
//!
//! 交互约定:
//! - `begin(ids)` 的 ids 顺序即粘贴顺序(前端按点选先后传入,见
//!   src/lib/continuousPaste.ts),FIFO 出队;
//! - 「开始」只武装不粘贴:窗口隐藏、焦点交还目标应用,第一条等用户按下
//!   物理触发热键(默认 Ctrl+V,低级键盘钩子拦截,见 utils/paste_hook);
//! - 快速连按不丢:投递忙碌期间的按键记入待办计数,当前条目完成后自动补上;
//! - 「自动连贴」由后端循环按固定间隔逐条投递,直到队列清空或用户停止,
//!   期间不武装钩子(节奏由循环控制,用户的 Ctrl+V 保持原生行为);
//! - 每次状态变化都发 `continuous-paste` 事件,前端(含重建后的新窗口)
//!   以事件里的 status 快照为准同步 UI。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::services::paste_service;
use crate::services::settings_service::load_settings;
use crate::utils::paste_hook;

/// 前端监听的进度事件名
pub const EVENT_NAME: &str = "continuous-paste";

/// 队列步进的监听抑制时长。单击条目路径(activate_item)用 500ms,
/// 队列步进追求节奏,只要盖过写入剪贴板的瞬间即可(重复内容本就会
/// 被采集端去重,顶多触发一次 last_used_at 更新,与显式更新等价)
const STEP_SUPPRESS_MS: u64 = 120;

#[derive(Debug, Default)]
struct QueueInner {
    /// 待粘贴条目,队首 = 下一条
    queue: VecDeque<i64>,
    /// 已按顺序粘贴的条目 id(重建窗口后据此恢复「已粘贴」标记)
    pasted: Vec<i64>,
    /// 本轮总数
    total: usize,
    /// 队列取空后的完成标记(保留 pasted 供 UI 展示,直到取消/开启新一轮)
    finished: bool,
}

/// 队列状态快照:前端 status 同步的唯一来源
#[derive(Debug, Clone, Serialize)]
pub struct QueueStatus {
    pub active: bool,
    pub total: usize,
    pub remaining: usize,
    pub next_id: Option<i64>,
    pub pasted: Vec<i64>,
    pub auto_running: bool,
    /// 物理触发钩子是否已武装
    pub hotkey_registered: bool,
}

/// 防重入 TTL:单条投递(含焦点归还、写剪贴板、模拟按键、抑制等待)
/// 正常远小于 1s;超时视为投递线程卡死,允许重新进入 —— 否则一次卡死
/// 会让整个队列永久无响应,只能重启应用。自动连贴循环每条续期
const BUSY_TTL_MS: u64 = 8000;

pub struct ContinuousPasteState {
    inner: Mutex<QueueInner>,
    /// 投递防重入截止时间(毫秒时间戳,0 = 空闲):同一时刻只允许一条在投
    busy_until: AtomicU64,
    /// 忙碌期间收到的物理触发计数:当前条目完成后逐条补上,
    /// 快速连按不丢步、也不并发
    pending_steps: AtomicUsize,
    auto_running: AtomicBool,
    auto_cancel: AtomicBool,
    /// 会话代号:begin() 自增。旧的自动连贴循环据此发现自己已过时并退出,
    /// 不会误清新会话的标志位
    generation: AtomicU64,
}

impl ContinuousPasteState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(QueueInner::default()),
            busy_until: AtomicU64::new(0),
            pending_steps: AtomicUsize::new(0),
            auto_running: AtomicBool::new(false),
            auto_cancel: AtomicBool::new(false),
            generation: AtomicU64::new(0),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, QueueInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 开启新一轮队列(替换旧队列)。ids 顺序 = 粘贴顺序(前端按点选先后传入)。
    /// 顺带卸掉上一轮可能残留的步进钩子,由各启动路径决定是否重新武装
    pub fn begin(&self, ids: Vec<i64>) {
        let total = ids.len();
        *self.lock() = QueueInner {
            queue: ids.into(),
            pasted: Vec::new(),
            total,
            finished: false,
        };
        self.pending_steps.store(0, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.auto_running.store(false, Ordering::SeqCst);
        self.auto_cancel.store(false, Ordering::SeqCst);
        paste_hook::uninstall();
    }

    pub fn pop_next(&self) -> Option<i64> {
        self.lock().queue.pop_front()
    }

    /// 投递失败时把条目放回队首,允许用户重试
    pub fn push_front(&self, id: i64) {
        self.lock().queue.push_front(id);
    }

    pub fn mark_pasted(&self, id: i64) {
        self.lock().pasted.push(id);
    }

    /// 队列取空:保留 pasted 供 UI 展示「已完成」,直到取消或开启新一轮
    pub fn finish(&self) {
        self.lock().finished = true;
    }

    pub fn clear(&self) {
        *self.lock() = QueueInner::default();
        self.pending_steps.store(0, Ordering::SeqCst);
    }

    pub fn is_empty(&self) -> bool {
        self.lock().queue.is_empty()
    }

    pub fn is_auto_running(&self) -> bool {
        self.auto_running.load(Ordering::SeqCst)
    }

    pub fn request_auto_stop(&self) {
        self.auto_cancel.store(true, Ordering::SeqCst);
    }

    fn set_auto_running(&self, value: bool) {
        self.auto_running.store(value, Ordering::SeqCst);
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// 忙碌期间记一笔待办步进(封顶防滥用)
    fn add_pending_step(&self) {
        let _ = self.pending_steps.fetch_update(
            Ordering::SeqCst,
            Ordering::SeqCst,
            |n| if n < paste_hook::MAX_PENDING { Some(n + 1) } else { None },
        );
    }

    /// 取一笔待办;没有则返回 false
    fn take_pending_step(&self) -> bool {
        self.pending_steps
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                if n > 0 {
                    Some(n - 1)
                } else {
                    None
                }
            })
            .is_ok()
    }

    /// 进入投递(防重入)。false = TTL 内已有投递在进行。
    /// 截止时间已过(投递线程卡死)时允许接管,队列不会永久卡死
    fn try_enter_delivery(&self) -> bool {
        let now = crate::utils::window_manager::now_ms();
        let current = self.busy_until.load(Ordering::SeqCst);
        if current != 0 && current > now {
            return false;
        }
        self.busy_until
            .compare_exchange(current, now + BUSY_TTL_MS, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// 自动连贴循环每投一条续期一次:整轮队列耗时再长也不触发 TTL
    fn renew_delivery(&self) {
        self.busy_until.store(
            crate::utils::window_manager::now_ms() + BUSY_TTL_MS,
            Ordering::SeqCst,
        );
    }

    fn exit_delivery(&self) {
        self.busy_until.store(0, Ordering::SeqCst);
    }

    pub fn status_snapshot(&self) -> QueueStatus {
        let guard = self.lock();
        QueueStatus {
            active: guard.total > 0 && !guard.finished,
            total: guard.total,
            remaining: guard.queue.len(),
            next_id: guard.queue.front().copied(),
            pasted: guard.pasted.clone(),
            auto_running: self.auto_running.load(Ordering::SeqCst),
            hotkey_registered: paste_hook::is_installed(),
        }
    }
}

/// 物理触发热键按下(已被钩子吞掉,目标应用收不到):自动连贴中忽略;
/// 空闲则立即步进,忙碌则记待办,当前条目完成后自动补上
pub fn on_physical_trigger(app: &AppHandle) {
    let state = app.state::<ContinuousPasteState>();
    if state.is_auto_running() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_once(&app).await {
            eprintln!("连续粘贴步进失败: {}", e);
        }
    });
}

/// 武装物理步进钩子:从设置解析触发组合(默认 Ctrl+V),
/// 解析失败退回 Ctrl+V。返回是否武装成功
pub fn install_step_hook(app: &AppHandle) -> bool {
    let settings = load_settings(app);
    let (mods, key_vk) =
        paste_hook::parse_trigger(&settings.seq_paste_modifier, &settings.seq_paste_key)
            .unwrap_or((paste_hook::MOD_CTRL, 'V' as u32));
    paste_hook::install(app.clone(), mods, key_vk)
}

/// 从队列头投递一条到当前焦点输入框。
/// Err = 投递失败(条目已放回队首,错误已通过事件上报);
/// Ok = 已粘贴一条,或队列已结束。
pub async fn run_once(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<ContinuousPasteState>();
    // 防重入:已在投递则记待办,当前条目完成后补一步 ——
    // 快速连按每一按都对应一条,不丢、不并发
    if !state.try_enter_delivery() {
        state.add_pending_step();
        return Ok(());
    }
    let mut result = run_once_inner(app).await;
    while result.is_ok() && state.take_pending_step() {
        state.renew_delivery();
        result = run_once_inner(app).await;
    }
    state.exit_delivery();
    result
}

async fn run_once_inner(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<ContinuousPasteState>();
    loop {
        let Some(id) = state.pop_next() else {
            // 队列已空:卸掉钩子并标记完成(pasted 保留给 UI)
            paste_hook::uninstall();
            state.finish();
            emit_status(app, "finished", None);
            return Ok(());
        };
        // 队列开始后窗口通常已销毁,前台即目标应用,不动焦点 —— 粘贴落进
        // 用户当前光标所在的输入框;窗口还开着时(面板内点「粘贴下一条」)
        // 先销毁窗口并把焦点还给唤出前记录的目标。「粘贴后保持打开」不影响
        // 此路径:开始连贴时面板本来就会立即隐藏
        let handling = if app
            .get_webview_window("main")
            .map(|w| w.is_visible().unwrap_or(false))
            .unwrap_or(false)
        {
            paste_service::FocusHandling::HideFirst
        } else {
            paste_service::FocusHandling::AtTarget
        };
        // 模拟的 Ctrl+V 是注入事件,步进钩子会原样放行,投递期间无需
        // 任何"热键真空期" —— 物理连按全程可被捕获排队
        match paste_service::deliver_item_by_id(
            app,
            id,
            handling,
            true,
            STEP_SUPPRESS_MS,
        )
        .await
        {
            Ok(paste_service::Delivery::Delivered) => {
                let was_last = state.is_empty();
                state.mark_pasted(id);
                if was_last {
                    // 最后一条投完:立即卸钩并标记完成,把 Ctrl+V 还给系统,
                    // 不必等下一次触发才发现队列已空
                    paste_hook::uninstall();
                    state.finish();
                    emit_status(app, "finished", None);
                } else {
                    emit_status(app, "pasted", None);
                }
                return Ok(());
            }
            // 条目已被删除(入队后列表变动):跳过,继续下一条
            Ok(paste_service::Delivery::Missing) => continue,
            Err(e) => {
                state.push_front(id);
                emit_status(app, "error", Some(&e));
                return Err(e);
            }
        }
    }
}

/// 自动连贴:按固定间隔逐条投递,直到队列清空、出错或用户停止。
/// 通过 `tauri::async_runtime::spawn` 在后台运行,不阻塞调用方
pub async fn run_auto(app: AppHandle, interval_ms: u64) {
    let state = app.state::<ContinuousPasteState>();
    if state.is_auto_running() {
        return;
    }
    state.set_auto_running(true);
    let my_gen = state.generation();
    let interval = std::time::Duration::from_millis(interval_ms.clamp(300, 10_000));
    emit_status(&app, "auto-started", None);
    loop {
        // 新一轮队列已开始:立即退出,绝不触碰新会话的标志位
        if state.generation() != my_gen {
            return;
        }
        if state.auto_cancel.load(Ordering::SeqCst) {
            break;
        }
        if run_once(&app).await.is_err() {
            break;
        }
        if state.is_empty() {
            break; // 队列已清空(run_once 已发 finished)
        }
        tokio::time::sleep(interval).await;
    }
    if state.generation() == my_gen {
        state.set_auto_running(false);
        emit_status(&app, "auto-stopped", None);
    }
}

/// 广播队列状态快照;message 仅在错误事件上携带
pub fn emit_status(app: &AppHandle, event: &str, message: Option<&str>) {
    let status = app.state::<ContinuousPasteState>().status_snapshot();
    let payload = match message {
        Some(m) => serde_json::json!({"event": event, "status": status, "message": m}),
        None => serde_json::json!({"event": event, "status": status}),
    };
    let _ = app.emit(EVENT_NAME, payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_is_fifo() {
        let state = ContinuousPasteState::new();
        state.begin(vec![3, 1, 2]);
        assert_eq!(state.pop_next(), Some(3));
        assert_eq!(state.pop_next(), Some(1));
        state.mark_pasted(3);
        state.mark_pasted(1);
        assert_eq!(state.pop_next(), Some(2));
        assert_eq!(state.pop_next(), None);
        state.finish();
        let s = state.status_snapshot();
        assert_eq!(s.total, 3);
        assert_eq!(s.pasted, vec![3, 1]);
        assert_eq!(s.remaining, 0);
        assert!(!s.active, "队列取空且完成后不再是 active");
    }

    #[test]
    fn failed_item_returns_to_queue_front() {
        let state = ContinuousPasteState::new();
        state.begin(vec![7, 8]);
        assert_eq!(state.pop_next(), Some(7));
        state.push_front(7);
        assert_eq!(state.pop_next(), Some(7), "放回队首后应先重试同一条");
    }

    #[test]
    fn begin_replaces_previous_queue() {
        let state = ContinuousPasteState::new();
        state.begin(vec![1, 2]);
        state.mark_pasted(1);
        state.begin(vec![9]);
        let s = state.status_snapshot();
        assert_eq!(s.total, 1);
        assert_eq!(s.remaining, 1);
        assert_eq!(s.next_id, Some(9));
        assert!(s.pasted.is_empty(), "新会话不带旧会话的已粘贴记录");
        assert!(s.active);
    }

    #[test]
    fn clear_resets_everything() {
        let state = ContinuousPasteState::new();
        state.begin(vec![1, 2]);
        state.mark_pasted(1);
        state.clear();
        let s = state.status_snapshot();
        assert!(!s.active);
        assert_eq!(s.total, 0);
        assert!(s.pasted.is_empty());
        assert_eq!(s.next_id, None);
    }

    #[test]
    fn pending_steps_are_counted_and_capped() {
        let state = ContinuousPasteState::new();
        for _ in 0..10 {
            state.add_pending_step();
        }
        assert!(state.take_pending_step());
        assert!(state.take_pending_step());
        // 取到 0 之后再取返回 false
        for _ in 0..20 {
            let _ = state.take_pending_step();
        }
        assert!(!state.take_pending_step());
    }
}
