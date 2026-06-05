import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTheme } from '../contexts/ThemeContext';
import type { AppSettings } from '../types/settings';
import { DEFAULT_SETTINGS } from '../types/settings';

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

export function SettingsPanel({ isOpen, onClose }: Props) {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const { theme, toggleTheme } = useTheme();

  useEffect(() => {
    if (isOpen) {
      invoke<AppSettings>('load_settings').then(setSettings).catch(console.error);
    }
  }, [isOpen]);

  const handleSave = async () => {
    try {
      await invoke('save_settings', { settings });
      await invoke('pause_monitoring', { paused: settings.paused });
      await invoke('register_hotkey', {
        modifier: settings.hotkey_modifier,
        key: settings.hotkey_key,
      });
      onClose();
    } catch (err) {
      console.error('Save settings failed:', err);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white dark:bg-gray-800 rounded-xl shadow-xl w-full max-w-md mx-4 overflow-hidden">
        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
          <h2 className="text-lg font-semibold">⚙️ 设置</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600">✕</button>
        </div>

        <div className="px-6 py-4 space-y-4">
          {/* 主题 */}
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">深色模式</span>
            <button
              onClick={toggleTheme}
              className={`relative w-12 h-6 rounded-full transition-colors ${
                theme === 'dark' ? 'bg-blue-500' : 'bg-gray-300'
              }`}
            >
              <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white transition-transform ${
                theme === 'dark' ? 'translate-x-6' : ''
              }`} />
            </button>
          </div>

          {/* 保留天数 */}
          <div>
            <label className="text-sm font-medium">历史保留天数</label>
            <p className="text-xs text-gray-500 mb-1">0 表示永久保留</p>
            <input
              type="number"
              value={settings.retention_days}
              onChange={(e) => setSettings({ ...settings, retention_days: Number(e.target.value) })}
              className="w-full px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900"
              min={0}
            />
          </div>

          {/* 最大条数 */}
          <div>
            <label className="text-sm font-medium">最大存储条数</label>
            <p className="text-xs text-gray-500 mb-1">0 表示无限制</p>
            <input
              type="number"
              value={settings.max_item_count}
              onChange={(e) => setSettings({ ...settings, max_item_count: Number(e.target.value) })}
              className="w-full px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900"
              min={0}
            />
          </div>

          {/* 热键 */}
          <div>
            <label className="text-sm font-medium">全局热键</label>
            <p className="text-xs text-gray-500 mb-1">唤出窗口的快捷键</p>
            <div className="flex gap-2">
              <input
                type="text"
                value={settings.hotkey_modifier}
                onChange={(e) => setSettings({ ...settings, hotkey_modifier: e.target.value })}
                className="flex-1 px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900"
                placeholder="Ctrl+Shift"
              />
              <input
                type="text"
                value={settings.hotkey_key}
                onChange={(e) => setSettings({ ...settings, hotkey_key: e.target.value })}
                className="w-20 px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900"
                placeholder="V"
              />
            </div>
          </div>

          {/* 开机自启 */}
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">开机自启动</span>
            <button
              onClick={() => setSettings({ ...settings, start_with_windows: !settings.start_with_windows })}
              className={`relative w-12 h-6 rounded-full transition-colors ${
                settings.start_with_windows ? 'bg-blue-500' : 'bg-gray-300'
              }`}
            >
              <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white transition-transform ${
                settings.start_with_windows ? 'translate-x-6' : ''
              }`} />
            </button>
          </div>

          {/* 暂停监听 */}
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium">暂停监听</span>
            <button
              onClick={() => setSettings({ ...settings, paused: !settings.paused })}
              className={`relative w-12 h-6 rounded-full transition-colors ${
                settings.paused ? 'bg-blue-500' : 'bg-gray-300'
              }`}
            >
              <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white transition-transform ${
                settings.paused ? 'translate-x-6' : ''
              }`} />
            </button>
          </div>
        </div>

        <div className="flex justify-end gap-3 px-6 py-4 border-t border-gray-200 dark:border-gray-700">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm rounded-lg bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600"
          >
            取消
          </button>
          <button
            onClick={handleSave}
            className="px-4 py-2 text-sm rounded-lg bg-blue-500 text-white hover:bg-blue-600"
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}
