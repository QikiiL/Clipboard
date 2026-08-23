import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTheme } from '../contexts/ThemeContext';
import { useClipboardStore } from '../stores/clipboardStore';
import type { AppSettings } from '../types/settings';
import { DEFAULT_SETTINGS } from '../types/settings';
import { XIcon } from './icons';
import type { UpdateInfo } from './UpdateDialog';

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

interface StorageInfo {
  data_dir: string;
  is_default: boolean;
  default_dir: string;
}

// 清除范围选项:days 为 0 表示全部,>0 表示清除 N 天前(含更早)的记录
const CLEAR_RANGES = [
  { days: 0, label: '全部' },
  { days: 90, label: '三个月前' },
  { days: 30, label: '一个月前' },
  { days: 7, label: '七天前' },
  { days: 3, label: '三天前' },
] as const;

export function SettingsPanel({ isOpen, onClose }: Props) {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [recording, setRecording] = useState(false);
  const [winVEnabled, setWinVEnabled] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [clearDays, setClearDays] = useState(0);
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null);
  const [updateChecking, setUpdateChecking] = useState(false);
  const [updateResult, setUpdateResult] = useState<UpdateInfo | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [pwdCopied, setPwdCopied] = useState(false);
  // settingsRef 始终持有最新设置,避免回调闭包读到旧值
  const settingsRef = useRef<AppSettings>(DEFAULT_SETTINGS);
  // savedRef 持有最近一次已持久化的值,用于判断数字输入是否真的改了
  const savedRef = useRef<AppSettings>(DEFAULT_SETTINGS);
  const recordingRef = useRef(false);
  const { theme, toggleTheme } = useTheme();
  const setPaused = useClipboardStore((s) => s.setPaused);

  const applySettings = useCallback((next: AppSettings) => {
    settingsRef.current = next;
    setSettings(next);
  }, []);

  useEffect(() => {
    if (isOpen) {
      invoke<AppSettings>('load_settings').then((s) => {
        // 与默认值合并,避免旧配置缺字段导致保存时丢失(如 pinned)
        const merged = { ...DEFAULT_SETTINGS, ...s };
        applySettings(merged);
        savedRef.current = merged;
        setPaused(s.paused);
        setWinVEnabled(s.win_v_integration ?? false);
      }).catch(console.error);
      setRecording(false);
      recordingRef.current = false;
      setClearConfirmOpen(false);
      setClearDays(0);
      invoke<StorageInfo>('get_storage_info')
        .then(setStorageInfo)
        .catch(console.error);
    }
  }, [isOpen, setPaused, applySettings]);

  useEffect(() => {
    if (!isOpen) return;
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (recordingRef.current) {
          setRecording(false);
          recordingRef.current = false;
        } else if (clearConfirmOpen) {
          setClearConfirmOpen(false);
        } else {
          onClose();
        }
      }
    };
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen, onClose, clearConfirmOpen]);

  // 即改即存:应用新值并持久化;paused 变化时同步监听开关
  const saveNow = useCallback(async (next: AppSettings) => {
    const prev = settingsRef.current;
    applySettings(next);
    try {
      await invoke('save_settings', { settings: next });
      savedRef.current = next;
      if (prev.paused !== next.paused) {
        await invoke('pause_monitoring', { paused: next.paused });
        setPaused(next.paused);
      }
      // 列表查询上限跟随设置(0=不限制)
      useClipboardStore
        .getState()
        .setMaxItems(next.max_item_count > 0 ? next.max_item_count : 100000);
    } catch (err) {
      console.error('Auto-save settings failed:', err);
    }
  }, [applySettings, setPaused]);

  // 热键类修改需先注册成功再保存,失败则回读后端恢复原值
  const saveHotkey = useCallback(async (next: AppSettings) => {
    applySettings(next);
    try {
      await invoke('register_hotkey', {
        modifier: next.hotkey_modifier,
        key: next.hotkey_key,
      });
      await saveNow(next);
    } catch (err) {
      console.error('Register hotkey failed:', err);
      alert('快捷键注册失败: ' + err);
      invoke<AppSettings>('load_settings')
        .then((s) => {
          const merged = { ...DEFAULT_SETTINGS, ...s };
          applySettings(merged);
          savedRef.current = merged;
        })
        .catch(console.error);
    }
  }, [applySettings, saveNow]);

  // Hotkey recording handler
  const handleRecordKeyDown = useCallback((e: KeyboardEvent) => {
    if (!recordingRef.current) return;
    e.preventDefault();
    e.stopPropagation();

    const modifiers: string[] = [];
    if (e.ctrlKey) modifiers.push('Ctrl');
    if (e.altKey) modifiers.push('Alt');
    if (e.shiftKey) modifiers.push('Shift');
    if (e.metaKey) modifiers.push('Super');

    const modifierKeys = ['Control', 'Alt', 'Shift', 'Meta'];
    if (modifierKeys.includes(e.key)) return;

    // Only accept single letter keys (A-Z) and digits (0-9)
    const isValidKey = /^[a-zA-Z0-9]$/.test(e.key);
    if (!isValidKey) return;

    const modifier = modifiers.join('+');
    const mainKey = e.key.toUpperCase();
    const next = { ...settingsRef.current, hotkey_modifier: modifier, hotkey_key: mainKey };
    setRecording(false);
    recordingRef.current = false;
    void saveHotkey(next);
  }, [saveHotkey]);

  useEffect(() => {
    if (!recording) return;
    document.addEventListener('keydown', handleRecordKeyDown);
    return () => document.removeEventListener('keydown', handleRecordKeyDown);
  }, [recording, handleRecordKeyDown]);

  const startRecording = () => {
    setRecording(true);
    recordingRef.current = true;
  };

  const resetHotkey = () => {
    void saveHotkey({
      ...settingsRef.current,
      hotkey_modifier: DEFAULT_SETTINGS.hotkey_modifier,
      hotkey_key: DEFAULT_SETTINGS.hotkey_key,
    });
  };

  const handleToggleWinV = async () => {
    const next = !winVEnabled;
    try {
      if (next) {
        await invoke('enable_win_v_integration');
      } else {
        await invoke('disable_win_v_integration');
      }
      setWinVEnabled(next);
      // 后端已持久化该字段,同步本地镜像避免下次保存时覆盖
      applySettings({ ...settingsRef.current, win_v_integration: next });
    } catch (err) {
      console.error('[Win+V] Toggle failed:', err);
      alert('Win+V 切换失败: ' + err);
    }
  };

  const handleToggleAutostart = () => {
    void saveNow({ ...settingsRef.current, start_with_windows: !settingsRef.current.start_with_windows });
  };

  const handleTogglePaused = () => {
    void saveNow({ ...settingsRef.current, paused: !settingsRef.current.paused });
  };

  const setCloseBehavior = (value: AppSettings['close_behavior']) => {
    void saveNow({ ...settingsRef.current, close_behavior: value });
  };

  // 数字输入:失焦时若值有变则保存
  const persistNumbersIfChanged = () => {
    if (settingsRef.current !== savedRef.current) {
      void saveNow(settingsRef.current);
    }
  };

  const doClearHistory = useCallback(async () => {
    setClearConfirmOpen(false);
    try {
      setClearing(true);
      await invoke('clear_history', { days: clearDays });
    } catch (err) {
      console.error('Clear history failed:', err);
      alert('清空失败: ' + err);
    } finally {
      setClearing(false);
    }
  }, [clearDays]);

  // 更改/恢复存储位置:后端选目录+校验+写指针后重启,冷启动完成数据搬运
  const handleChangeStorage = async () => {
    if (
      !window.confirm(
        '更改存储位置将迁移数据库与图片,迁移完成后应用会自动重启。所选文件夹必须是空文件夹。继续?'
      )
    ) {
      return;
    }
    try {
      await invoke('change_storage_location');
    } catch (err) {
      alert('更改存储位置失败: ' + err);
    }
  };

  const handleResetStorage = async () => {
    if (!window.confirm('把数据迁回默认位置并重启应用,继续?')) return;
    try {
      await invoke('reset_storage_location');
    } catch (err) {
      alert('恢复默认位置失败: ' + err);
    }
  };

  // 手动检查更新:结果内联展示,下载按钮打开浏览器
  const handleCheckUpdate = async () => {
    setUpdateChecking(true);
    setUpdateError(null);
    setUpdateResult(null);
    try {
      const info = await invoke<UpdateInfo>('check_update');
      setUpdateResult(info);
    } catch (err) {
      setUpdateError(String(err));
    } finally {
      setUpdateChecking(false);
    }
  };

  const openUrl = (url: string) => {
    invoke('open_external_url', { url }).catch(console.error);
  };

  // 与 UpdateDialog 相同的双保险:点击下载自动复制密码 + 明文展示可手动复制
  const copyUpdatePassword = async () => {
    if (!updateResult?.lanzou_password) return;
    try {
      await invoke('write_clipboard_text', { text: updateResult.lanzou_password });
      setPwdCopied(true);
      setTimeout(() => setPwdCopied(false), 2000);
    } catch (err) {
      console.error('Copy password failed:', err);
    }
  };

  const displayShortcut = recording
    ? '请按下快捷键…'
    : `${settings.hotkey_modifier}+${settings.hotkey_key}`;

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-surface rounded-[14px] shadow-dialog border border-hairline w-full max-w-md mx-4 max-h-[90vh] flex flex-col overflow-hidden">
        <div className="flex items-center justify-between px-5 pt-3.5 pb-3 border-b border-hairline">
          <h2 className="text-sm font-semibold">设置</h2>
          <button
            onClick={onClose}
            className="flex items-center justify-center w-[26px] h-[26px] rounded-lg text-faint hover:bg-app hover:text-muted transition-colors"
          >
            <XIcon size={13} />
          </button>
        </div>

        <div className="px-5 py-2 space-y-4 overflow-y-auto">
          {/* 主题 */}
          <div className="flex items-center justify-between">
            <span className="text-[12.5px] font-medium">深色模式</span>
            <button
              onClick={toggleTheme}
              role="switch"
              aria-checked={theme === 'dark'}
              aria-label="深色模式"
              className={`relative w-[35px] h-5 rounded-full transition-colors duration-150 ${
                theme === 'dark' ? 'bg-accent' : 'bg-hairline'
              }`}
            >
              <span className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-surface shadow-sm transition-transform duration-150 ${
                theme === 'dark' ? 'translate-x-[15px]' : ''
              }`} />
            </button>
          </div>

          {/* 保留天数 */}
          <div>
            <label className="text-[12.5px] font-medium">历史保留天数</label>
            <p className="text-[11px] text-faint mb-1">0 表示永久保留</p>
            <input
              type="number"
              value={settings.retention_days}
              onChange={(e) => applySettings({ ...settingsRef.current, retention_days: Math.max(0, Number(e.target.value) || 0) })}
              onBlur={persistNumbersIfChanged}
              className="w-[88px] h-[29px] px-2.5 text-center rounded-[9px] border border-hairline bg-transparent text-[12.5px] tabular-nums"
              min={0}
            />
          </div>

          {/* 最大条数 */}
          <div>
            <label className="text-[12.5px] font-medium">最大存储条数</label>
            <p className="text-[11px] text-faint mb-1">0 表示无限制</p>
            <input
              type="number"
              value={settings.max_item_count}
              onChange={(e) => applySettings({ ...settingsRef.current, max_item_count: Math.max(0, Number(e.target.value) || 0) })}
              onBlur={persistNumbersIfChanged}
              className="w-[88px] h-[29px] px-2.5 text-center rounded-[9px] border border-hairline bg-transparent text-[12.5px] tabular-nums"
              min={0}
            />
          </div>

          {/* 热键 */}
          <div>
            <label className="text-[12.5px] font-medium">全局热键</label>
            <p className="text-[11px] text-faint mb-1">唤出窗口的快捷键</p>
            <div className="flex gap-2 items-center">
              <div
                onClick={winVEnabled ? undefined : startRecording}
                className={`flex-1 h-[30px] rounded-[9px] text-center font-mono text-xs select-none transition-[background-color,border-color,color] duration-150 ${
                  winVEnabled
                    ? 'border border-hairline bg-app text-faint cursor-not-allowed'
                    : recording
                      ? 'border border-accent bg-accent-soft text-accent animate-pulse cursor-pointer'
                      : 'border border-hairline bg-app text-muted hover:border-accent cursor-pointer'
                } flex items-center justify-center`}
              >
                {winVEnabled ? '已停用(Win+V 模式)' : displayShortcut}
              </div>
              <button
                onClick={winVEnabled ? undefined : resetHotkey}
                title="恢复默认快捷键"
                className={`h-[30px] px-3 text-[11px] rounded-[9px] border border-hairline transition-colors duration-150 ${
                  winVEnabled
                    ? 'bg-app text-faint cursor-not-allowed'
                    : 'bg-app text-muted hover:bg-hairline'
                }`}
              >
                重置
              </button>
            </div>
            <p className="text-[11px] text-faint mt-1">
              {recording ? '按下想要的快捷键组合…' : '点击上方区域录制新快捷键,录制完成即生效'}
            </p>
          </div>

          {/* Win+V 替代系统剪贴板 */}
          <div className="flex items-center justify-between">
            <div>
              <span className="text-[12.5px] font-medium">替代系统 Win+V</span>
              <p className="text-[11px] text-faint">禁用系统剪贴板历史,使用本应用替代</p>
            </div>
            <button
              onClick={handleToggleWinV}
              role="switch"
              aria-checked={winVEnabled}
              aria-label="Win+V 替代系统剪贴板"
              className={`relative w-[35px] h-5 rounded-full transition-colors duration-150 ${
                winVEnabled ? 'bg-accent' : 'bg-hairline'
              }`}
            >
              <span className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-surface shadow-sm transition-transform duration-150 ${
                winVEnabled ? 'translate-x-[15px]' : ''
              }`} />
            </button>
          </div>
          {winVEnabled && (
            <p className="text-[11px] text-warn-text">
              当前已启用 Win+V,自定义快捷键暂时停用。
            </p>
          )}

          {/* 开机自启 */}
          <div className="flex items-center justify-between">
            <span className="text-[12.5px] font-medium">开机自启动</span>
            <button
              onClick={handleToggleAutostart}
              role="switch"
              aria-checked={settings.start_with_windows}
              aria-label="开机自启动"
              className={`relative w-[35px] h-5 rounded-full transition-colors duration-150 ${
                settings.start_with_windows ? 'bg-accent' : 'bg-hairline'
              }`}
            >
              <span className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-surface shadow-sm transition-transform duration-150 ${
                settings.start_with_windows ? 'translate-x-[15px]' : ''
              }`} />
            </button>
          </div>

          {/* 暂停监听 */}
          <div className="flex items-center justify-between">
            <span className="text-[12.5px] font-medium">暂停监听</span>
            <button
              onClick={handleTogglePaused}
              role="switch"
              aria-checked={settings.paused}
              aria-label="暂停监听"
              className={`relative w-[35px] h-5 rounded-full transition-colors duration-150 ${
                settings.paused ? 'bg-accent' : 'bg-hairline'
              }`}
            >
              <span className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-surface shadow-sm transition-transform duration-150 ${
                settings.paused ? 'translate-x-[15px]' : ''
              }`} />
            </button>
          </div>

          {/* 数据存储位置 */}
          <div>
            <div className="flex items-center justify-between">
              <span className="text-[12.5px] font-medium">数据存储位置</span>
              <div className="flex gap-2">
                {storageInfo && !storageInfo.is_default && (
                  <button
                    onClick={handleResetStorage}
                    className="h-[30px] px-3 text-[11px] rounded-[9px] border border-hairline bg-app text-muted hover:bg-hairline transition-colors duration-150"
                  >
                    恢复默认
                  </button>
                )}
                <button
                  onClick={handleChangeStorage}
                  className="h-[30px] px-3 text-[11px] rounded-[9px] border border-hairline bg-app text-muted hover:bg-hairline transition-colors duration-150"
                >
                  更改位置
                </button>
              </div>
            </div>
            <p className="text-[11px] text-faint mt-1 break-all">
              {storageInfo ? storageInfo.data_dir : '加载中…'}
            </p>
            <p className="text-[11px] text-faint mt-1">
              数据库与图片保存在此目录,迁移在重启后进行且失败自动回退
            </p>
          </div>

          {/* 清空历史 */}
          <div className="flex items-center justify-between">
            <div>
              <span className="text-[12.5px] font-medium">清空剪贴板历史</span>
              <p className="text-[11px] text-faint">按时间范围清除未收藏的记录及图片,收藏保留</p>
            </div>
            <button
              onClick={() => setClearConfirmOpen(true)}
              disabled={clearing}
              className={`h-[30px] px-4 text-[12px] rounded-[9px] border border-hairline transition-colors duration-150 ${
                clearing
                  ? 'bg-app text-faint cursor-not-allowed'
                  : 'bg-app text-danger hover:bg-hairline'
              }`}
            >
              {clearing ? '清空中…' : '清空'}
            </button>
          </div>

          {/* 软件更新 */}
          <div>
            <div className="flex items-center justify-between">
              <span className="text-[12.5px] font-medium">软件更新</span>
              <button
                onClick={handleCheckUpdate}
                disabled={updateChecking}
                className={`h-[30px] px-4 text-[11px] rounded-[9px] border border-hairline transition-colors duration-150 ${
                  updateChecking
                    ? 'bg-app text-faint cursor-not-allowed'
                    : 'bg-app text-muted hover:bg-hairline'
                }`}
              >
                {updateChecking ? '检查中…' : '检查更新'}
              </button>
            </div>
            {updateResult && !updateResult.has_update && (
              <p className="text-[11px] text-faint mt-1">已是最新版本(v{updateResult.current})</p>
            )}
            {updateResult && updateResult.has_update && (
              <div className="mt-1.5">
                <p className="text-[11px] text-faint">
                  当前 v{updateResult.current} → 最新 <span className="text-accent">v{updateResult.latest}</span>
                </p>
                {updateResult.notes && (
                  <p className="text-[11px] text-faint mt-1 whitespace-pre-wrap">{updateResult.notes}</p>
                )}
                {updateResult.lanzou && updateResult.lanzou_password && (
                  <div className="flex items-center gap-2 mt-2 px-3 py-2 rounded-[8px] bg-app border border-hairline">
                    <span className="text-[11px] text-faint shrink-0">蓝奏云密码</span>
                    <span className="text-[13px] font-mono font-medium tracking-widest text-ink select-all truncate">
                      {updateResult.lanzou_password}
                    </span>
                    <button
                      onClick={copyUpdatePassword}
                      className="ml-auto shrink-0 h-[22px] px-2.5 text-[11px] rounded-[6px] border border-hairline text-muted hover:bg-hairline transition-colors duration-150"
                    >
                      {pwdCopied ? '已复制' : '复制'}
                    </button>
                  </div>
                )}
                <div className="flex gap-2 mt-2">
                  {updateResult.lanzou && (
                    <button
                      onClick={() => {
                        if (updateResult.lanzou_password) void copyUpdatePassword();
                        openUrl(updateResult.lanzou!);
                      }}
                      className="h-[28px] px-3 text-[11px] rounded-[8px] border border-hairline text-muted hover:bg-hairline transition-colors duration-150"
                    >
                      蓝奏云下载
                    </button>
                  )}
                  {updateResult.github && (
                    <button
                      onClick={() => openUrl(updateResult.github!)}
                      className="h-[28px] px-3 text-[11px] rounded-[8px] border border-hairline text-muted hover:bg-hairline transition-colors duration-150"
                    >
                      GitHub 下载
                    </button>
                  )}
                </div>
              </div>
            )}
            {updateError && (
              <p className="text-[11px] text-faint mt-1">{updateError}(无网络或版本服务不可用)</p>
            )}
          </div>

          {/* 关闭行为 */}
          <div>
            <span className="text-[12.5px] font-medium">关闭行为</span>
            <div className="flex mt-2 gap-[3px] p-[3px] rounded-[10px] bg-app">
              {([
                { value: 'ask', label: '询问' },
                { value: 'minimize', label: '最小化到托盘' },
                { value: 'close', label: '直接关闭' },
              ] as const).map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => setCloseBehavior(opt.value)}
                  className={`flex-1 h-[27px] text-[11.5px] rounded-[7px] transition-[background-color,color,box-shadow] duration-150 ${
                    settings.close_behavior === opt.value
                      ? 'bg-surface text-accent font-semibold shadow-lift'
                      : 'text-muted hover:text-faint'
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="px-5 pt-3 pb-4 border-t border-hairline">
          <span className="text-[11px] text-faint">更改即时保存,无需手动确认</span>
        </div>
      </div>

      {/* 清空历史确认弹窗(居中) */}
      {clearConfirmOpen && (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/50">
          <div className="bg-surface rounded-[14px] shadow-dialog border border-hairline w-full max-w-xs mx-4 overflow-hidden">
            <div className="px-5 pt-3.5 pb-3 border-b border-hairline">
              <h3 className="text-sm font-semibold">清空剪贴板历史</h3>
            </div>
            <div className="px-5 py-4">
              <p className="text-[12px] font-medium mb-2">清除范围</p>
              <div className="flex gap-[3px] p-[3px] rounded-[10px] bg-app">
                {CLEAR_RANGES.map((opt) => (
                  <button
                    key={opt.days}
                    onClick={() => setClearDays(opt.days)}
                    className={`flex-1 h-[26px] text-[11px] rounded-[7px] transition-[background-color,color,box-shadow] duration-150 ${
                      clearDays === opt.days
                        ? 'bg-surface text-accent font-semibold shadow-lift'
                        : 'text-muted hover:text-faint'
                    }`}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
              <p className="text-[12.5px] leading-relaxed text-ink mt-3">
                {clearDays === 0
                  ? '将删除除收藏外的全部记录及其图片文件。'
                  : `将删除"${CLEAR_RANGES.find((r) => r.days === clearDays)?.label}"及更早的未收藏记录及其图片文件。`}
                此操作不可恢复,收藏的条目(含图片)会保留。
              </p>
            </div>
            <div className="flex justify-end gap-2 px-5 pt-3 pb-4 border-t border-hairline">
              <button
                onClick={() => setClearConfirmOpen(false)}
                className="h-[31px] px-4 text-[12.5px] rounded-[10px] text-muted hover:bg-app transition-colors duration-150"
              >
                取消
              </button>
              <button
                onClick={doClearHistory}
                className="h-[31px] px-4 text-[12.5px] rounded-[10px] bg-danger text-on-accent font-medium hover:bg-danger-deep transition-colors duration-150"
              >
                清空
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
