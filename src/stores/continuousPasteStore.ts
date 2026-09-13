import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { useClipboardStore } from './clipboardStore';
import { buildOrderMap, orderQueueByDisplay } from '../lib/continuousPaste';

/** 与 Rust 侧 continuous_paste::QueueStatus 对应 */
export interface ContinuousStatus {
  active: boolean;
  total: number;
  remaining: number;
  next_id: number | null;
  pasted: number[];
  auto_running: boolean;
  hotkey_registered: boolean;
}

const EMPTY_SET = new Set<number>();

interface ContinuousPasteStore {
  /** 框选模式开关(纯 UI 状态) */
  mode: boolean;
  /** 框选选中的条目(无序集合;粘贴顺序由列表显示顺序决定) */
  selectedIds: Set<number>;
  /** 拖拽框选过程中的实时预览集合 */
  bandPreview: Set<number>;
  /** 每条的粘贴序号(1 起,按显示顺序),用于顺序徽标 */
  orderById: Map<number, number>;
  /** 后端队列状态(唯一事实来源,跨窗口销毁存活) */
  status: ContinuousStatus | null;
  /** 已粘贴条目集合(从 status.pasted 派生,便于 O(1) 判断) */
  pastedIds: Set<number>;
  lastError: string | null;
  /** shift 点击范围选择的锚点 */
  lastClickedId: number | null;

  setMode: (on: boolean) => void;
  toggleMode: () => void;
  toggleSelect: (id: number) => void;
  /** shift+点击:从锚点到该条目的显示区间全选 */
  selectRangeTo: (id: number) => void;
  /** 橡皮筋框选结束:并入选中集合(累积制;清空用 clearSelection) */
  applyBand: (ids: number[]) => void;
  setBandPreview: (ids: number[]) => void;
  clearSelection: () => void;
  recomputeOrder: () => void;
  applyStatus: (s: ContinuousStatus) => void;
  setLastError: (msg: string | null) => void;
  /** 窗口(重)建后从后端同步队列状态 */
  refreshStatus: () => Promise<void>;
  start: () => Promise<void>;
  startAuto: (intervalMs?: number) => Promise<void>;
  pasteNext: () => Promise<void>;
  stopAuto: () => Promise<void>;
  cancel: () => Promise<void>;
}

export const useContinuousPasteStore = create<ContinuousPasteStore>((set, get) => ({
  mode: false,
  selectedIds: EMPTY_SET,
  bandPreview: EMPTY_SET,
  orderById: new Map(),
  status: null,
  pastedIds: EMPTY_SET,
  lastError: null,
  lastClickedId: null,

    setMode: (on) => {
      if (on === get().mode) return;
      if (!on) {
        // 退出模式:清空框选状态;后端队列仍在跑则一并取消
        const status = get().status;
        set({
          mode: false,
          selectedIds: EMPTY_SET,
          bandPreview: EMPTY_SET,
          orderById: new Map(),
          lastError: null,
          lastClickedId: null,
        });
        if (status?.active) {
          invoke<ContinuousStatus>('cancel_continuous_paste')
            .then((s) => get().applyStatus(s))
            .catch(console.error);
        }
        return;
      }
      set({ mode: true, lastError: null });
      // 上一轮已结束(完成/取消)的残留状态清掉,回到框选阶段;
      // 否则会一直渲染"连续粘贴结束"横幅,把框选界面顶掉。
      // 后端队列仍在进行(active)时保留执行视图
      const status = get().status;
      if (status && !status.active) {
        set({ status: null, pastedIds: EMPTY_SET });
      }
    },

  toggleMode: () => get().setMode(!get().mode),

  recomputeOrder: () => {
    const { mode, selectedIds } = get();
    if (!mode) return;
    const items = useClipboardStore.getState().items;
    set({ orderById: buildOrderMap(selectedIds, items) });
  },

  toggleSelect: (id) => {
    const selectedIds = new Set(get().selectedIds);
    if (selectedIds.has(id)) {
      selectedIds.delete(id);
    } else {
      selectedIds.add(id);
    }
    set({ selectedIds, lastClickedId: id });
    get().recomputeOrder();
  },

  selectRangeTo: (id) => {
    const { lastClickedId, selectedIds: prev } = get();
    const items = useClipboardStore.getState().items;
    const endIndex = items.findIndex((i) => i.id === id);
    if (endIndex < 0) return;
    const selectedIds = new Set(prev);
    const startIndex =
      lastClickedId === null ? -1 : items.findIndex((i) => i.id === lastClickedId);
    if (startIndex < 0) {
      selectedIds.add(id);
    } else {
      const [lo, hi] =
        startIndex <= endIndex ? [startIndex, endIndex] : [endIndex, startIndex];
      for (let i = lo; i <= hi; i++) selectedIds.add(items[i].id);
    }
    set({ selectedIds });
    get().recomputeOrder();
  },

  applyBand: (ids) => {
    // 累积制:并入现有选择 —— 配合滚轮翻页/多次拖拽框长列表时,
    // 先前框住的条目不会被新框替换;清空走 clearSelection(按钮/空白单击)
    const selectedIds = new Set(get().selectedIds);
    for (const id of ids) selectedIds.add(id);
    set({ selectedIds, bandPreview: EMPTY_SET });
    get().recomputeOrder();
  },

  setBandPreview: (ids) => {
    set({ bandPreview: ids.length ? new Set(ids) : EMPTY_SET });
  },

  clearSelection: () =>
    set({
      selectedIds: EMPTY_SET,
      bandPreview: EMPTY_SET,
      orderById: new Map(),
      lastClickedId: null,
    }),

  applyStatus: (s) => set({ status: s, pastedIds: new Set(s.pasted) }),

  setLastError: (lastError) => set({ lastError }),

  refreshStatus: async () => {
    try {
      const s = await invoke<ContinuousStatus>('get_continuous_paste_status');
      get().applyStatus(s);
      // 后端队列仍在进行(窗口销毁期间按过热键):重建的窗口直接回到执行视图
      if (s.active) set({ mode: true });
    } catch {
      // 后端不可用(理论上不会发生):保持默认空状态
    }
  },

  start: async () => {
    const items = useClipboardStore.getState().items;
    const ids = orderQueueByDisplay(get().selectedIds, items);
    if (ids.length === 0) return;
    try {
      const s = await invoke<ContinuousStatus>('start_continuous_paste', { ids });
      // 开始只武装不粘贴,并隐藏窗口(焦点交还目标应用);
      // 窗口销毁会吞掉本响应,能走到这里说明已武装完成
      get().clearSelection();
      get().applyStatus(s);
    } catch (err) {
      get().setLastError(String(err));
    }
  },

  startAuto: async (intervalMs = 800) => {
    const items = useClipboardStore.getState().items;
    const ids = orderQueueByDisplay(get().selectedIds, items);
    if (ids.length === 0) return;
    try {
      await invoke('start_continuous_paste_auto', { ids, intervalMs });
      get().clearSelection();
      // 窗口很快会被首条投递销毁;乐观置一个执行中状态,重建后以后端为准
      set({
        status: {
          active: true,
          total: ids.length,
          remaining: ids.length,
          next_id: ids[0],
          pasted: [],
          auto_running: true,
          hotkey_registered: false,
        },
      });
    } catch (err) {
      get().setLastError(String(err));
    }
  },

  pasteNext: async () => {
    try {
      const s = await invoke<ContinuousStatus>('continuous_paste_next');
      get().applyStatus(s);
    } catch (err) {
      get().setLastError(String(err));
    }
  },

  stopAuto: async () => {
    try {
      const s = await invoke<ContinuousStatus>('stop_continuous_paste_auto');
      get().applyStatus(s);
    } catch (err) {
      console.error(err);
    }
  },

  cancel: async () => {
    try {
      const s = await invoke<ContinuousStatus>('cancel_continuous_paste');
      get().applyStatus(s);
    } catch (err) {
      console.error(err);
    }
  },
}));
