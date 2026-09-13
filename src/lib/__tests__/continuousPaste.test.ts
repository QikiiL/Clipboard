import { describe, expect, it } from 'vitest';
import { buildOrderMap, orderQueueByDisplay } from '../continuousPaste';

describe('orderQueueByDisplay', () => {
  it('按列表显示顺序(FIFO)排序,而非选择的先后', () => {
    const items = [{ id: 3 }, { id: 1 }, { id: 2 }];
    expect(orderQueueByDisplay([2, 3, 1], items)).toEqual([3, 1, 2]);
  });

  it('过滤不在当前列表中的 id,且同一 id 只入队一次', () => {
    const items = [{ id: 1 }, { id: 2 }];
    expect(orderQueueByDisplay([9, 2, 2, 1], items)).toEqual([1, 2]);
  });

  it('空选择返回空队列', () => {
    expect(orderQueueByDisplay([], [{ id: 1 }])).toEqual([]);
  });

  it('空列表返回空队列', () => {
    expect(orderQueueByDisplay([1, 2], [])).toEqual([]);
  });
});

describe('buildOrderMap', () => {
  it('序号从 1 开始且跟随显示顺序', () => {
    const map = buildOrderMap([5, 7], [{ id: 7 }, { id: 5 }, { id: 9 }]);
    expect(map.get(7)).toBe(1);
    expect(map.get(5)).toBe(2);
    expect(map.has(9)).toBe(false);
  });

  it('空选择返回空映射', () => {
    expect(buildOrderMap([], [{ id: 1 }]).size).toBe(0);
  });
});
