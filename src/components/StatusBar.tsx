import { useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { useClipboardStore } from '../stores/clipboardStore';
import { queryItems } from '../lib/db';
import { ArrowUpIcon } from './icons';

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// 排除原因 → 状态栏文案
const EXCLUSION_LABELS: Record<string, string> = {
  app: '已排除:来源进程',
  pattern: '已排除:匹配规则',
  sensitive: '已排除:疑似密钥',
};

export function StatusBar() {
  const items = useClipboardStore((s) => s.items);
  const isPaused = useClipboardStore((s) => s.isPaused);
  const [dbSize, setDbSize] = useState<number | null>(null);
  const [pasteMode, setPasteMode] = useState(false);
  const [appVersion, setAppVersion] = useState('');
  // 携带自增序号:连续排除同一原因时 state 值不变,不带序号的话
  // React 不会重渲染,定时器也不会重置
  const [excluded, setExcluded] = useState<{ reason: string; hash: string; seq: number } | null>(
    null
  );
  const excludeSeq = useRef(0);
  const totalCount = items.length;
  const favoriteCount = useMemo(() => items.filter((i) => i.is_favorite).length, [items]);

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => {});
  }, []);

  // 排除规则命中时不入库,若不提示就成了静默丢数据——规则配得过宽时
  // 用户会以为应用坏了,这里给一个短暂可见的提示
  useEffect(() => {
    const unlisten = listen<{ reason: string; hash: string }>('item-excluded', (event) => {
      excludeSeq.current += 1;
      setExcluded({
        reason: event.payload.reason,
        hash: event.payload.hash,
        seq: excludeSeq.current,
      });
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // 8 秒而非常见的 4 秒:这是用户唯一的补救机会。内容被排除时后端会把 hash
  // 写进 last_hash,所以错过这轮后再复制同一份内容不会有任何反应(去重直接
  // 跳过,提示条也不会再出现),得先复制点别的把 last_hash 冲掉才行 ——
  // 这个门道用户不可能自己想到。既然提示条现在带可点击的操作,就得多留点时间
  useEffect(() => {
    if (!excluded) return;
    const timer = setTimeout(() => setExcluded(null), 8000);
    return () => clearTimeout(timer);
  }, [excluded]);

  // 误判时的自救路径。没有它,被误排除的内容再复制多少次都进不了历史,
  // 用户只能整体关掉敏感识别 —— 那等于把整个功能废掉
  const handleKeepExcluded = async () => {
    if (!excluded) return;
    try {
      await invoke('allow_excluded_item', { hash: excluded.hash });
      setExcluded(null);
    } catch (err) {
      console.error('Failed to allow excluded item:', err);
    }
  };

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
    // 轮询自身感知变化即可;依赖 totalCount 会在每次条数变化时重建定时器
  }, []);

  return (
    <div className="flex items-center justify-between h-9 px-4 border-t border-hairline text-[11px] text-faint tabular-nums">
      <div className="flex items-center gap-3">
        <span>共 {totalCount} 条</span>
        <span>{favoriteCount} 收藏</span>
        {dbSize !== null && <span>{formatSize(dbSize)}</span>}
        {appVersion && <span>v{appVersion}</span>}
        {excluded && (
          <span
            className="flex items-center gap-1.5 px-1.5 py-[1px] rounded-full bg-warn/20 text-warn-text"
            title="命中排除规则,该内容未写入历史"
          >
            {EXCLUSION_LABELS[excluded.reason] ?? '已排除'}
            <button
              onClick={handleKeepExcluded}
              title="把这条内容记入历史,并加入豁免名单,以后不再排除"
              className="px-1 rounded-full bg-warn/30 hover:bg-warn/50 transition-colors"
            >
              仍要记录
            </button>
          </span>
        )}
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
