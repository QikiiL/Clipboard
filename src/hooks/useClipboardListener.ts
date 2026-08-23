import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useClipboardStore } from '../stores/clipboardStore';

export function useClipboardListener(loadItems: (searchQuery?: string, groupId?: number | null, favoritesOnly?: boolean) => Promise<void>) {
  useEffect(() => {
    const reloadItems = () => {
      const { searchQuery, selectedGroup, showFavorites } = useClipboardStore.getState();
      return loadItems(
        searchQuery || undefined,
        selectedGroup?.id ?? null,
        showFavorites,
      );
    };

    (async () => {
      try {
        await reloadItems();
      } catch (err) {
        console.error('Failed to load items:', err);
      }
    })();

    const unlistenClipboard = listen('clipboard-changed', () => {
      reloadItems().catch(console.error);
    });

    const unlistenDelete = listen('item-deleted', () => {
      reloadItems().catch(console.error);
    });

    const unlistenPause = listen<boolean>('monitoring-paused', (event) => {
      useClipboardStore.getState().setPaused(event.payload);
    });

    // Subscribe to store changes (searchQuery, selectedGroup, showFavorites, maxItems)
    // maxItems 纳入比较:设置里改上限、启动时加载设置后都会触发按新上限重查
    const unsubscribeStore = useClipboardStore.subscribe((state, prevState) => {
      if (
        state.searchQuery !== prevState.searchQuery ||
        state.selectedGroup !== prevState.selectedGroup ||
        state.showFavorites !== prevState.showFavorites ||
        state.maxItems !== prevState.maxItems
      ) {
        reloadItems().catch(console.error);
      }
    });

    return () => {
      unlistenClipboard.then((fn) => fn());
      unlistenDelete.then((fn) => fn());
      unlistenPause.then((fn) => fn());
      unsubscribeStore();
    };
  }, [loadItems]);
}
