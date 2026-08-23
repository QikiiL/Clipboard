import { useState, useEffect } from 'react';
import { XIcon } from './icons';

interface Props {
  isOpen: boolean;
  onChoice: (choice: 'close' | 'minimize', remember: boolean) => void;
  onClose: () => void;
}

export function CloseConfirmDialog({ isOpen, onChoice, onClose }: Props) {
  const [remember, setRemember] = useState(false);

  useEffect(() => {
    if (!isOpen) {
      setRemember(false);
      return;
    }
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-surface rounded-[14px] shadow-dialog border border-hairline w-full max-w-sm mx-4 overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-5 pt-3.5 pb-3 border-b border-hairline">
          <h2 className="text-sm font-semibold">关闭确认</h2>
          <button
            onClick={onClose}
            className="flex items-center justify-center w-[26px] h-[26px] rounded-lg text-faint hover:bg-app hover:text-muted transition-colors"
          >
            <XIcon size={13} />
          </button>
        </div>

        {/* Body */}
        <div className="px-5 py-4 space-y-4">
          <p className="text-[13px] text-ink">
            您希望如何处理窗口?
          </p>

          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={remember}
              onChange={(e) => setRemember(e.target.checked)}
              className="w-3.5 h-3.5 rounded-sm accent-accent"
            />
            <span className="text-[13px] text-ink">
              记住我的选择
            </span>
          </label>
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 px-5 pt-3 pb-4 border-t border-hairline">
          <button
            onClick={() => onChoice('close', remember)}
            className="h-[31px] px-4 text-[12.5px] rounded-[10px] bg-danger text-on-accent hover:bg-danger-deep transition-colors duration-150"
          >
            关闭应用
          </button>
          <button
            onClick={() => onChoice('minimize', remember)}
            className="h-[31px] px-4 text-[12.5px] rounded-[10px] shadow-lift bg-accent text-on-accent font-medium hover:bg-accent-deep transition-colors duration-150"
          >
            最小化到托盘
          </button>
        </div>
      </div>
    </div>
  );
}
