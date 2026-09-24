/**
 * 连续粘贴(框选模式)的纯逻辑。
 *
 * 队列顺序 = 用户**点选的先后顺序**:先点的先粘,后点的后粘。
 * 框选(橡皮筋)没有逐条点击,按条目在列表里的显示顺序(从上到下)入队;
 * Shift + 点击区间按手势方向(锚点 → 点击处)入队。
 */

/** 选择序列 → 粘贴队列:按点选先后顺序,过滤已取消的 id,重复 id 只入队一次 */
export function orderQueueBySelection(
  selectionOrder: number[],
  selected: Set<number>
): number[] {
  const seen = new Set<number>();
  const ordered: number[] = [];
  for (const id of selectionOrder) {
    if (selected.has(id) && !seen.has(id)) {
      ordered.push(id);
      seen.add(id);
    }
  }
  return ordered;
}

/** 粘贴队列 → 每条的粘贴序号(从 1 起,先点的为 1),用于条目上的顺序徽标 */
export function buildOrderMap(queue: number[]): Map<number, number> {
  const map = new Map<number, number>();
  queue.forEach((id, index) => map.set(id, index + 1));
  return map;
}
