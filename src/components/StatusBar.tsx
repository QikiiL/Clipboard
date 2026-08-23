import { useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useClipboardStore } from '../stores/clipboardStore';
import { queryItems } from '../lib/db';
import { ArrowUpIcon } from './icons';

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function StatusBar() {
  const items = useClipboardStore((s) => s.items);
  const isPaused = useClipboardStore((s) => s.isPaused);
  const [dbSize, setDbSize] = useState<number | null>(null);
  const [pasteMode, setPasteMode] = useState(false);
  const totalCount = items.length;
  const favoriteCount = useMemo(() => items.filter((i) => i.is_favorite).length, [items]);

  useEffect(() => {
    (async () => {
      try {
        const mode = await invoke<boolean>('get_paste_mode');
        setPasteMode(mode);
      } catch (err) {
        console.error('Failed to load paste mode:', err);
      }
    })();
    const unlisten = listen<boolean>('paste-mode-changed', (event) => {
      setPasteMode(event.payload);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // 复制模式仅在需要时手动切换(如资源管理器中避免误触发 Ctrl+V)
  const handleTogglePasteMode = async () => {
    const next = !pasteMode;
    try {
      await invoke('set_paste_mode', { inputMode: next });
      setPasteMode(next);
    } catch (err) {
      console.error('Failed to set paste mode:', err);
    }
  };

  useEffect(() => {
    let cancelled = false;
    const loadSize = async () => {
      try {
        const rows = await queryItems<{ size: number }>(
          "SELECT (SELECT page_count FROM pragma_page_count()) * (SELECT page_size FROM pragma_page_size()) AS size"
        );
        if (!cancelled && rows.length > 0) setDbSize(rows[0].size);
      } catch (err) {
        console.error('Failed to load db size:', err);
      }
    };
    loadSize();
    const timer = setInterval(loadSize, 10000);
    return () => { cancelled = true; clearInterval(timer); };
  }, [totalCount]);

  return (
    <div className="flex items-center justify-between h-9 px-4 border-t border-hairline text-[11px] text-faint tabular-nums">
      <div className="flex items-center gap-3">
        <span>共 {totalCount} 条</span>
        <span>{favoriteCount} 收藏</span>
        {dbSize !== null && <span>{formatSize(dbSize)}</span>}
      </div>
      <div className="flex items-center gap-3">
        <button
          onClick={handleTogglePasteMode}
          title="切换单击条目的行为(粘贴到之前点击的输入框 / 仅复制到剪贴板)"
          className={`flex items-center gap-1.5 px-[11px] py-[3px] rounded-full transition-[background-color,color,box-shadow] duration-150 ${
            pasteMode
              ? 'bg-accent-soft text-accent font-semibold ring-1 ring-inset ring-accent-ring'
              : 'bg-surface shadow-lift text-muted hover:shadow-lift-hover'
          }`}
        >
          {pasteMode && <ArrowUpIcon size={11} />}
          {pasteMode ? '单击粘贴' : '单击复制'}
        </button>
        <div className="flex items-center gap-1.5">
          <span className={`w-1.5 h-1.5 rounded-full ${isPaused ? 'bg-warn' : 'bg-ok'}`} title={isPaused ? '已暂停' : '监听中'} />
          <span>{isPaused ? '已暂停' : '监听中'}</span>
        </div>
      </div>
    </div>
  );
}
