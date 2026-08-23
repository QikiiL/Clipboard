// 转义 LIKE 通配符。SQLite 的 LIKE 默认没有转义符,必须搭配 ESCAPE '\'
// 子句使用,否则反斜杠会被当作字面字符、% 和 _ 仍作为通配符。
export function escapeLike(s: string): string {
  return s.replace(/[%_[\]\\]/g, (ch) => `\\${ch}`);
}

export const DEFAULT_ITEM_LIMIT = 500;

export function buildItemQuery(filters: {
  searchQuery?: string;
  groupId?: number | null;
  favoritesOnly?: boolean;
  limit?: number;
}): { sql: string; params: unknown[] } {
  let sql = 'SELECT * FROM items';
  const params: unknown[] = [];
  const conditions: string[] = [];

  if (filters.searchQuery) {
    conditions.push("(content LIKE ? ESCAPE '\\' OR preview LIKE ? ESCAPE '\\')");
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

  sql += ' ORDER BY last_used_at DESC LIMIT ?';
  params.push(filters.limit ?? DEFAULT_ITEM_LIMIT);
  return { sql, params };
}
