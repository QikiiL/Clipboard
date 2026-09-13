// 前端通用格式化工具。原先分散在 ClipboardItem / StatusBar 各自实现,
// 语义相同却各写一份,容易改一处漏一处。

/**
 * 解析 SQLite `datetime('now')` 存储的 UTC 时间文本,形如
 * "YYYY-MM-DD HH:MM:SS"。`new Date()` 会把它当本地时间解析,
 * 必须显式补 'T' 和 'Z' 标记为 UTC;解析失败返回 null。
 */
export function dbTimeToDate(dateStr: string): Date | null {
  const normalized = dateStr.includes('T') ? dateStr : dateStr.replace(' ', 'T') + 'Z';
  const date = new Date(normalized);
  return Number.isNaN(date.getTime()) ? null : date;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
