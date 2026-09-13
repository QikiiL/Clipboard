/**
 * 连续粘贴(框选模式)的纯逻辑。
 *
 * 队列顺序 = 列表的显示顺序(items 数组顺序,时间倒序):
 * 界面顶部的条目最先粘贴,FIFO 出队,与用户视觉预期一致。
 */

/** 框选集合 → 粘贴队列:按当前列表显示顺序排序,并过滤不在列表中的 id */
export function orderQueueByDisplay(
  selected: Iterable<number>,
  items: { id: number }[]
): number[] {
  const pending = new Set(selected);
  const ordered: number[] = [];
  for (const item of items) {
    if (pending.has(item.id)) {
      ordered.push(item.id);
      pending.delete(item.id); // 同一 id 只入队一次
    }
  }
  return ordered;
}

/** 框选集合 → 每条的粘贴序号(从 1 起,按显示顺序),用于条目上的顺序徽标 */
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
