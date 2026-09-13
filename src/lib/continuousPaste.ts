/**
 * 连续粘贴(框选模式)的纯逻辑。
 *
 * 队列顺序 = 真·先进先出(FIFO),按内容被**复制的时间先后**:
 * 列表按时间倒序展示(顶部最新、底部最旧),所以从列表**底部往上**遍历 ——
 * 最早复制的内容最先粘贴,最新复制的最后粘贴。例如依次复制 A、B、C,
 * 列表显示 C、B、A,框选三者后的粘贴顺序是 A → B → C。
 */

/** 框选集合 → 粘贴队列:按复制时间先后(列表从底部到顶部)排序,FIFO 出队 */
export function orderQueueByDisplay(
  selected: Iterable<number>,
  items: { id: number }[]
): number[] {
  const pending = new Set(selected);
  const ordered: number[] = [];
  for (let i = items.length - 1; i >= 0; i--) {
    const item = items[i];
    if (pending.has(item.id)) {
      ordered.push(item.id);
      pending.delete(item.id); // 同一 id 只入队一次
    }
  }
  return ordered;
}

/** 框选集合 → 每条的粘贴序号(从 1 起,复制最早的为 1),用于条目上的顺序徽标 */
export function buildOrderMap(
  selected: Iterable<number>,
  items: { id: number }[]
): Map<number, number> {
  const map = new Map<number, number>();
  let order = 1;
  for (const id of orderQueueByDisplay(selected, items)) {
    map.set(id, order++);
  }
  return map;
}
