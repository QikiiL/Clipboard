import { memo } from 'react';
import type { ClipboardItem as ClipboardItemType, ClipboardType } from '../types/clipboard';
import { ClipboardType as CT } from '../types/clipboard';

interface Props {
  item: ClipboardItemType;
  onPaste: (item: ClipboardItemType) => void;
  onDelete: (id: number) => void;
  onToggleFavorite: (id: number) => void;
}

const typeIcons: Record<ClipboardType, string> = {
  [CT.Text]: '📝',
  [CT.Link]: '🔗',
  [CT.Image]: '🖼️',
  [CT.File]: '📁',
};

function formatTime(dateStr: string): string {
  const date = new Date(dateStr);
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
  onPaste,
  onDelete,
  onToggleFavorite,
}: Props) {
  return (
    <div className="group flex items-start gap-3 p-3 rounded-lg border border-transparent hover:border-blue-200 dark:hover:border-blue-800 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-all cursor-pointer"
         onClick={() => onPaste(item)}>
      <div className="text-2xl flex-shrink-0 mt-0.5">
        {typeIcons[item.type]}
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-sm text-gray-900 dark:text-gray-100 truncate">
          {item.preview || item.content}
        </p>
        <div className="flex items-center gap-2 mt-1">
          <span className="text-xs text-gray-500">{formatTime(item.last_used_at)}</span>
          {item.copy_count > 1 && (
            <span className="text-xs bg-gray-200 dark:bg-gray-700 px-1.5 py-0.5 rounded-full">
              ×{item.copy_count}
            </span>
          )}
        </div>
      </div>
      <div className="flex-shrink-0 flex items-center gap-1 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
        <button
          onClick={(e) => { e.stopPropagation(); onToggleFavorite(item.id); }}
          className="p-1 hover:bg-gray-200 dark:hover:bg-gray-700 rounded"
          title={item.is_favorite ? '取消收藏' : '收藏'}
        >
          {item.is_favorite ? '⭐' : '☆'}
        </button>
        <button
          onClick={(e) => { e.stopPropagation(); onDelete(item.id); }}
          className="p-1 hover:bg-red-100 dark:hover:bg-red-900/30 rounded text-red-500"
          title="删除"
        >
          🗑️
        </button>
      </div>
    </div>
  );
});
