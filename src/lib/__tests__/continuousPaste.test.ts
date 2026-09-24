import { describe, expect, it } from 'vitest';
import { buildOrderMap, orderQueueBySelection } from '../continuousPaste';

describe('orderQueueBySelection', () => {
  it('按点选先后顺序排序:先点的先粘,与列表位置无关', () => {
    // 列表按时间倒序显示 3,2,1;用户按 1 → 3 → 2 的顺序点选
    expect(orderQueueBySelection([1, 3, 2], new Set([1, 3, 2]))).toEqual([1, 3, 2]);
  });

  it('重复点选(取消后重选)以最新位置为准', () => {
    // 点 1、点 2、取消 1、重点 1:队列变为 2 → 1
    const order = [1, 2, 1, 1].reduce(
      (acc, id) => {
        // 模拟 toggleSelect:已选则移出,未选则追加
        if (acc.includes(id)) return acc.filter((v) => v !== id);
        return [...acc, id];
      },
      [] as number[]
    );
    expect(order).toEqual([2, 1]);
    expect(orderQueueBySelection(order, new Set([1, 2]))).toEqual([2, 1]);
  });

  it('过滤已取消(不在选中集合)的 id,且同一 id 只入队一次', () => {
    expect(orderQueueBySelection([9, 1, 1, 2], new Set([1, 2]))).toEqual([1, 2]);
  });

  it('空选择序列返回空队列', () => {
    expect(orderQueueBySelection([], new Set([1]))).toEqual([]);
  });

  it('选中集合为空返回空队列', () => {
    expect(orderQueueBySelection([1, 2], new Set())).toEqual([]);
  });
});

describe('buildOrderMap', () => {
  it('序号从 1 开始且属于先点的条目', () => {
    const map = buildOrderMap([5, 7]);
    expect(map.get(5)).toBe(1);
    expect(map.get(7)).toBe(2);
  });

  it('空队列返回空映射', () => {
    expect(buildOrderMap([]).size).toBe(0);
  });
});
