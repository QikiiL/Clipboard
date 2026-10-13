import { createContext, useContext, useState, useEffect, useMemo, useCallback, type ReactNode } from 'react';
import { STYLE_KEY, readStoredStyle, type UiStyle } from '../lib/uiStyle';

type Theme = 'light' | 'dark';

interface ThemeContextValue {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
  /** 界面风格(浮笺/瑞士索引/雾绿工作台),写成 <html data-style> */
  style: UiStyle;
  setStyle: (style: UiStyle) => void;
}

const ThemeContext = createContext<ThemeContextValue | undefined>(undefined);

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<Theme>(() => {
    const stored = localStorage.getItem('theme');
    if (stored === 'light' || stored === 'dark') return stored;
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  });
  const [style, setStyle] = useState<UiStyle>(readStoredStyle);

  // theme 与 style 写的是同一个 DOM 根节点,放一个 effect 里一起写
  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle('dark', theme === 'dark');
    root.dataset.style = style; // 含 fuji,恒写(便于调试与选择器书写)
    localStorage.setItem('theme', theme);
    localStorage.setItem(STYLE_KEY, style);
  }, [theme, style]);

  const toggleTheme = useCallback(() => setTheme((t) => (t === 'light' ? 'dark' : 'light')), []);
  const value = useMemo(
    () => ({ theme, setTheme, toggleTheme, style, setStyle }),
    [theme, toggleTheme, style]
  );

  return (
    <ThemeContext.Provider value={value}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) throw new Error('useTheme must be used within ThemeProvider');
  return context;
}
