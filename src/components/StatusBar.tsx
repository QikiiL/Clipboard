import { useClipboardStore } from '../stores/clipboardStore';

export function StatusBar() {
  const items = useClipboardStore((s) => s.items);
  const totalCount = items.length;
  const favoriteCount = items.filter((i) => i.is_favorite).length;

  return (
    <div className="flex items-center justify-between px-4 py-2 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50 text-xs text-gray-500">
      <div className="flex items-center gap-4">
        <span>共 {totalCount} 条记录</span>
        <span>⭐ {favoriteCount} 收藏</span>
      </div>
      <div className="flex items-center gap-2">
        <span className="w-2 h-2 rounded-full bg-green-500" title="监听中" />
        <span>监听中</span>
      </div>
    </div>
  );
}
