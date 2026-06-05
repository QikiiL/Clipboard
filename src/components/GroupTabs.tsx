import { useClipboardStore } from '../stores/clipboardStore';
import { useDatabase } from '../hooks/useDatabase';

export function GroupTabs() {
  const { groups, selectedGroup, setSelectedGroup, showFavorites, setShowFavorites } = useClipboardStore();
  const { loadItems } = useDatabase();
  const searchQuery = useClipboardStore((s) => s.searchQuery);

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

  return (
    <div className="flex items-center gap-2 px-4 py-2 overflow-x-auto border-b border-gray-100 dark:border-gray-800">
      <button
        onClick={handleShowAll}
        className={`flex-shrink-0 px-3 py-1.5 text-sm rounded-full transition-colors ${
          !showFavorites && selectedGroup === null
            ? 'bg-blue-500 text-white'
            : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
        }`}
      >
        全部
      </button>
      <button
        onClick={handleShowFavorites}
        className={`flex-shrink-0 px-3 py-1.5 text-sm rounded-full transition-colors ${
          showFavorites
            ? 'bg-blue-500 text-white'
            : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
        }`}
      >
        ⭐ 收藏
      </button>
      {groups.map((group) => (
        <button
          key={group.id}
          onClick={() => handleSelectGroup(group)}
          className={`flex-shrink-0 px-3 py-1.5 text-sm rounded-full transition-colors ${
            selectedGroup?.id === group.id
              ? 'bg-blue-500 text-white'
              : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
          }`}
        >
          {group.icon} {group.name}
        </button>
      ))}
    </div>
  );
}
