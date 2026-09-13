import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useClipboardStore } from '../stores/clipboardStore';
import type { ClipboardGroup } from '../types/group';
import { PlusIcon, StarIcon } from './icons';
import { PromptDialog, ConfirmDialog } from './Dialogs';

const tabBase =
  'flex-shrink-0 flex items-center gap-1.5 px-[13px] py-[5.5px] text-[12.5px] rounded-full transition-[background-color,color,box-shadow] duration-150 @max-narrow:px-[9px] @max-narrow:py-[4px] @max-narrow:text-[12px] @min-wide:px-[15px] @min-wide:py-[6px] @min-wide:text-[13px]';
const tabIdle = `${tabBase} bg-surface shadow-lift text-muted hover:text-faint hover:shadow-lift-hover`;
const tabActive = `${tabBase} bg-accent-soft text-accent font-semibold ring-1 ring-inset ring-accent-ring`;

export function GroupTabs() {
  const groups = useClipboardStore((s) => s.groups);
  const selectedGroup = useClipboardStore((s) => s.selectedGroup);
  const setSelectedGroup = useClipboardStore((s) => s.setSelectedGroup);
  const setView = useClipboardStore((s) => s.setView);
  const showFavorites = useClipboardStore((s) => s.showFavorites);
  const [menu, setMenu] = useState<{ group: ClipboardGroup; x: number; y: number } | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [renameTarget, setRenameTarget] = useState<ClipboardGroup | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ClipboardGroup | null>(null);

  // 列表重查由 useClipboardListener 对 store(选中分组/收藏过滤)的订阅负责,
  // 分组列表重查由 useDatabase 对 groups-changed 事件的监听负责 ——
  // 此处再显式 loadItems/loadGroups 会与之重复,每次操作发出 2-3 次相同查询
  const handleSelectGroup = (group: typeof selectedGroup) => {
    // 一次 set 同时更新两个字段:分开 set 会触发订阅方两次重查
    setView({ selectedGroup: group, showFavorites: false });
  };

  const handleShowFavorites = () => {
    setView({ selectedGroup: null, showFavorites: true });
  };

  const handleShowAll = () => {
    setView({ selectedGroup: null, showFavorites: false });
  };

  // 以下 handler 失败时直接 throw,由对话框内联展示错误
  const handleCreate = async (name: string) => {
    await invoke('create_group', { name });
    setCreateOpen(false);
  };

  const handleRename = async (group: ClipboardGroup, name: string) => {
    await invoke('update_group', { id: group.id, name, icon: group.icon });
    // 分组列表由 groups-changed 事件刷新;这里只同步选中分组的最新名称
    if (useClipboardStore.getState().selectedGroup?.id === group.id) {
      setSelectedGroup({ ...group, name });
    }
    setRenameTarget(null);
  };

  const handleDelete = async (group: ClipboardGroup) => {
    await invoke('delete_group', { id: group.id });
    // useDatabase 对 groups-changed 的监听会重查分组,并把已删除的
    // 选中分组清空(连带触发列表重查),这里无需再处理选中态
    setDeleteTarget(null);
  };

  return (
    <>
      <div className="flex items-center gap-[7px] px-4 pt-1 pb-3 overflow-x-auto @max-narrow:gap-[5px] @max-narrow:px-2.5 @max-narrow:pb-2 @min-wide:gap-2 @min-wide:px-5 @min-wide:pb-3.5">
        <button
          onClick={handleShowAll}
          className={!showFavorites && selectedGroup === null ? tabActive : tabIdle}
        >
          全部
        </button>
        <button
          onClick={handleShowFavorites}
          className={showFavorites ? tabActive : tabIdle}
        >
          <span className="text-star"><StarIcon size={13} /></span>
          收藏
        </button>
        {groups.map((group) => (
          <button
            key={group.id}
            onClick={() => handleSelectGroup(group)}
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu({ group, x: e.clientX, y: e.clientY });
            }}
            className={selectedGroup?.id === group.id ? tabActive : tabIdle}
          >
            {group.icon} {group.name}
          </button>
        ))}
        <button
          onClick={() => setCreateOpen(true)}
          title="新建分组"
          className="flex-shrink-0 flex items-center justify-center w-[27px] h-[27px] rounded-full text-faint hover:bg-hairline hover:text-muted transition-colors"
        >
          <PlusIcon size={14} />
        </button>
        {menu && (
          <>
            <div className="fixed inset-0 z-40" onClick={() => setMenu(null)} onContextMenu={(e) => { e.preventDefault(); setMenu(null); }} />
            <div
              className="fixed z-50 min-w-[120px] bg-surface rounded-lg shadow-dialog border border-hairline py-1"
              style={{ left: menu.x, top: menu.y }}
            >
              <button
                onClick={() => { setRenameTarget(menu.group); setMenu(null); }}
                className="w-full text-left px-3 py-1.5 text-[13px] text-ink hover:bg-app"
              >
                重命名
              </button>
              <button
                onClick={() => { setDeleteTarget(menu.group); setMenu(null); }}
                className="w-full text-left px-3 py-1.5 text-[13px] text-danger hover:bg-app"
              >
                删除分组
              </button>
            </div>
          </>
        )}
      </div>
      {createOpen && (
        <PromptDialog
          title="新建分组"
          label="分组名称:"
          placeholder="输入分组名称"
          onConfirm={handleCreate}
          onClose={() => setCreateOpen(false)}
        />
      )}
      {renameTarget && (
        <PromptDialog
          title="重命名分组"
          label="分组名称:"
          initialValue={renameTarget.name}
          onConfirm={async (name) => {
            if (name === renameTarget.name) {
              setRenameTarget(null);
              return;
            }
            await handleRename(renameTarget, name);
          }}
          onClose={() => setRenameTarget(null)}
        />
      )}
      {deleteTarget && (
        <ConfirmDialog
          title="删除分组"
          message={`删除分组 "${deleteTarget.name}"?\n该分组下的条目将变为未分组。`}
          onConfirm={() => handleDelete(deleteTarget)}
          onClose={() => setDeleteTarget(null)}
        />
      )}
    </>
  );
}
