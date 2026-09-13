//! 弹出面板的自适应尺寸与 DPI 感知。
//!
//! 面板是“隐藏即销毁、唤出即重建”的小型弹窗,没有常规窗口“创建一次就固定”的语义:
//! 每次重建都要依据「鼠标所在显示器 + 其工作区 + 该显示器缩放因子」重新算一遍尺寸与位置。
//! 不做这件事会踩两个坑:
//!   1. 直接把保存的物理尺寸 set_size,在 1366×768 这类小屏上会越出工作区(盖住任务栏);
//!   2. 100% 与 150% 显示器之间移动时,物理尺寸不变,但视觉大小相差 50%。
//!
//! 本模块所有“逻辑尺寸”都指 CSS 像素,与设计稿基准 420×520 同单位。

use tauri::window::Monitor;
use tauri::{AppHandle, PhysicalPosition, PhysicalSize};

use super::window_manager::WindowState;

/// 设计基准(逻辑像素):用户实测偏好的面板尺寸
const DESIGN_W: f64 = 420.0;
const DESIGN_H: f64 = 520.0;
/// 上一版默认尺寸(逻辑像素):已保存状态里仍是该值的,一次性跟随新默认放大
const LEGACY_DESIGN_W: f64 = 370.0;
const LEGACY_DESIGN_H: f64 = 455.0;

/// 旧默认尺寸的一次性迁移:窗口状态里保存的仍是上一版默认(370×455 逻辑)
/// 的,跟随新默认放大;用户手动调整过的其他尺寸一律不动。
/// 与 seq 热键迁移同理 —— 只改默认值的话,老用户被保存状态一直恢复旧尺寸,
/// 永远看不到新默认。
pub fn migrate_legacy_size(state: &mut WindowState) {
    if let (Some(w), Some(h)) = (state.w, state.h) {
        // 无 scale 字段的旧文件按 1.0 处理:物理 370×455 ≈ 逻辑 370×455
        let scale = state.scale.unwrap_or(1.0);
        if scale <= 0.0 {
            return;
        }
        let logical_w = w as f64 / scale;
        let logical_h = h as f64 / scale;
        if (logical_w - LEGACY_DESIGN_W).abs() < 0.5 && (logical_h - LEGACY_DESIGN_H).abs() < 0.5 {
            state.w = Some((DESIGN_W * scale).round() as u32);
            state.h = Some((DESIGN_H * scale).round() as u32);
        }
    }
}
/// 最小尺寸(逻辑像素):防止用户把面板拖到不可用(与 builder.min_inner_size 保持一致)
pub const MIN_W: f64 = 320.0;
pub const MIN_H: f64 = 360.0;
/// 工作区四周留边(逻辑像素):避免面板紧贴屏幕/任务栏边缘
const MARGIN: f64 = 16.0;
/// 缩放基准:以 1080p 屏幕为 1.0,面板随屏幕大小适度缩放
const REF_W: f64 = 1920.0;
const REF_H: f64 = 1080.0;
/// 缩放上下限:小屏不低于 0.9(再小内容不可读),大屏不超过 1.25(再大就失去“小面板”定位)
const MIN_SCALE: f64 = 0.9;
const MAX_SCALE: f64 = 1.25;
/// work_area 异常时的兜底任务栏高度(物理像素),约等于 Windows 默认任务栏
const FALLBACK_TASKBAR_H: u32 = 48;

/// 创建窗口后调用:按目标显示器的工作区与缩放因子,设置并钳制面板尺寸与位置。
/// 拿不到任何显示器信息时静默降级(恢复保存尺寸或 370×455 逻辑 + 居中)。
pub fn apply_initial_geometry(app: &AppHandle, window: &tauri::WebviewWindow, state: &WindowState) {
    let Some(monitor) = pick_target_monitor(app) else {
        fallback(window, state);
        return;
    };
    let scale = monitor.scale_factor();
    let (work_pos, work_size) = work_area(&monitor);

    // 尺寸:跨 DPI 还原 + 工作区/最小尺寸钳制(修掉小屏越界的 bug)
    let size = clamp_size(desired_size(state, &monitor), work_size, scale);
    let _ = window.set_size(size);

    // 位置:有历史且仍在屏内则保留(但用新尺寸重新钳制),否则在目标工作区居中
    let on_screen = match (state.x, state.y) {
        (Some(x), Some(y)) => position_on_screen(app, x, y),
        _ => false,
    };
    let pos = desired_position(state, size, work_pos, work_size, scale, on_screen);
    let _ = window.set_position(pos);
}

/// 响应 WindowEvent::ScaleFactorChanged:面板被拖到缩放不同的显示器时,
/// 保持逻辑尺寸(视觉大小)不变,再按新工作区重新钳制。
/// 只在缩放变化时动作,不会覆盖用户在当前 DPI 下手动调整的尺寸。
pub fn handle_scale_factor_changed(
    window: &tauri::WebviewWindow,
    new_scale: f64,
    new_inner_size: PhysicalSize<u32>,
) {
    if new_scale <= 0.0 || !new_scale.is_finite() {
        return;
    }
    // 事件自带的 new_inner_size 即为新 scale 下的物理尺寸;个别平台可能给 0,此时回读窗口
    let current = if new_inner_size.width > 0 && new_inner_size.height > 0 {
        new_inner_size
    } else {
        match window.inner_size() {
            Ok(s) => s,
            Err(_) => return,
        }
    };

    // physical → logical(用新 scale)后重新按逻辑尺寸设置;tao 会按新 scale 换算,
    // 于是逻辑尺寸不变,面板在不同 DPI 显示器上“看起来一样大”
    let logical_w = current.width as f64 / new_scale;
    let logical_h = current.height as f64 / new_scale;
    let _ = window.set_size(tauri::LogicalSize::new(logical_w, logical_h));

    let Some(monitor) = window.current_monitor().ok().flatten() else {
        return;
    };
    let scale = monitor.scale_factor();
    let (work_pos, work_size) = work_area(&monitor);

    // 目标物理尺寸 = 刚锚定的逻辑尺寸 × 当前 scale;仅在越出工作区时才改
    let target = clamp_size(
        PhysicalSize::new(
            (logical_w * scale).round().max(1.0) as u32,
            (logical_h * scale).round().max(1.0) as u32,
        ),
        work_size,
        scale,
    );
    if target.width != current.width || target.height != current.height {
        let _ = window.set_size(target);
    }

    // 位置越界就拉回工作区(尽量不移动)
    if let Ok(pos) = window.outer_position() {
        let margin = (MARGIN * scale).round() as i32;
        let min_x = work_pos.x + margin;
        let min_y = work_pos.y + margin;
        let max_x = (work_pos.x + work_size.width as i32 - target.width as i32 - margin).max(min_x);
        let max_y =
            (work_pos.y + work_size.height as i32 - target.height as i32 - margin).max(min_y);
        let nx = pos.x.clamp(min_x, max_x);
        let ny = pos.y.clamp(min_y, max_y);
        if nx != pos.x || ny != pos.y {
            let _ = window.set_position(PhysicalPosition::new(nx, ny));
        }
    }
}

/// 目标显示器:弹出面板贴合用户“此刻”的操作位置。
/// 顺序为 鼠标所在显示器 → 主显示器 → 列表第一个;任一环节失败均继续降级,最终可能返回 None。
fn pick_target_monitor(app: &AppHandle) -> Option<Monitor> {
    if let Ok(cursor) = app.cursor_position() {
        if let Ok(monitors) = app.available_monitors() {
            if let Some(m) = monitors
                .into_iter()
                .find(|m| contains_point(m, cursor.x, cursor.y))
            {
                return Some(m);
            }
        }
    }
    if let Ok(Some(m)) = app.primary_monitor() {
        return Some(m);
    }
    app.available_monitors()
        .ok()
        .and_then(|v| v.into_iter().next())
}

/// 物理点是否落在显示器矩形内(用于找出鼠标所在显示器)
fn contains_point(m: &Monitor, x: f64, y: f64) -> bool {
    let p = m.position();
    let s = m.size();
    x >= p.x as f64
        && y >= p.y as f64
        && x < p.x as f64 + s.width as f64
        && y < p.y as f64 + s.height as f64
}

/// 工作区(物理像素):位置 + 尺寸,已排除任务栏。
/// 优先用 `Monitor::work_area()`(Tauri 2.11 返回 `PhysicalRect<i32, u32>`);
/// 个别虚拟显示器/远程会话会给出 0 尺寸的工作区,此时退化为“显示器尺寸 − 底部 48px”,
/// 保证后续钳制仍有意义而不是把窗口压成 0。
fn work_area(monitor: &Monitor) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    let rect = monitor.work_area();
    if rect.size.width > 0 && rect.size.height > 0 {
        (rect.position, rect.size)
    } else {
        let size = monitor.size();
        let h = size.height.saturating_sub(FALLBACK_TASKBAR_H).max(1);
        (*monitor.position(), PhysicalSize::new(size.width, h))
    }
}

/// 目标物理尺寸。
/// - 有历史尺寸:先按保存时的 scale 还原为逻辑尺寸,再按当前 scale 换算 —— 跨 DPI 视觉一致;
///   旧版文件没有 scale 字段时按当前 scale 处理(物理尺寸不变),避免首次升级时尺寸跳变。
/// - 无历史尺寸:按显示器逻辑尺寸相对 1080p 基准缩放(1080p→1.0→420×520,与现状一致)。
fn desired_size(state: &WindowState, monitor: &Monitor) -> PhysicalSize<u32> {
    let scale = monitor.scale_factor().max(0.1);
    match (state.w, state.h) {
        (Some(w), Some(h)) => match state.scale {
            Some(saved) if saved > 0.0 => {
                let logical_w = w as f64 / saved;
                let logical_h = h as f64 / saved;
                PhysicalSize::new(
                    (logical_w * scale).round().max(1.0) as u32,
                    (logical_h * scale).round().max(1.0) as u32,
                )
            }
            _ => PhysicalSize::new(w, h),
        },
        _ => {
            let logical_w = monitor.size().width as f64 / scale;
            let logical_h = monitor.size().height as f64 / scale;
            let ratio = (logical_w / REF_W).min(logical_h / REF_H);
            let s = ratio.clamp(MIN_SCALE, MAX_SCALE);
            PhysicalSize::new((DESIGN_W * s).round() as u32, (DESIGN_H * s).round() as u32)
        }
    }
}

/// 把尺寸钳制进「工作区 − 四周留边」,且不小于最小尺寸(逻辑单位换算到物理)。
/// 工作区小到放不下最小尺寸时以最小尺寸为准 —— 可用性优先于“严格不越界”。
fn clamp_size(size: PhysicalSize<u32>, work: PhysicalSize<u32>, scale: f64) -> PhysicalSize<u32> {
    let margin = MARGIN * scale;
    let min_w = (MIN_W * scale).round();
    let min_h = (MIN_H * scale).round();
    let max_w = (work.width as f64 - margin * 2.0).max(min_w);
    let max_h = (work.height as f64 - margin * 2.0).max(min_h);
    PhysicalSize::new(
        (size.width as f64).clamp(min_w, max_w).round() as u32,
        (size.height as f64).clamp(min_h, max_h).round() as u32,
    )
}

/// 目标位置:有历史位置(且仍在屏内)时保留并重新钳制,否则在工作区居中。
fn desired_position(
    state: &WindowState,
    size: PhysicalSize<u32>,
    work_pos: PhysicalPosition<i32>,
    work_size: PhysicalSize<u32>,
    scale: f64,
    on_screen: bool,
) -> PhysicalPosition<i32> {
    let margin = (MARGIN * scale).round() as i32;
    let min_x = work_pos.x + margin;
    let min_y = work_pos.y + margin;
    // 右/下边界 = 工作区右下角 − 面板尺寸 − 留边;工作区过小(放不下)时退化为留边位置
    let max_x = (work_pos.x + work_size.width as i32 - size.width as i32 - margin).max(min_x);
    let max_y = (work_pos.y + work_size.height as i32 - size.height as i32 - margin).max(min_y);

    if on_screen {
        if let (Some(x), Some(y)) = (state.x, state.y) {
            return PhysicalPosition::new(x.clamp(min_x, max_x), y.clamp(min_y, max_y));
        }
    }
    // 居中
    let cx = work_pos.x + (work_size.width as i32 - size.width as i32) / 2;
    let cy = work_pos.y + (work_size.height as i32 - size.height as i32) / 2;
    PhysicalPosition::new(cx.clamp(min_x, max_x), cy.clamp(min_y, max_y))
}

/// 兜底(拿不到显示器信息:登录瞬间枚举不全等):
/// 有保存尺寸就恢复其物理值,否则 420×520 逻辑尺寸;位置交给系统居中。
fn fallback(window: &tauri::WebviewWindow, state: &WindowState) {
    match (state.w, state.h) {
        (Some(w), Some(h)) => {
            let _ = window.set_size(PhysicalSize::new(w, h));
        }
        _ => {
            let _ = window.set_size(tauri::LogicalSize::new(DESIGN_W, DESIGN_H));
        }
    }
    let _ = window.center();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_default_size_follows_new_default() {
        // 仍是旧默认 370×455 逻辑(150% 缩放下物理 555×683)→ 放大到新默认
        let mut state = WindowState {
            w: Some(555),
            h: Some(683),
            scale: Some(1.5),
            ..Default::default()
        };
        migrate_legacy_size(&mut state);
        assert_eq!(state.w, Some((420.0_f64 * 1.5).round() as u32));
        assert_eq!(state.h, Some((520.0_f64 * 1.5).round() as u32));
    }

    #[test]
    fn customized_size_is_not_touched() {
        // 用户手动调过的尺寸保持原样
        let mut state = WindowState {
            w: Some(640),
            h: Some(800),
            scale: Some(1.0),
            ..Default::default()
        };
        migrate_legacy_size(&mut state);
        assert_eq!(state.w, Some(640));
        assert_eq!(state.h, Some(800));
    }

    #[test]
    fn missing_scale_falls_back_to_physical_match() {
        // 旧文件没有 scale 字段:物理 370×455 视作逻辑值迁移
        let mut state = WindowState {
            w: Some(370),
            h: Some(455),
            scale: None,
            ..Default::default()
        };
        migrate_legacy_size(&mut state);
        assert_eq!(state.w, Some(420));
        assert_eq!(state.h, Some(520));
    }
}
