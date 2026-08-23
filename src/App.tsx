import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ThemeProvider } from './contexts/ThemeContext';
import { SearchBar } from './components/SearchBar';
import { GroupTabs } from './components/GroupTabs';
import { ClipboardList } from './components/ClipboardList';
import { StatusBar } from './components/StatusBar';
import { SettingsPanel } from './components/SettingsPanel';
import { CloseConfirmDialog } from './components/CloseConfirmDialog';
import {
  PinIcon,
  SettingsIcon,
  MinusIcon,
  MaximizeIcon,
  RestoreIcon,
  XIcon,
} from './components/icons';
import { useClipboardListener } from './hooks/useClipboardListener';
import { useDatabase } from './hooks/useDatabase';
import { UpdateDialog, type UpdateInfo } from './components/UpdateDialog';

function AppContent() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [showCloseDialog, setShowCloseDialog] = useState(false);
  const [pinned, setPinned] = useState(true);
  const [maximized, setMaximized] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const { loadItems } = useDatabase();
  useClipboardListener(loadItems);

  // 启动时静默检查更新;同一新版本只提醒一次(记录在 localStorage,
  // 窗口销毁重建后不会重复弹窗),用户可在设置里随时手动检查
  useEffect(() => {
    invoke<UpdateInfo>('check_update')
      .then((info) => {
        if (
          info.has_update &&
          localStorage.getItem('clipboard-update-dismissed') !== info.latest
        ) {
          setUpdateInfo(info);
        }
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    const win = getCurrentWindow();
    win.isMaximized().then(setMaximized).catch(console.error);
    const unlisten = win.onResized(async () => {
      try {
        setMaximized(await win.isMaximized());
      } catch (err) {
        console.error('Failed to query maximize state:', err);
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  useEffect(() => {
    const unlisten = listen('ask-close-behavior', () => {
      setShowCloseDialog(true);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  useEffect(() => {
    invoke<{ pinned?: boolean }>('load_settings')
      .then((s) => setPinned(s.pinned ?? true))
      .catch(console.error);
  }, []);

  const handleTogglePin = useCallback(async () => {
    const next = !pinned;
    try {
      await invoke('set_always_on_top', { pinned: next });
      setPinned(next);
    } catch (err) {
      console.error('Toggle pin failed:', err);
    }
  }, [pinned]);

  const handleCloseChoice = useCallback(async (choice: 'close' | 'minimize', remember: boolean) => {
    try {
      if (remember) {
        await invoke('set_close_behavior', { behavior: choice });
      }
      setShowCloseDialog(false);
      if (choice === 'minimize') {
        await invoke('hide_window');
      } else {
        await invoke('close_app');
      }
    } catch (err) {
      console.error('Close choice failed:', err);
      setShowCloseDialog(false);
    }
  }, []);

  const handleMinimize = useCallback(() => {
    getCurrentWindow().minimize().catch(console.error);
  }, []);

  const handleToggleMaximize = useCallback(() => {
    getCurrentWindow().toggleMaximize().catch(console.error);
  }, []);

  // 触发与原生标题栏关闭相同的 CloseRequested 流程(询问/最小化/关闭)
  const handleClose = useCallback(() => {
    getCurrentWindow().close().catch(console.error);
  }, []);

  const iconBtn =
    'flex items-center justify-center w-[30px] h-[30px] rounded-lg text-muted hover:bg-hairline hover:text-faint transition-colors';

  return (
    <div className="flex flex-col h-screen bg-app text-ink">
      <header
        data-tauri-drag-region
        className="flex items-center justify-between h-[46px] pl-5 pr-1.5 border-b border-hairline select-none"
      >
        <h1 data-tauri-drag-region className="text-[13px] font-semibold tracking-wide cursor-default">
          剪贴板
        </h1>
        <div className="flex items-center">
          <div className="flex items-center gap-1 pr-2.5">
            <button
              onClick={handleTogglePin}
              className={`flex items-center justify-center w-[30px] h-[30px] rounded-lg transition-colors ${
                pinned
                  ? 'bg-accent-soft text-accent'
                  : 'text-muted hover:bg-hairline hover:text-faint'
              }`}
              title={pinned ? '取消置顶' : '置顶:悬浮于所有应用之上'}
            >
              <PinIcon size={16} />
            </button>
            <button
              onClick={() => setSettingsOpen(true)}
              className={iconBtn}
              title="设置"
            >
              <SettingsIcon size={16} />
            </button>
          </div>
          <div className="flex items-center gap-0.5">
            <button onClick={handleMinimize} className={iconBtn} title="最小化">
              <MinusIcon size={14} />
            </button>
            <button onClick={handleToggleMaximize} className={iconBtn} title={maximized ? '还原' : '最大化'}>
              {maximized ? <RestoreIcon size={14} /> : <MaximizeIcon size={14} />}
            </button>
            <button onClick={handleClose} className={iconBtn} title="关闭">
              <XIcon size={14} />
            </button>
          </div>
        </div>
      </header>
      <SearchBar />
      <GroupTabs />
      <ClipboardList />
      <StatusBar />
      <SettingsPanel isOpen={settingsOpen} onClose={() => setSettingsOpen(false)} />
      <CloseConfirmDialog isOpen={showCloseDialog} onChoice={handleCloseChoice} onClose={() => setShowCloseDialog(false)} />
      {updateInfo && (
        <UpdateDialog
          info={updateInfo}
          onClose={() => {
            localStorage.setItem('clipboard-update-dismissed', updateInfo.latest);
            setUpdateInfo(null);
          }}
        />
      )}
    </div>
  );
}

function App() {
  return (
    <ThemeProvider>
      <AppContent />
    </ThemeProvider>
  );
}

export default App;
