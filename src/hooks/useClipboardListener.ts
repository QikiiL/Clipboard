import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { queryItems, escapeLike } from '../lib/db';
import { useClipboardStore } from '../stores/clipboardStore';
import type { ClipboardItem } from '../types/clipboard';

export function useClipboardListener() {
  const { setItems } = useClipboardStore();
  const requestId = useRef(0);

  const reloadItems = async () => {
    const id = ++requestId.current;
    const { searchQuery, selectedGroup, showFavorites } = useClipboardStore.getState();
    let sql = 'SELECT * FROM items';
    const params: unknown[] = [];
    const conditions: string[] = [];

    if (searchQuery) {
      conditions.push('(content LIKE ? OR preview LIKE ?)');
      params.push(`%${escapeLike(searchQuery)}%`, `%${escapeLike(searchQuery)}%`);
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

    sql += ' ORDER BY last_used_at DESC LIMIT 500';
    const items = await queryItems(sql, params);
    if (id === requestId.current) {
      setItems(items as ClipboardItem[]);
    }
  };

  useEffect(() => {
    (async () => {
      try {
        await reloadItems();
      } catch (err) {
        console.error('Failed to load items:', err);
      }
    })();

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
