import { useEffect, useRef, useState } from 'react';

export interface BandRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

interface Options {
  containerRef: React.RefObject<HTMLDivElement | null>;
  /** false 时 hook 完全不介入(不绑监听、不出框) */
  enabled: boolean;
  /** 框选结束:提交整个拖拽过程中累积命中的条目(并入选中集合) */
  onApply: (ids: number[]) => void;
  /** 拖拽过程中的实时预览(累积命中的条目集合) */
  onPreview: (ids: number[]) => void;
  /** 在空白处单击(未拖拽):清空选择 */
  onBackgroundClick: () => void;
}

/** 移动超过该距离才算拖拽框选,否则视为单击(避免手抖误触发框选) */
const DRAG_THRESHOLD = 4;
/** 拖拽全程的最大跨度小于该尺寸时不应用选择,视为误拖,保护已有选择 */
const MIN_BAND_SIZE = 8;

/**
 * 框选模式的橡皮筋选区(rubber-band selection)。
 *
 * 交互约定:
 * - 在条目上按下并原地松开 = 单击 → 条目自身的 onClick 负责切换选中;
 * - 按下后移动超过阈值 = 拖拽框选 → 与 `[data-item-id]` 行求矩形交集;
 * - **累积制**:一次拖拽中滚轮翻页/列表滚动时,所有被框划过的条目都计入
 *   本次选择(拖 Union),松手后并入现有选择 —— 长列表分几屏框选时
 *   先前框住的内容不会丢;多次拖拽同样并入选中集合;
 * - 在空白处单击 = 清空选择(累积制的"重来"入口);
 * - 在按钮/输入框上按下不进入框选,悬停操作按钮照常可用。
 *
 * didDragRef 供列表在 click 捕获阶段拦截「拖拽结束后的那次 click」,
 * 避免把拖拽误判成对条目的点击(见 ClipboardList)。
 */
export function useRubberBandSelect({
  containerRef,
  enabled,
  onApply,
  onPreview,
  onBackgroundClick,
}: Options) {
  const [band, setBand] = useState<BandRect | null>(null);
  const didDragRef = useRef(false);

  // items.length 参与依赖:列表从空变为非空时滚动容器才真正挂载,
  // 需要重新绑定监听(重绑是幂等的,不会重复响应同一次按下)
  useEffect(() => {
    if (!enabled) return;
    const container = containerRef.current;
    if (!container) return;

    let startX = 0;
    let startY = 0;
    let active = false;
    let startedOnItem = false;
    let pendingRect: BandRect | null = null;
    let maxSpan = 0;
    // 本次拖拽累积命中的条目:滚轮翻页时矩形固定、列表滑动,
    // 只按"最终矩形"求交集会丢掉先前划过的条目
    let dragUnion: Set<number> = new Set();
    let raf = 0;

    const hitTest = (left: number, top: number, right: number, bottom: number) => {
      const ids: number[] = [];
      container.querySelectorAll<HTMLElement>('[data-item-id]').forEach((el) => {
        const r = el.getBoundingClientRect();
        if (r.left < right && r.right > left && r.top < bottom && r.bottom > top) {
          const id = Number(el.dataset.itemId);
          if (Number.isFinite(id)) ids.push(id);
        }
      });
      return ids;
    };

    // rAF 合并 mousemove/wheel 的高频触发,一帧最多算一次命中
    const flush = () => {
      raf = 0;
      const rect = pendingRect;
      if (!rect) return;
      for (const id of hitTest(
        rect.left,
        rect.top,
        rect.left + rect.width,
        rect.top + rect.height
      )) {
        dragUnion.add(id);
      }
      // 预览直接显示累积结果:滚过去的条目保持高亮,与松手后的结果一致
      onPreview([...dragUnion]);
      setBand(rect);
    };
    const schedule = (rect: BandRect) => {
      pendingRect = rect;
      if (!raf) raf = requestAnimationFrame(flush);
    };

    const onMouseMove = (e: MouseEvent) => {
      const dx = e.clientX - startX;
      const dy = e.clientY - startY;
      if (!active && Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) {
        return;
      }
      active = true;
      didDragRef.current = true;
      document.body.style.userSelect = 'none';
      const rect: BandRect = {
        left: Math.min(startX, e.clientX),
        top: Math.min(startY, e.clientY),
        width: Math.abs(dx),
        height: Math.abs(dy),
      };
      maxSpan = Math.max(maxSpan, rect.width, rect.height);
      schedule(rect);
    };

    // 拖拽期间滚轮滚动列表:内容从固定矩形下滑过,必须重算命中并累积
    const onWheel = () => {
      if (active && pendingRect) schedule(pendingRect);
    };
    const onMouseUp = () => {
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
      window.removeEventListener('wheel', onWheel);
      if (raf) {
        cancelAnimationFrame(raf);
        raf = 0;
      }
      document.body.style.userSelect = '';
      setBand(null);
      if (active) {
        onPreview([]);
        if (maxSpan >= MIN_BAND_SIZE && dragUnion.size > 0) {
          onApply([...dragUnion]);
        }
        // 全程没划出有效跨度或没命中任何条目:视为误拖,保留已有选择
      } else if (!startedOnItem) {
        onBackgroundClick();
      }
    };

    const onMouseDown = (e: MouseEvent) => {
      // 先复位:上一次拖拽的 click 拦截标记不能泄漏到新手势
      didDragRef.current = false;
      if (e.button !== 0) return;
      const target = e.target as Element | null;
      if (target?.closest('button, input, textarea, a')) return;
      // 目标就是滚动容器本身 = 内边距或滚动条区域,不启动框选
      if (target === container) return;
      startX = e.clientX;
      startY = e.clientY;
      active = false;
      startedOnItem = !!target?.closest('[data-item-id]');
      dragUnion = new Set();
      pendingRect = null;
      maxSpan = 0;
      window.addEventListener('mousemove', onMouseMove);
      window.addEventListener('mouseup', onMouseUp);
      window.addEventListener('wheel', onWheel);
    };

    container.addEventListener('mousedown', onMouseDown);
    return () => {
      container.removeEventListener('mousedown', onMouseDown);
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
      window.removeEventListener('wheel', onWheel);
      if (raf) cancelAnimationFrame(raf);
      document.body.style.userSelect = '';
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [containerRef, enabled, onApply, onPreview, onBackgroundClick]);

  return { band, didDragRef };
}
