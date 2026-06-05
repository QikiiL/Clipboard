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

  setItems: (items: ClipboardItem[]) => void;
  addItem: (item: ClipboardItem) => void;
  updateItem: (item: ClipboardItem) => void;
  removeItem: (id: number) => void;
  setGroups: (groups: ClipboardGroup[]) => void;
  setSelectedGroup: (group: ClipboardGroup | null) => void;
  setSearchQuery: (query: string) => void;
  setShowFavorites: (show: boolean) => void;
  setLoading: (loading: boolean) => void;
}

export const useClipboardStore = create<ClipboardStore>((set) => ({
  items: [],
  groups: [],
  selectedGroup: null,
  searchQuery: '',
  showFavorites: false,
  isLoading: false,

  setItems: (items) => set({ items }),
  addItem: (item) => set((state) => ({ items: [item, ...state.items] })),
  updateItem: (item) => set((state) => ({
    items: state.items.map((i) => (i.id === item.id ? item : i)),
  })),
  removeItem: (id) => set((state) => ({
    items: state.items.filter((i) => i.id !== id),
  })),
  setGroups: (groups) => set({ groups }),
  setSelectedGroup: (group) => set({ selectedGroup: group }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setShowFavorites: (showFavorites) => set({ showFavorites }),
  setLoading: (isLoading) => set({ isLoading }),
}));
