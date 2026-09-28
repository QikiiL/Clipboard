import { useState, useEffect, useRef } from 'react';
import { useClipboardStore } from '../stores/clipboardStore';
import { useDebounce } from '../hooks/useDebounce';
import { SearchIcon, XIcon } from './icons';

export function SearchBar() {
  const [inputValue, setInputValue] = useState('');
  const debouncedQuery = useDebounce(inputValue, 300);
  const setSearchQuery = useClipboardStore((s) => s.setSearchQuery);
  const storeQuery = useClipboardStore((s) => s.searchQuery);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setSearchQuery(debouncedQuery);
  }, [debouncedQuery, setSearchQuery]);

  // 反向同步:store 被外部清空(空状态的「清空搜索」按钮)时输入框跟着清,
  // 否则列表已恢复全部、输入框里却还留着旧关键词
  useEffect(() => {
    if (storeQuery === '') setInputValue('');
  }, [storeQuery]);

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
    <div className="px-4 pt-3 pb-2.5 @max-narrow:px-2.5 @max-narrow:pt-2 @max-narrow:pb-2 @min-wide:px-5 @min-wide:pt-3.5 @min-wide:pb-3">
      <div className="flex items-center gap-2 h-9 rounded-[10px] bg-surface border border-hairline px-3 text-faint transition-[border-color,box-shadow] duration-150 focus-within:border-accent focus-within:ring-[3px] focus-within:ring-accent-ring @max-narrow:h-8 @max-narrow:px-2.5 @min-wide:h-10 @min-wide:px-3.5">
        <SearchIcon size={14} />
        <input
          ref={inputRef}
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          placeholder="搜索剪贴板…"
          className="flex-1 min-w-0 bg-transparent border-none outline-none text-[13px] text-ink placeholder:text-faint @max-narrow:text-[12px] @min-wide:text-[14px]"
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
