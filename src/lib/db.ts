import Database from '@tauri-apps/plugin-sql';
import { invoke } from '@tauri-apps/api/core';

let db: Database | null = null;

export async function getDb(): Promise<Database> {
  if (!db) {
    const dbPath = await invoke<string>('get_db_path');
    db = await Database.load(dbPath);
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

// 只读查询入口;写操作一律走 Rust 命令
export async function queryItems<T = unknown>(sql: string, params: unknown[] = []): Promise<T[]> {
  const db = await getDb();
  const results = await db.select(sql, params) as Record<string, unknown>[];
  return results.map(mapItem) as unknown as T[];
}
