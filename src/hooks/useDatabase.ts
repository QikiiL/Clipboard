import { useEffect, useRef } from 'react';
import { queryItems, escapeLike } from '../lib/db';
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
      let sql = 'SELECT * FROM items';
      const params: unknown[] = [];
      const conditions: string[] = [];

      if (searchQuery) {
        conditions.push('(content LIKE ? OR preview LIKE ?)');
        params.push(`%${escapeLike(searchQuery)}%`, `%${escapeLike(searchQuery)}%`);
      }

      if (groupId !== undefined && groupId !== null) {
        conditions.push('group_id = ?');
        params.push(groupId);
      }

      if (favoritesOnly) {
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
