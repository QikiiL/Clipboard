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
