import { describe, it, expect, beforeEach } from 'vitest';
import { useClipboardStore } from '../clipboardStore';

describe('clipboardStore', () => {
  beforeEach(() => {
    useClipboardStore.setState({
      items: [],
      groups: [],
      selectedGroup: null,
      searchQuery: '',
      showFavorites: false,
      isLoading: false,
    });
  });

  it('incrementRequestId increments and returns the new id', () => {
    const id1 = useClipboardStore.getState().incrementRequestId();
    const id2 = useClipboardStore.getState().incrementRequestId();
    expect(id2).toBe(id1 + 1);
    expect(useClipboardStore.getState().requestId).toBe(id2);
  });

  it('setMaxItems updates the query limit', () => {
    useClipboardStore.getState().setMaxItems(100000);
    expect(useClipboardStore.getState().maxItems).toBe(100000);
  });

  it('setView updates selectedGroup and showFavorites atomically', () => {
    // 单次 set 应用两个字段:订阅方(useClipboardListener)只会收到一次
    // 通知,不会为每个字段各触发一次重查
    const group = { id: 1, name: '工作', icon: '📁', sort_order: 0, created_at: '' };
    useClipboardStore.getState().setView({ selectedGroup: group, showFavorites: false });
    expect(useClipboardStore.getState().selectedGroup).toEqual(group);
    expect(useClipboardStore.getState().showFavorites).toBe(false);

    useClipboardStore.getState().setView({ selectedGroup: null, showFavorites: true });
    expect(useClipboardStore.getState().selectedGroup).toBeNull();
    expect(useClipboardStore.getState().showFavorites).toBe(true);
  });
});
