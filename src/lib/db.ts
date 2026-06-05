import Database from '@tauri-apps/plugin-sql';

let db: Database | null = null;

export async function getDb(): Promise<Database> {
  if (!db) {
    db = await Database.load('sqlite:clipboard.db');
  }
  return db;
}

export async function queryItems(sql: string, params: unknown[] = []) {
  const db = await getDb();
  return db.select(sql, params);
}

export async function executeSql(sql: string, params: unknown[] = []) {
  const db = await getDb();
  return db.execute(sql, params);
}
