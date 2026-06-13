import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { queryItems } from '../lib/db';
import { buildItemQuery } from '../lib/queryBuilder';
import { useClipboardStore } from '../stores/clipboardStore';
import type { ClipboardItem } from '../types/clipboard';

export function useClipboardListener() {
  const { setItems } = useClipboardStore();
  const requestId = useRef(0);

  const reloadItems = async () => {
    const id = ++requestId.current;
    const { searchQuery, selectedGroup, showFavorites } = useClipboardStore.getState();
    const { sql, params } = buildItemQuery({
      searchQuery: searchQuery || undefined,
      groupId: selectedGroup?.id,
      favoritesOnly: showFavorites,
    });
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

    const unlistenPause = listen<boolean>('monitoring-paused', (event) => {
      useClipboardStore.getState().setPaused(event.payload);
    });

    return () => {
      unlistenClipboard.then((fn) => fn());
      unlistenDelete.then((fn) => fn());
      unlistenPause.then((fn) => fn());
    };
  }, []);
}
