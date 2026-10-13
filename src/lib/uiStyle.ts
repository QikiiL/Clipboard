/**
 * 界面风格(浮笺 / 瑞士索引 / 雾绿工作台)的纯函数与常量。
 * 风格只影响渲染:状态由 ThemeContext 持有,落 localStorage['ui-style'],
 * 再写成 <html data-style="…"> 供 globals.css 的属性选择器分层。
 * 这里不碰 DOM 之外的东西,便于单测(见 __tests__/uiStyle.test.ts)。
 */

export type UiStyle = 'fuji' | 'swiss' | 'mist';

/** localStorage 键名;与 theme 同级(都是纯前端渲染偏好,不进 AppSettings) */
export const STYLE_KEY = 'ui-style';

export const UI_STYLES = [
  { value: 'fuji', label: '浮笺' },
  { value: 'swiss', label: '瑞士索引' },
  { value: 'mist', label: '雾绿工作台' },
] as const;

/** 未知 / 缺失 / 脏值一律回落浮笺,老用户零迁移 */
export function normalizeStyle(v: string | null): UiStyle {
  return v === 'swiss' || v === 'mist' ? v : 'fuji';
}

export function readStoredStyle(): UiStyle {
  return normalizeStyle(localStorage.getItem(STYLE_KEY));
}

/**
 * 虚拟列表的行高估计(px)。浮笺保持现状 55;A 瑞士索引行高 52(方案 §3.3);
 * C 雾绿工作台 60 + 12px 行间隙 = 72。
 * 只是"估计":真实行高仍由 measureElement 量出,风格切换后由调用方
 * virtualizer.measure() 重算,否则已滚动的长列表会按旧估计错位。
 */
const ROW_ESTIMATE: Record<UiStyle, number> = { fuji: 55, swiss: 52, mist: 72 };

export function rowEstimate(style: UiStyle): number {
  return ROW_ESTIMATE[style];
}
