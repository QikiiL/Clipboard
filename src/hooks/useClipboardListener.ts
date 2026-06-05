import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { queryItems } from '../lib/db';
import { useClipboardStore } from '../stores/clipboardStore';
import type { ClipboardItem } from '../types/clipboard';

export function useClipboardListener() {
  const { setItems } = useClipboardStore();

  const reloadItems = async () => {
    const { searchQuery, selectedGroup, showFavorites } = useClipboardStore.getState();
    let sql = 'SELECT * FROM items';
    const params: unknown[] = [];
    const conditions: string[] = [];

    if (searchQuery) {
      conditions.push('(content LIKE ? OR preview LIKE ?)');
      params.push(`%${searchQuery}%`, `%${searchQuery}%`);
    }

    if (selectedGroup) {
      conditions.push('group_id = ?');
      params.push(selectedGroup.id);
    }

    if (showFavorites) {
      conditions.push('is_favorite = 1');
    }

    if (conditions.length > 0) {
      sql += ' WHERE ' + conditions.join(' AND ');
    }

    sql += ' ORDER BY last_used_at DESC';
    const items = await queryItems(sql, params);
    setItems(items as ClipboardItem[]);
  };

  useEffect(() => {
    reloadItems();

    const unlistenClipboard = listen('clipboard-changed', () => {
      reloadItems();
    });

    const unlistenDelete = listen('item-deleted', () => {
      reloadItems();
    });

    return () => {
      unlistenClipboard.then((fn) => fn());
      unlistenDelete.then((fn) => fn());
    };
  }, []);
}
