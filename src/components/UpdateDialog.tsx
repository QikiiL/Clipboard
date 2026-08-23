import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface UpdateInfo {
  current: string;
  latest: string;
  has_update: boolean;
  notes?: string | null;
  github?: string | null;
  lanzou?: string | null;
  lanzou_password?: string | null;
}

interface Props {
  info: UpdateInfo;
  onClose: () => void;
}

function openUrl(url: string) {
  invoke('open_external_url', { url }).catch((err) => console.error('Open URL failed:', err));
}

/** 把更新说明拆成分点:优先按换行,兼容旧清单用分号分隔的写法 */
export function parseUpdateNotes(notes?: string | null): string[] {
  if (!notes) return [];
  return notes
    .split(/\r?\n/)
    .flatMap((line) => line.split(/[;；]/))
    .map((s) => s.trim().replace(/^[。.]$/, '').trim())
    .filter(Boolean);
}

/** 发现新版本:展示更新说明,用户选择从 GitHub 或蓝奏云手动下载 */
export function UpdateDialog({ info, onClose }: Props) {
  const [pwdCopied, setPwdCopied] = useState(false);
  const downloadBtn =
    'flex-1 h-[31px] text-[12.5px] rounded-[10px] font-medium transition-colors duration-150';
  const noteLines = parseUpdateNotes(info.notes);

  const copyPassword = async () => {
    if (!info.lanzou_password) return;
    try {
      await invoke('write_clipboard_text', { text: info.lanzou_password });
      setPwdCopied(true);
      setTimeout(() => setPwdCopied(false), 2000);
    } catch (err) {
      console.error('Copy password failed:', err);
    }
  };

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/50">
      <div className="bg-surface rounded-[14px] shadow-dialog border border-hairline w-full max-w-xs mx-4 max-h-[85vh] flex flex-col overflow-hidden">
        <div className="px-5 pt-3.5 pb-3 border-b border-hairline shrink-0">
          <h3 className="text-sm font-semibold">
            发现新版本 <span className="text-accent">v{info.latest}</span>
          </h3>
        </div>
        <div className="px-5 py-4 overflow-y-auto overscroll-contain">
          <p className="text-[11px] text-faint mb-2">当前版本 v{info.current}</p>
          {noteLines.length > 0 ? (
            <ul className="space-y-1.5">
              {noteLines.map((line, i) => (
                <li key={i} className="flex gap-2 text-[13px] leading-relaxed text-ink">
                  <span className="text-accent shrink-0 select-none">•</span>
                  <span className="min-w-0 break-words">{line}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-[13px] leading-relaxed text-ink">优化与问题修复。</p>
          )}
          {info.lanzou && info.lanzou_password && (
            <div className="flex items-center gap-2 mt-3 px-3 py-2 rounded-[8px] bg-app border border-hairline">
              <span className="text-[11px] text-faint shrink-0">蓝奏云密码</span>
              <span className="text-[13px] font-mono font-medium tracking-widest text-ink select-all truncate">
                {info.lanzou_password}
              </span>
              <button
                onClick={copyPassword}
                className="ml-auto shrink-0 h-[22px] px-2.5 text-[11px] rounded-[6px] border border-hairline text-muted hover:bg-hairline transition-colors duration-150"
              >
                {pwdCopied ? '已复制' : '复制'}
              </button>
            </div>
          )}
          <p className="text-[11px] text-faint mt-3">
            点击下方按钮前往下载页,下载完成后直接运行安装包覆盖安装,数据自动保留。
            运行安装包时会请求管理员权限(UAC),若火绒等安全软件弹出提示,请选择允许。
          </p>
        </div>
        <div className="flex justify-end gap-2 px-5 pt-3 pb-4 border-t border-hairline shrink-0">
          <button
            onClick={onClose}
            className="h-[31px] px-4 text-[12.5px] rounded-[10px] text-muted hover:bg-app transition-colors duration-150"
          >
            以后再说
          </button>
          {info.lanzou && (
            <button
              onClick={() => {
                // 双保险一:点击下载时自动把密码写入剪贴板
                if (info.lanzou_password) void copyPassword();
                openUrl(info.lanzou!);
              }}
              className={`${downloadBtn} border border-hairline text-muted hover:bg-hairline`}
            >
              蓝奏云下载
            </button>
          )}
          {info.github && (
            <button
              onClick={() => openUrl(info.github!)}
              className={`${downloadBtn} bg-accent text-on-accent hover:bg-accent-deep`}
            >
              GitHub 下载
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
