import { useEffect, useRef } from 'react';
import { queryItems } from '../lib/db';
import { buildItemQuery } from '../lib/queryBuilder';
import { useClipboardStore } from '../stores/clipboardStore';
import type { ClipboardItem } from '../types/clipboard';
import type { ClipboardGroup } from '../types/group';

export function useDatabase() {
  const { setItems, setGroups, setLoading } = useClipboardStore();
  const requestId = useRef(0);

  const loadItems = async (searchQuery?: string, groupId?: number | null, favoritesOnly?: boolean) => {
    const id = ++requestId.current;
    setLoading(true);
    try {
      const { sql, params } = buildItemQuery({ searchQuery, groupId, favoritesOnly });
      const items = await queryItems(sql, params);
      if (id === requestId.current) {
        setItems(items as ClipboardItem[]);
      }
    } finally {
      if (id === requestId.current) {
        setLoading(false);
      }
    }
  };

  const loadGroups = async () => {
    const groups = await queryItems('SELECT * FROM groups ORDER BY sort_order');
    setGroups(groups as ClipboardGroup[]);
  };

  useEffect(() => {
    (async () => {
      try {
        await loadGroups();
      } catch (err) {
        console.error('Failed to load groups:', err);
      }
    })();
  }, []);

  return { loadItems, loadGroups };
}
