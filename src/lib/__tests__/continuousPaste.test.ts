import { describe, expect, it } from 'vitest';
import { buildOrderMap, orderQueueByDisplay } from '../continuousPaste';

describe('orderQueueByDisplay', () => {
  it('按复制时间先后排序:列表底部(最早复制)最先粘贴,真 FIFO', () => {
    // 依次复制 A(id1)、B(id2)、C(id3),列表按时间倒序显示 3,2,1
    const items = [{ id: 3 }, { id: 2 }, { id: 1 }];
    expect(orderQueueByDisplay([3, 1, 2], items)).toEqual([1, 2, 3]);
  });

  it('过滤不在当前列表中的 id,且同一 id 只入队一次', () => {
    const items = [{ id: 2 }, { id: 1 }];
    expect(orderQueueByDisplay([9, 1, 1, 2], items)).toEqual([1, 2]);
  });

  it('空选择返回空队列', () => {
    expect(orderQueueByDisplay([], [{ id: 1 }])).toEqual([]);
  });

  it('空列表返回空队列', () => {
    expect(orderQueueByDisplay([1, 2], [])).toEqual([]);
  });
});

describe('buildOrderMap', () => {
  it('序号从 1 开始且属于最早复制的内容(列表底部)', () => {
    // 列表显示 7(新), 5(旧):5 更早复制,序号 1
    const map = buildOrderMap([5, 7], [{ id: 7 }, { id: 5 }, { id: 9 }]);
    expect(map.get(5)).toBe(1);
    expect(map.get(7)).toBe(2);
    expect(map.has(9)).toBe(false);
  });

  it('空选择返回空映射', () => {
    expect(buildOrderMap([], [{ id: 1 }]).size).toBe(0);
  });
});
