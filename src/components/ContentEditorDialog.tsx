import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ClipboardItem as ClipboardItemType } from '../types/clipboard';
import { CopyIcon, XIcon } from './icons';

/** 查看 / 编辑条目的完整内容。item 为 null 时不渲染;
 *  仅文本类条目由列表的「眼睛」按钮打开(图片条目不提供入口)。 */
export function ContentEditorDialog({
  item,
  onClose,
}: {
  item: ClipboardItemType | null;
  onClose: () => void;
}) {
  const [text, setText] = useState('');
  const [selection, setSelection] = useState({ start: 0, end: 0 });
  const [autoClose, setAutoClose] = useState(true);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const areaRef = useRef<HTMLTextAreaElement>(null);
  const copyTimer = useRef<number | undefined>(undefined);

  // item 变化(打开新条目)时重置草稿与全部临时状态,避免残留上一条的选区/错误
  useEffect(() => {
    if (!item) return;
    setText(item.content);
    setSelection({ start: 0, end: 0 });
    setCopied(false);
    setError(null);
    setSaving(false);
    // 聚焦但不全选:全选会让用户随手输入就覆盖原内容
    areaRef.current?.focus();
  }, [item]);

  // Esc 关闭。挂在 window 上而非面板上,保证焦点在 textarea 内也能命中
  useEffect(() => {
    if (!item) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [item, onClose]);

  useEffect(() => () => window.clearTimeout(copyTimer.current), []);

  const syncSelection = useCallback(() => {
    const el = areaRef.current;
    if (!el) return;
    setSelection({ start: el.selectionStart, end: el.selectionEnd });
  }, []);

  const hasSelection = selection.end > selection.start;

  const handleCopy = async () => {
    const el = areaRef.current;
    if (!el) return;
    const selected = el.value.slice(el.selectionStart, el.selectionEnd);
    if (!selected) return;
    setError(null);
    try {
      await invoke('write_clipboard_text', { text: selected });
      if (autoClose) {
        onClose();
        return;
      }
      // 不自动关闭时给一次短暂的成功反馈
      setCopied(true);
      window.clearTimeout(copyTimer.current);
      copyTimer.current = window.setTimeout(() => setCopied(false), 1200);
    } catch (err) {
      setError(String(err));
    }
  };

  // Ctrl+C 原生复制同样算「复制选中」:勾选自动关闭时也关窗。
  // 剪贴板写入发生在 copy 事件派发完成之后,故延后一拍再关,
  // 确保复制先落地、关闭后主程序的剪贴板监控才能捕获到新内容
  const handleNativeCopy = useCallback(() => {
    if (autoClose) window.setTimeout(onClose, 0);
  }, [autoClose, onClose]);

  const handleSave = async () => {
    if (!item || saving) return;
    setSaving(true);
    setError(null);
    try {
      await invoke('update_item_content', { id: item.id, content: text });
      onClose();
    } catch (err) {
      setError(String(err));
      setSaving(false);
    }
  };

  if (!item) return null;

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center bg-black/50 clip-fade-in"
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="bg-surface rounded-[14px] shadow-dialog border border-hairline w-full max-w-[560px] mx-4 overflow-hidden flex flex-col max-h-[88vh] clip-pop-in">
        <div className="flex items-center justify-between px-5 pt-3.5 pb-3 border-b border-hairline">
          <h3 className="text-sm font-semibold">查看 / 编辑内容</h3>
          <button
            onClick={onClose}
            className="flex items-center justify-center w-[26px] h-[26px] rounded-[7px] text-faint hover:bg-hairline hover:text-muted transition-colors"
            title="关闭"
          >
            <XIcon size={14} />
          </button>
        </div>

        {/* min-h + overflow-y-auto:极矮窗口下 textarea 的 160px 下限可能
            撑破面板 88vh 上限,让内容区自己滚动,保证底部按钮永远可见 */}
        <div className="px-5 py-4 flex-1 min-h-0 overflow-y-auto">
          <textarea
            ref={areaRef}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onSelect={syncSelection}
            onKeyUp={syncSelection}
            onMouseUp={syncSelection}
            onCopy={handleNativeCopy}
            className="w-full h-[46vh] min-h-[160px] px-3 py-2 rounded-[10px] border border-hairline bg-app text-[13px] leading-relaxed text-ink outline-none resize-none overflow-y-auto focus:border-accent transition-colors"
          />
          {error && <p className="mt-2 text-[11.5px] text-danger">{error}</p>}
        </div>

        {/* 底部固定两行(选项一行、按钮一行):窄面板下横向一行会被挤到
            每个字竖排折行(实测 370px 面板溢出),按钮一律 nowrap 防折行 */}
        <div className="px-5 pt-3 pb-4 border-t border-hairline">
          <label className="flex items-start gap-2 text-[12.5px] text-muted cursor-pointer select-none">
            <input
              type="checkbox"
              checked={autoClose}
              onChange={(e) => setAutoClose(e.target.checked)}
              style={{ accentColor: 'var(--accent)' }}
              className="mt-px flex-shrink-0"
            />
            复制选中后自动关闭此窗口
          </label>
          <div className="flex items-center justify-end gap-2 mt-3">
            <button
              onClick={() => void handleCopy()}
              disabled={!hasSelection}
              className={`flex items-center gap-1.5 h-[31px] px-3 text-[12.5px] whitespace-nowrap rounded-[10px] transition-colors duration-150 ${
                hasSelection
                  ? 'text-muted hover:bg-app'
                  : 'text-faint cursor-not-allowed'
              }`}
            >
              <CopyIcon size={13} />
              {copied ? '已复制' : '复制选中'}
            </button>
            <button
              onClick={onClose}
              className="h-[31px] px-4 text-[12.5px] whitespace-nowrap rounded-[10px] text-muted hover:bg-app transition-colors duration-150"
            >
              取消
            </button>
            <button
              onClick={() => void handleSave()}
              disabled={saving}
              className={`h-[31px] px-5 text-[12.5px] whitespace-nowrap rounded-[10px] font-medium transition-colors duration-150 ${
                saving
                  ? 'bg-accent-soft text-faint cursor-not-allowed'
                  : 'bg-accent text-on-accent hover:bg-accent-deep'
              }`}
            >
              保存
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
