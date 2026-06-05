import { escapeLike } from './db';

export function buildItemQuery(filters: {
  searchQuery?: string;
  groupId?: number | null;
  favoritesOnly?: boolean;
}): { sql: string; params: unknown[] } {
  let sql = 'SELECT * FROM items';
  const params: unknown[] = [];
  const conditions: string[] = [];

  if (filters.searchQuery) {
    conditions.push('(content LIKE ? OR preview LIKE ?)');
    params.push(`%${escapeLike(filters.searchQuery)}%`, `%${escapeLike(filters.searchQuery)}%`);
  }

  if (filters.groupId !== undefined && filters.groupId !== null) {
    conditions.push('group_id = ?');
    params.push(filters.groupId);
  }

  if (filters.favoritesOnly) {
    conditions.push('is_favorite = 1');
  }

  if (conditions.length > 0) {
    sql += ' WHERE ' + conditions.join(' AND ');
  }

  sql += ' ORDER BY last_used_at DESC LIMIT 500';
  return { sql, params };
}
