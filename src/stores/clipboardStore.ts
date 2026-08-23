import { create } from 'zustand';
import type { ClipboardItem } from '../types/clipboard';
import type { ClipboardGroup } from '../types/group';

interface ClipboardStore {
  items: ClipboardItem[];
  groups: ClipboardGroup[];
  selectedGroup: ClipboardGroup | null;
  searchQuery: string;
  showFavorites: boolean;
  isLoading: boolean;
  isPaused: boolean;
  requestId: number;
  maxItems: number;

  setItems: (items: ClipboardItem[]) => void;
  setGroups: (groups: ClipboardGroup[]) => void;
  setSelectedGroup: (group: ClipboardGroup | null) => void;
  setSearchQuery: (query: string) => void;
  setShowFavorites: (show: boolean) => void;
  setLoading: (loading: boolean) => void;
  setPaused: (paused: boolean) => void;
  setMaxItems: (maxItems: number) => void;
  incrementRequestId: () => number;
}

export const useClipboardStore = create<ClipboardStore>((set, get) => ({
  items: [],
  groups: [],
  selectedGroup: null,
  searchQuery: '',
  showFavorites: false,
  isLoading: false,
  isPaused: false,
  requestId: 0,
  maxItems: 500,

  setItems: (items) => set({ items }),
  setGroups: (groups) => set({ groups }),
  setSelectedGroup: (group) => set({ selectedGroup: group }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setShowFavorites: (showFavorites) => set({ showFavorites }),
  setLoading: (isLoading) => set({ isLoading }),
  setPaused: (isPaused) => set({ isPaused }),
  setMaxItems: (maxItems) => set({ maxItems }),
  incrementRequestId: () => {
    const id = get().requestId + 1;
    set({ requestId: id });
    return id;
  },
}));
