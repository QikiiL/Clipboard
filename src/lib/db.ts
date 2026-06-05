import Database from '@tauri-apps/plugin-sql';

let db: Database | null = null;

export async function getDb(): Promise<Database> {
  if (!db) {
    db = await Database.load('sqlite:clipboard.db');
  }
  return db;
}

// Map SQLite integer booleans to JS booleans
function mapItem(item: Record<string, unknown>): Record<string, unknown> {
  return {
    ...item,
    is_favorite: Boolean(item.is_favorite),
  };
}

export function escapeLike(s: string): string {
  return s.replace(/[%_[\]]/g, (ch) => `\\${ch}`);
}

export async function queryItems<T = unknown>(sql: string, params: unknown[] = []): Promise<T[]> {
  const db = await getDb();
  const results = await db.select(sql, params) as Record<string, unknown>[];
  return results.map(mapItem) as unknown as T[];
}

export async function executeSql(sql: string, params: unknown[] = []) {
  const db = await getDb();
  return db.execute(sql, params);
}
