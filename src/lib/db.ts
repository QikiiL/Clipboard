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

export async function queryItems(sql: string, params: unknown[] = []) {
  const db = await getDb();
  const results = await db.select(sql, params);
  return results.map(mapItem);
}

export async function executeSql(sql: string, params: unknown[] = []) {
  const db = await getDb();
  return db.execute(sql, params);
}
