import { useEffect, useRef, useState } from 'react';

const overlayClass = 'fixed inset-0 z-[60] flex items-center justify-center bg-black/50';
const panelClass =
  'bg-surface rounded-[14px] shadow-dialog border border-hairline w-full max-w-xs mx-4 overflow-hidden';

/** 输入型对话框:替代 WebView2 下渲染残缺的原生 window.prompt */
export function PromptDialog({
  title,
  label,
  initialValue = '',
  placeholder,
  confirmText = '确认',
  onConfirm,
  onClose,
}: {
  title: string;
  label?: string;
  initialValue?: string;
  placeholder?: string;
  confirmText?: string;
  onConfirm: (value: string) => Promise<void>;
  onClose: () => void;
}) {
  const [value, setValue] = useState(initialValue);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const submit = async () => {
    if (!value.trim() || submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      await onConfirm(value.trim());
    } catch (err) {
      setError(String(err));
      setSubmitting(false);
    }
  };

  return (
    <div className={overlayClass} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={panelClass} onKeyDown={(e) => e.key === 'Escape' && onClose()}>
        <div className="px-5 pt-3.5 pb-3 border-b border-hairline">
          <h3 className="text-sm font-semibold">{title}</h3>
        </div>
        <div className="px-5 py-4">
          {label && <p className="text-[13px] text-ink mb-2">{label}</p>}
          <input
            ref={inputRef}
            value={value}
            placeholder={placeholder}
            maxLength={30}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && void submit()}
            className="w-full h-[33px] px-3 rounded-[10px] border border-hairline bg-app text-[13px] text-ink outline-none focus:border-accent transition-colors"
          />
          {error && <p className="mt-2 text-[11.5px] text-danger">{error}</p>}
        </div>
        <div className="flex justify-end gap-2 px-5 pt-3 pb-4 border-t border-hairline">
          <button
            onClick={onClose}
            className="h-[31px] px-4 text-[12.5px] rounded-[10px] text-muted hover:bg-app transition-colors duration-150"
          >
            取消
          </button>
          <button
            onClick={() => void submit()}
            disabled={!value.trim() || submitting}
            className={`h-[31px] px-5 text-[12.5px] rounded-[10px] font-medium transition-colors duration-150 ${
              !value.trim() || submitting
                ? 'bg-accent-soft text-faint cursor-not-allowed'
                : 'bg-accent text-on-accent hover:bg-accent-deep'
            }`}
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}

/** 确认型对话框:替代原生 window.confirm,danger 风格确认按钮 */
export function ConfirmDialog({
  title,
  message,
  confirmText = '删除',
  onConfirm,
  onClose,
}: {
  title: string;
  message: string;
  confirmText?: string;
  onConfirm: () => Promise<void>;
  onClose: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const submit = async () => {
    if (submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      await onConfirm();
    } catch (err) {
      setError(String(err));
      setSubmitting(false);
    }
  };

  return (
    <div className={overlayClass} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={panelClass} onKeyDown={(e) => e.key === 'Escape' && onClose()}>
        <div className="px-5 pt-3.5 pb-3 border-b border-hairline">
          <h3 className="text-sm font-semibold">{title}</h3>
        </div>
        <div className="px-5 py-4">
          <p className="text-[13px] leading-relaxed text-ink whitespace-pre-wrap">{message}</p>
          {error && <p className="mt-2 text-[11.5px] text-danger">{error}</p>}
        </div>
        <div className="flex justify-end gap-2 px-5 pt-3 pb-4 border-t border-hairline">
          <button
            onClick={onClose}
            className="h-[31px] px-4 text-[12.5px] rounded-[10px] text-muted hover:bg-app transition-colors duration-150"
          >
            取消
          </button>
          <button
            onClick={() => void submit()}
            disabled={submitting}
            className={`h-[31px] px-5 text-[12.5px] rounded-[10px] font-medium transition-colors duration-150 ${
              submitting
                ? 'bg-danger text-on-accent/70 cursor-not-allowed'
                : 'bg-danger text-on-accent hover:bg-danger-deep'
            }`}
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}
