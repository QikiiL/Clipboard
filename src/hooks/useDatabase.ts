import { useCallback, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { queryItems } from '../lib/db';
import { buildItemQuery } from '../lib/queryBuilder';
import { useClipboardStore } from '../stores/clipboardStore';
import type { ClipboardItem } from '../types/clipboard';
import type { ClipboardGroup } from '../types/group';
import type { AppSettings } from '../types/settings';

export function useDatabase() {
  const { setItems, setGroups, setLoading, incrementRequestId } = useClipboardStore();

  const loadItems = useCallback(async (searchQuery?: string, groupId?: number | null, favoritesOnly?: boolean) => {
    const id = incrementRequestId();
    setLoading(true);
    try {
      const { sql, params } = buildItemQuery({
        searchQuery,
        groupId,
        favoritesOnly,
        limit: useClipboardStore.getState().maxItems,
      });
      const items = await queryItems(sql, params);
      if (id === useClipboardStore.getState().requestId) {
        setItems(items as ClipboardItem[]);
      }
    } finally {
      if (id === useClipboardStore.getState().requestId) {
        setLoading(false);
      }
    }
  }, [setItems, setLoading, incrementRequestId]);

  const loadGroups = useCallback(async () => {
    const groups = await queryItems('SELECT * FROM groups ORDER BY sort_order');
    setGroups(groups as ClipboardGroup[]);
  }, [setGroups]);

  useEffect(() => {
    (async () => {
      try {
        await loadGroups();
      } catch (err) {
        console.error('Failed to load groups:', err);
      }
    })();

    // 启动时同步设置到前端状态:列表上限(0=不限制)与暂停状态
    (async () => {
      try {
        const settings = await invoke<AppSettings>('load_settings');
        const store = useClipboardStore.getState();
        store.setMaxItems(settings.max_item_count > 0 ? settings.max_item_count : 100000);
        store.setPaused(settings.paused);
      } catch (err) {
        console.error('Failed to load settings:', err);
      }
    })();

    const unlistenGroups = listen('groups-changed', () => {
      loadGroups().catch(console.error);
      // 选中的分组可能已被删除
      const { selectedGroup, setSelectedGroup } = useClipboardStore.getState();
      if (selectedGroup) {
        queryItems('SELECT id FROM groups WHERE id = ?', [selectedGroup.id])
          .then((rows) => {
            if (rows.length === 0) setSelectedGroup(null);
          })
          .catch(console.error);
      }
    });

    return () => {
      unlistenGroups.then((fn) => fn());
    };
  }, [loadGroups]);

  return { loadItems, loadGroups };
}
