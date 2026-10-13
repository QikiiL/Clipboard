import { afterEach, describe, expect, it } from 'vitest';
import { STYLE_KEY, UI_STYLES, normalizeStyle, readStoredStyle } from '../uiStyle';

describe('normalizeStyle', () => {
  it('三个合法风格 id 原样返回', () => {
    expect(normalizeStyle('fuji')).toBe('fuji');
    expect(normalizeStyle('swiss')).toBe('swiss');
    expect(normalizeStyle('mist')).toBe('mist');
  });

  it('缺失值(null)回落浮笺', () => {
    expect(normalizeStyle(null)).toBe('fuji');
  });

  it('未知值回落浮笺', () => {
    expect(normalizeStyle('dark')).toBe('fuji');
    expect(normalizeStyle('A')).toBe('fuji');
    expect(normalizeStyle('swiss ')).toBe('fuji');
    expect(normalizeStyle('')).toBe('fuji');
  });

  it('大小写敏感:大写不当作合法值', () => {
    expect(normalizeStyle('Swiss')).toBe('fuji');
    expect(normalizeStyle('FUJI')).toBe('fuji');
  });
});

describe('readStoredStyle', () => {
  afterEach(() => localStorage.removeItem(STYLE_KEY));

  it('localStorage 键名固定为 ui-style', () => {
    expect(STYLE_KEY).toBe('ui-style');
  });

  it('已存合法值 → 读回该值', () => {
    localStorage.setItem(STYLE_KEY, 'mist');
    expect(readStoredStyle()).toBe('mist');
  });

  it('没有该键(老用户) → 浮笺', () => {
    expect(readStoredStyle()).toBe('fuji');
  });

  it('存了脏值 → 浮笺', () => {
    localStorage.setItem(STYLE_KEY, '{"style":"swiss"}');
    expect(readStoredStyle()).toBe('fuji');
  });

  it('清掉该键后刷新 → 浮笺', () => {
    localStorage.setItem(STYLE_KEY, 'swiss');
    expect(readStoredStyle()).toBe('swiss');
    localStorage.removeItem(STYLE_KEY);
    expect(readStoredStyle()).toBe('fuji');
  });
});

describe('UI_STYLES', () => {
  it('三项且顺序为 浮笺 / 瑞士索引 / 雾绿工作台', () => {
    expect(UI_STYLES.map((s) => s.value)).toEqual(['fuji', 'swiss', 'mist']);
    expect(UI_STYLES.map((s) => s.label)).toEqual(['浮笺', '瑞士索引', '雾绿工作台']);
  });
});
