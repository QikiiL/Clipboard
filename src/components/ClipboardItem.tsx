import { memo, useState, useEffect, useCallback } from 'react';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { useClipboardStore } from '../stores/clipboardStore';
import type { ClipboardItem as ClipboardItemType, ClipboardType } from '../types/clipboard';
import { ClipboardType as CT } from '../types/clipboard';
import {
  TextIcon,
  LinkIcon,
  ImageIcon,
  FileIcon,
  StarIcon,
  StarOutlineIcon,
  FolderIcon,
  FolderFillIcon,
  TrashIcon,
} from './icons';

// 图片缩略图经 asset 协议按需加载(不经 IPC/base64);
// 文件大小按需取一次后缓存为数字,几乎不占内存
const fileSizeCache = new Map<string, number>();

async function fetchAssetSize(url: string): Promise<number | null> {
  try {
    const resp = await fetch(url);
    if (!resp.ok) return null;
    return (await resp.blob()).size;
  } catch {
    return null;
  }
}

interface Props {
  item: ClipboardItemType;
  onActivate: (item: ClipboardItemType) => void;
  onDelete: (id: number) => void;
  onToggleFavorite: (id: number) => void;
}

const typeIcons: Record<ClipboardType, typeof TextIcon> = {
  [CT.Text]: TextIcon,
  [CT.Link]: LinkIcon,
  [CT.Image]: ImageIcon,
  [CT.File]: FileIcon,
};

// 文件类内容是多行路径:主行显示首个文件名,次行显示完整路径(多个时注明数量)
function getFileMeta(content: string): { name: string; meta: string } {
  const lines = content.split('\n').filter(Boolean);
  const first = lines[0] ?? '';
  const name = first.split('\\').pop() || first;
  const meta = lines.length > 1 ? `${lines.length} 个文件 · ${first}` : first;
  return { name, meta };
}

function formatImageTitle(dateStr: string): string {
  const normalized = dateStr.includes('T') ? dateStr : dateStr.replace(' ', 'T') + 'Z';
  const d = new Date(normalized);
  if (isNaN(d.getTime())) return '图片';
  const pad = (n: number) => String(n).padStart(2, '0');
  return `图片 ${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatTime(dateStr: string): string {
  // SQLite datetime('now') 存的是 UTC,形如 "YYYY-MM-DD HH:MM:SS"。
  // new Date() 会把它当本地时间解析,必须显式补 'T' 和 'Z' 标记为 UTC。
  const normalized = dateStr.includes('T')
    ? dateStr
    : dateStr.replace(' ', 'T') + 'Z';
  const date = new Date(normalized);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return '刚刚';
  if (diffMins < 60) return `${diffMins}分钟前`;
  if (diffHours < 24) return `${diffHours}小时前`;
  return `${diffDays}天前`;
}

export const ClipboardItemCard = memo(function ClipboardItemCard({
  item,
  onActivate,
  onDelete,
  onToggleFavorite,
}: Props) {
  const isImage = item.type === CT.Image;
  const imageSrc = isImage && item.file_path ? convertFileSrc(item.file_path) : null;
  const [imageFailed, setImageFailed] = useState(false);
  const [imgSize, setImgSize] = useState<{ w: number; h: number } | null>(null);
  const [fileSize, setFileSize] = useState<number | null>(() =>
    item.file_path ? fileSizeCache.get(item.file_path) ?? null : null
  );
  const [mousePos, setMousePos] = useState<{ x: number; y: number } | null>(null);
  const [groupMenuOpen, setGroupMenuOpen] = useState(false);
  const groups = useClipboardStore((s) => s.groups);

  const handleMouseEnter = useCallback((e: React.MouseEvent) => {
    setMousePos({ x: e.clientX, y: e.clientY });
  }, []);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    setMousePos({ x: e.clientX, y: e.clientY });
  }, []);

  const handleMouseLeave = useCallback(() => {
    setMousePos(null);
  }, []);

  const handleSetGroup = useCallback(async (itemId: number, groupId: number | null) => {
    setGroupMenuOpen(false);
    try {
      await invoke('set_item_group', { itemId, groupId });
      // 列表刷新由 clipboard-changed 事件触发
    } catch (err) {
      console.error('Set group failed:', err);
    }
  }, []);

  // Fix #5: Reset imageFailed when item.file_path changes
  useEffect(() => {
    setImageFailed(false);
    setImgSize(null);
  }, [item.file_path]);

  // 文件大小按需取一次(数字级缓存,几乎不占内存)
  useEffect(() => {
    if (!imageSrc || !item.file_path) return;
    const filePath = item.file_path; // capture locally to avoid ! assertion
    if (fileSize !== null || fileSizeCache.has(filePath)) {
      if (fileSize === null) setFileSize(fileSizeCache.get(filePath) ?? null);
      return;
    }
    let cancelled = false;
    fetchAssetSize(imageSrc).then((size) => {
      if (cancelled) return;
      if (size !== null) fileSizeCache.set(filePath, size);
      setFileSize(size);
    });
    return () => { cancelled = true; };
  }, [imageSrc, item.file_path, fileSize]);

  const renderImagePreview = () => {
    const previewX = mousePos ? Math.min(mousePos.x + 16, window.innerWidth - 370) : 0;
    const previewY = mousePos ? Math.max(mousePos.y - 150, 10) : 0;

    if (imageSrc && !imageFailed) {
      const metaParts = [
        'PNG',
        imgSize ? `${imgSize.w} × ${imgSize.h}` : null,
        fileSize !== null ? formatBytes(fileSize) : null,
      ].filter(Boolean);
      return (
        <div className="relative flex items-center gap-2.5 min-w-0 flex-1">
          <img
            src={imageSrc}
            alt="[图片]"
            className="h-11 w-11 rounded-lg border border-hairline object-cover flex-shrink-0"
            onError={() => setImageFailed(true)}
            onLoad={(e) => {
              if (!imgSize) {
                setImgSize({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight });
              }
            }}
            onMouseEnter={handleMouseEnter}
            onMouseMove={handleMouseMove}
            onMouseLeave={handleMouseLeave}
          />
          <div className="min-w-0 flex-1">
            <p className="text-[13px] leading-snug text-ink truncate">{formatImageTitle(item.created_at)}</p>
            <p className="mt-0.5 text-[11px] text-faint tabular-nums truncate">{metaParts.join(' · ')}</p>
          </div>
          {mousePos && (
            <div
              className="fixed z-[9999] pointer-events-none"
              style={{
                left: previewX,
                top: previewY,
              }}
            >
              <div className="bg-surface rounded-lg shadow-dialog border border-hairline p-2">
                <img
                  src={imageSrc}
                  alt="[图片预览]"
                  className="max-w-[350px] max-h-[300px] object-contain rounded"
                />
              </div>
            </div>
          )}
        </div>
      );
    }
    // Fallback: icon + text
    return (
      <p className="text-[13px] text-ink truncate">[图片]</p>
    );
  };

  return (
    <div
      className="group relative flex items-center gap-2.5 min-h-[52px] p-[10px] rounded-[10px] cursor-pointer transition-[background-color,box-shadow] duration-150 hover:bg-surface hover:shadow-lift"
      onClick={() => onActivate(item)}
    >
      {!(isImage && imageSrc && !imageFailed) && (
        <div className="flex-shrink-0 mt-px text-faint">
          {(() => {
            const TypeIcon = typeIcons[item.type];
            return <TypeIcon size={15} />;
          })()}
        </div>
      )}
      <div className="flex-1 min-w-0">
        {isImage ? (
          renderImagePreview()
        ) : item.type === CT.File ? (
          (() => {
            const { name, meta } = getFileMeta(item.content);
            return (
              <>
                <p className="text-[13px] leading-snug text-ink truncate">{name}</p>
                <p className="mt-0.5 text-[11px] text-faint tabular-nums truncate">{meta}</p>
              </>
            );
          })()
        ) : (
          <p className={`text-[13px] leading-snug truncate ${item.type === CT.Link ? 'text-accent' : 'text-ink'}`}>
            {item.preview || item.content}
          </p>
        )}
      </div>
      <div className="flex-shrink-0 flex items-center">
        {item.is_favorite && (
          <span className="text-star mr-2"><StarIcon size={13} /></span>
        )}
        {item.group_id !== null && (
          <span className="text-accent mr-2"><FolderFillIcon size={13} /></span>
        )}
        <span className="text-[11px] text-faint tabular-nums">{formatTime(item.last_used_at)}</span>
        <div className="flex items-center pl-2 w-0 overflow-hidden group-hover:w-[90px] group-focus-within:w-[90px] transition-[width] duration-150 ease-out">
          <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity duration-150">
            <button
              onMouseDown={(e) => e.preventDefault()}
              onClick={(e) => { e.stopPropagation(); onToggleFavorite(item.id); }}
              className="flex items-center justify-center w-[26px] h-[26px] rounded-[7px] text-faint hover:bg-hairline hover:text-muted transition-colors"
              title={item.is_favorite ? '取消收藏' : '收藏'}
            >
              {item.is_favorite ? <StarIcon size={14} /> : <StarOutlineIcon size={14} />}
            </button>
            <button
              onMouseDown={(e) => e.preventDefault()}
              onClick={(e) => { e.stopPropagation(); setGroupMenuOpen((v) => !v); }}
              className="flex items-center justify-center w-[26px] h-[26px] rounded-[7px] text-faint hover:bg-hairline hover:text-muted transition-colors"
              title={item.group_id !== null ? '更改分组' : '归入分组'}
            >
              {item.group_id !== null ? <FolderFillIcon size={14} /> : <FolderIcon size={14} />}
            </button>
            <button
              onMouseDown={(e) => e.preventDefault()}
              onClick={(e) => { e.stopPropagation(); onDelete(item.id); }}
              className="flex items-center justify-center w-[26px] h-[26px] rounded-[7px] text-faint hover:bg-hairline hover:text-danger transition-colors"
              title="删除"
            >
              <TrashIcon size={14} />
            </button>
          </div>
        </div>
      </div>
      {groupMenuOpen && (
        <>
          <div className="fixed inset-0 z-40" onClick={(e) => { e.stopPropagation(); setGroupMenuOpen(false); }} />
          <div
            className="absolute right-[10px] top-full mt-1 z-50 min-w-[140px] bg-surface rounded-lg shadow-dialog border border-hairline py-1"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              onClick={() => handleSetGroup(item.id, null)}
              className={`w-full text-left px-3 py-1.5 text-[13px] hover:bg-app ${
                item.group_id === null ? 'text-accent font-medium' : 'text-ink'
              }`}
            >
              {item.group_id === null ? '未分组' : '取消分组'}
            </button>
            {groups.map((g) => (
              <button
                key={g.id}
                onClick={() => handleSetGroup(item.id, g.id)}
                className={`w-full text-left px-3 py-1.5 text-[13px] hover:bg-app truncate ${
                  item.group_id === g.id ? 'text-accent font-medium' : 'text-ink'
                }`}
              >
                {g.icon} {g.name}
              </button>
            ))}
            {groups.length === 0 && (
              <p className="px-3 py-1.5 text-[11px] text-faint">暂无分组,可在分组栏新建</p>
            )}
          </div>
        </>
      )}
    </div>
  );
});
