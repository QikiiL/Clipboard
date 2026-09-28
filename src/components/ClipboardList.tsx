import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useClipboardStore } from '../stores/clipboardStore';
import { useContinuousPasteStore } from '../stores/continuousPasteStore';
import { ClipboardItemCard } from './ClipboardItem';
import { ContentEditorDialog } from './ContentEditorDialog';
import { ClipboardIcon, SearchIcon, StarOutlineIcon, FolderIcon } from './icons';
import { ghostBtn } from './settings-controls';
import { invoke } from '@tauri-apps/api/core';
import type { ClipboardItem as ClipboardItemType } from '../types/clipboard';
import { useRubberBandSelect } from '../hooks/useRubberBandSelect';

export function ClipboardList() {
  const { items, isLoading } = useClipboardStore();
  // 空状态分场景文案需要的过滤条件(仅在 items 为空时参与渲染)
  const searchQuery = useClipboardStore((s) => s.searchQuery);
  const showFavorites = useClipboardStore((s) => s.showFavorites);
  const selectedGroup = useClipboardStore((s) => s.selectedGroup);
  const scrollRef = useRef<HTMLDivElement>(null);
  // 正在编辑的条目:null 即不展示弹窗。弹窗必须挂在滚动容器之外渲染,
  // 否则会被滚动容器裁剪、且随虚拟列表的卸载而消失
  const [editingItem, setEditingItem] = useState<ClipboardItemType | null>(null);
  const handlePreview = useCallback((item: ClipboardItemType) => setEditingItem(item), []);

  const boxMode = useContinuousPasteStore((s) => s.mode);
  const queueActive = useContinuousPasteStore((s) => s.status?.active ?? false);
  const applyBand = useContinuousPasteStore((s) => s.applyBand);
  const setBandPreview = useContinuousPasteStore((s) => s.setBandPreview);
  const clearSelection = useContinuousPasteStore((s) => s.clearSelection);
  const recomputeOrder = useContinuousPasteStore((s) => s.recomputeOrder);

  // 虚拟滚动:只渲染可视区附近的行,长列表下 DOM 节点数与内存恒定。
  // 行用 top 定位(非 transform),行内 position:fixed 的悬停预览与菜单遮罩才能相对视口定位。
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 55,
    overscan: 6,
    getItemKey: (index) => items[index].id,
  });

  // 框选模式下条目增删会改变显示顺序,粘贴序号徽标需重算
  useEffect(() => {
    if (boxMode) recomputeOrder();
  }, [items, boxMode, recomputeOrder]);

  const bandEnabled = boxMode && !queueActive;
  const handleBandApply = useCallback(
    (ids: number[]) => applyBand(ids),
    [applyBand]
  );
  const handleBandPreview = useCallback(
    (ids: number[]) => setBandPreview(ids),
    [setBandPreview]
  );
  const handleBackgroundClick = useCallback(() => clearSelection(), [clearSelection]);

  const { band, didDragRef } = useRubberBandSelect({
    containerRef: scrollRef,
    enabled: bandEnabled,
    onApply: handleBandApply,
    onPreview: handleBandPreview,
    onBackgroundClick: handleBackgroundClick,
  });

  // 默认粘贴;状态栏可手动切换为"仅复制"
  const handleActivate = useCallback(async (item: ClipboardItemType) => {
    try {
      await invoke('activate_item', { id: item.id });
    } catch (err) {
      console.error('Activate item failed:', err);
    }
  }, []);

  const handleDelete = useCallback(async (id: number) => {
    try {
      await invoke('delete_item', { id });
    } catch (err) {
      console.error('Delete failed:', err);
    }
  }, []);

  const handleToggleFavorite = useCallback(async (id: number) => {
    try {
      await invoke('toggle_favorite', { id });
    } catch (err) {
      console.error('Toggle favorite failed:', err);
    }
  }, []);

  // 仅首次加载(尚无内容)时显示占位;后台刷新(收藏/删除/新复制触发的重查)原地更新,避免列表闪烁
  if (isLoading && items.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-[13px] text-faint">加载中…</div>
      </div>
    );
  }

  if (items.length === 0) {
    // 空状态分场景:搜索无结果 / 收藏为空 / 空分组 / 真·无记录。
    // 统一文案会让用户以为"没记录成功",搜索场景还要给「清空搜索」的出口
    const empty: { icon: ReactNode; title: ReactNode; desc: string; action?: ReactNode } =
      searchQuery
        ? {
            icon: <SearchIcon size={26} />,
            title: (
              <>
                没有匹配「<span className="text-muted">{searchQuery}</span>」的记录
              </>
            ),
            desc: '换个关键词试试',
            action: (
              <button
                onClick={() => useClipboardStore.getState().setSearchQuery('')}
                className={`${ghostBtn} mt-3 h-[30px] px-4 text-[12px]`}
              >
                清空搜索
              </button>
            ),
          }
        : showFavorites
          ? {
              icon: <StarOutlineIcon size={26} />,
              title: '还没有收藏',
              desc: '点条目上的 ☆ 把常用内容收进来',
            }
          : selectedGroup
            ? {
                icon: <FolderIcon size={26} />,
                title: '这个分组还是空的',
                desc: '点条目上的文件夹图标把内容归进来',
              }
            : {
                icon: <ClipboardIcon size={28} />,
                title: '暂无剪贴板记录',
                desc: '复制内容后将自动出现在这里',
              };
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-faint px-6">
        {empty.icon}
        <p className="mt-3 text-[13px] text-center break-all max-w-[300px]">{empty.title}</p>
        <p className="mt-1 text-[12px] text-center">{empty.desc}</p>
        {empty.action}
      </div>
    );
  }

  return (
    <>
      <div
        ref={scrollRef}
        className={`flex-1 overflow-y-auto px-3 pt-1 pb-2.5 @max-narrow:px-2 @min-wide:px-4 ${
          bandEnabled ? 'cursor-crosshair' : ''
        }`}
        onClickCapture={(e) => {
          // 拖拽框选结束的那次 click 会落到条目上:拦截,避免把拖拽误判成点击
          if (didDragRef.current) {
            didDragRef.current = false;
            e.stopPropagation();
          }
        }}
      >
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const item = items[virtualRow.index];
            return (
              <div
                key={item.id}
                data-index={virtualRow.index}
                data-item-id={item.id}
                ref={virtualizer.measureElement}
                className="pb-[3px]"
                style={{ position: 'absolute', top: virtualRow.start, left: 0, width: '100%' }}
              >
                <ClipboardItemCard
                  item={item}
                  onActivate={handleActivate}
                  onDelete={handleDelete}
                  onToggleFavorite={handleToggleFavorite}
                  onPreview={handlePreview}
                />
              </div>
            );
          })}
        </div>
      </div>
      {band && (
        // 框选矩形:视口坐标下的半透明描边框
        <div
          className="fixed z-[60] pointer-events-none border border-accent bg-accent/10 rounded-[4px]"
          style={{ left: band.left, top: band.top, width: band.width, height: band.height }}
        />
      )}
      <ContentEditorDialog item={editingItem} onClose={() => setEditingItem(null)} />
    </>
  );
}
