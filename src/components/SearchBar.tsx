import { useState, useEffect, useRef } from 'react';
import { useClipboardStore } from '../stores/clipboardStore';
import { useDebounce } from '../hooks/useDebounce';
import { SearchIcon, XIcon } from './icons';

export function SearchBar() {
  const [inputValue, setInputValue] = useState('');
  const debouncedQuery = useDebounce(inputValue, 300);
  const { setSearchQuery } = useClipboardStore();
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setSearchQuery(debouncedQuery);
  }, [debouncedQuery, setSearchQuery]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        e.preventDefault();
        inputRef.current?.focus();
      }
      if (e.key === 'Escape') {
        // 仅在搜索框聚焦时响应,避免对话框打开时按 Esc 关弹窗把搜索词也清掉
        if (document.activeElement === inputRef.current) {
          setInputValue('');
          inputRef.current?.blur();
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <div className="px-4 pt-3 pb-2.5">
      <div className="flex items-center gap-2 h-9 rounded-xl bg-surface shadow-lift px-3 text-faint focus-within:outline-2 focus-within:outline-accent-soft">
        <SearchIcon size={14} />
        <input
          ref={inputRef}
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          placeholder="搜索剪贴板…"
          className="flex-1 min-w-0 bg-transparent border-none outline-none text-[13px] text-ink placeholder:text-faint"
        />
        {inputValue && (
          <button
            onClick={() => setInputValue('')}
            className="flex items-center p-0.5 rounded hover:text-muted transition-colors"
            title="清空"
          >
            <XIcon size={12} />
          </button>
        )}
      </div>
    </div>
  );
}
