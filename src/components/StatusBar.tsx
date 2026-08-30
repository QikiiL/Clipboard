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

// 排除原因 → 状态栏文案。用「未记录」而不是「已排除」:后者是内部术语,
// 普通用户看不懂"排除"意味着什么,「未记录」直接说明结果
const EXCLUSION_LABELS: Record<string, string> = {
  app: '未记录:来自黑名单软件',
  pattern: '未记录:命中匹配规则',
  sensitive: '未记录:疑似密钥或卡号',
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

  // 主动关闭(知道了按钮)
  const handleDismissExcluded = () => setExcluded(null);

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

      {/* 排除提示:屏幕中央的浮层,不再压住状态栏或列表。
          外层 pointer-events-none 让背景仍可点;卡片本身 pointer-events-auto 让按钮可点。
          8 秒自动消失,也可点"知道了"主动关闭 */}
      {excluded && (
        <div
          className="fixed inset-0 z-[9999] pointer-events-none flex items-center justify-center"
          role="alertdialog"
          aria-label="排除规则提示"
        >
          <div className="pointer-events-auto bg-surface rounded-[14px] shadow-dialog border border-hairline p-5 max-w-[360px] flex flex-col items-center gap-3">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-warn flex-shrink-0" aria-hidden="true" />
              <span className="text-[14px] font-semibold text-ink">
                {EXCLUSION_LABELS[excluded.reason] ?? '未记录'}
              </span>
            </div>
            <p className="text-[12.5px] text-faint text-center leading-relaxed">
              这段内容符合排除规则,没有保存到历史里。如果是误判,可以点下面的按钮找回。
            </p>
            <div className="flex items-center gap-2 mt-1">
              <button
                onClick={handleDismissExcluded}
                className="h-[32px] px-4 text-[12.5px] rounded-[9px] text-muted hover:bg-hairline transition-colors duration-150"
              >
                知道了
              </button>
              <button
                onClick={handleKeepExcluded}
                className="h-[32px] px-4 text-[12.5px] rounded-[9px] bg-warn text-on-warn font-medium hover:bg-warn-deep transition-colors duration-150"
              >
                仍要记录
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
