import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useClipboardStore } from '../stores/clipboardStore';
import { useDatabase } from '../hooks/useDatabase';
import type { ClipboardGroup } from '../types/group';
import { PlusIcon, StarIcon } from './icons';

const tabBase =
  'flex-shrink-0 flex items-center gap-1.5 px-[13px] py-[5.5px] text-[12.5px] rounded-full transition-[background-color,color,box-shadow] duration-150';
const tabIdle = `${tabBase} bg-surface shadow-lift text-muted hover:text-faint hover:shadow-lift-hover`;
const tabActive = `${tabBase} bg-accent-soft text-accent font-semibold ring-1 ring-inset ring-accent-ring`;

export function GroupTabs() {
  const groups = useClipboardStore((s) => s.groups);
  const selectedGroup = useClipboardStore((s) => s.selectedGroup);
  const setSelectedGroup = useClipboardStore((s) => s.setSelectedGroup);
  const showFavorites = useClipboardStore((s) => s.showFavorites);
  const setShowFavorites = useClipboardStore((s) => s.setShowFavorites);
  const searchQuery = useClipboardStore((s) => s.searchQuery);
  const { loadItems, loadGroups } = useDatabase();
  const [menu, setMenu] = useState<{ group: ClipboardGroup; x: number; y: number } | null>(null);

  const handleSelectGroup = (group: typeof selectedGroup) => {
    setShowFavorites(false);
    setSelectedGroup(group);
    loadItems(searchQuery || undefined, group?.id ?? null, false);
  };

  const handleShowFavorites = () => {
    setSelectedGroup(null);
    setShowFavorites(true);
    loadItems(searchQuery || undefined, null, true);
  };

  const handleShowAll = () => {
    setSelectedGroup(null);
    setShowFavorites(false);
    loadItems(searchQuery || undefined, null, false);
  };

  const handleCreate = async () => {
    const name = window.prompt('新分组名称:');
    if (!name || !name.trim()) return;
    try {
      await invoke('create_group', { name: name.trim() });
      await loadGroups();
    } catch (err) {
      console.error('Create group failed:', err);
      alert(String(err));
    }
  };

  const handleRename = async (group: ClipboardGroup) => {
    const name = window.prompt('重命名分组', group.name);
    if (!name || !name.trim() || name.trim() === group.name) return;
    try {
      await invoke('update_group', { id: group.id, name: name.trim(), icon: group.icon });
      await loadGroups();
      // 同步选中分组的最新名称
      if (useClipboardStore.getState().selectedGroup?.id === group.id) {
        setSelectedGroup({ ...group, name: name.trim() });
      }
    } catch (err) {
      console.error('Rename group failed:', err);
      alert(String(err));
    }
  };

  const handleDelete = async (group: ClipboardGroup) => {
    if (!window.confirm(`删除分组 "${group.name}"?该分组下的条目将变为未分组。`)) return;
    try {
      await invoke('delete_group', { id: group.id });
      await loadGroups();
      if (useClipboardStore.getState().selectedGroup?.id === group.id) {
        setSelectedGroup(null);
        loadItems(searchQuery || undefined, null, false);
      }
    } catch (err) {
      console.error('Delete group failed:', err);
      alert(String(err));
    }
  };

  return (
    <div className="flex items-center gap-[7px] px-4 pt-1 pb-3 overflow-x-auto">
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
        onClick={handleCreate}
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
              onClick={() => { handleRename(menu.group); setMenu(null); }}
              className="w-full text-left px-3 py-1.5 text-[13px] text-ink hover:bg-app"
            >
              重命名
            </button>
            <button
              onClick={() => { handleDelete(menu.group); setMenu(null); }}
              className="w-full text-left px-3 py-1.5 text-[13px] text-danger hover:bg-app"
            >
              删除分组
            </button>
          </div>
        </>
      )}
    </div>
  );
}
