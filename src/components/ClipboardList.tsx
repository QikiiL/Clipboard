import { useClipboardStore } from '../stores/clipboardStore';
import { ClipboardItemCard } from './ClipboardItem';
import { invoke } from '@tauri-apps/api/core';
import type { ClipboardItem as ClipboardItemType } from '../types/clipboard';

export function ClipboardList() {
  const { items, isLoading } = useClipboardStore();

  const handlePaste = async (item: ClipboardItemType) => {
    try {
      await invoke('paste_item', { id: item.id });
    } catch (err) {
      console.error('Paste failed:', err);
    }
  };

  const handleDelete = async (id: number) => {
    try {
      await invoke('delete_item', { id });
    } catch (err) {
      console.error('Delete failed:', err);
    }
  };

  const handleToggleFavorite = async (id: number) => {
    try {
      await invoke('toggle_favorite', { id });
    } catch (err) {
      console.error('Toggle favorite failed:', err);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-gray-500">加载中...</div>
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-gray-400">
        <div className="text-4xl mb-3">📋</div>
        <p>暂无剪贴板记录</p>
        <p className="text-sm mt-1">复制内容后将自动出现在这里</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1 p-2 overflow-y-auto flex-1">
      {items.map((item) => (
        <ClipboardItemCard
          key={item.id}
          item={item}
          onPaste={handlePaste}
          onDelete={handleDelete}
          onToggleFavorite={handleToggleFavorite}
        />
      ))}
    </div>
  );
}
