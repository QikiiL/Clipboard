import { describe, it, expect, beforeEach } from 'vitest';
import { useClipboardStore } from '../clipboardStore';
import { ClipboardType } from '../../types/clipboard';

const mockItem = {
  id: 1,
  type: ClipboardType.Text,
  content: 'test content',
  content_hash: 'abc123',
  file_path: null,
  preview: 'test content',
  copy_count: 1,
  is_favorite: false,
  group_id: null,
  created_at: '2026-06-05T00:00:00',
  last_used_at: '2026-06-05T00:00:00',
};

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

  it('addItem prepends to items', () => {
    useClipboardStore.getState().addItem(mockItem);
    expect(useClipboardStore.getState().items).toHaveLength(1);
    expect(useClipboardStore.getState().items[0].id).toBe(1);
  });

  it('updateItem replaces matching item', () => {
    useClipboardStore.getState().addItem(mockItem);
    const updated = { ...mockItem, copy_count: 5 };
    useClipboardStore.getState().updateItem(updated);
    expect(useClipboardStore.getState().items[0].copy_count).toBe(5);
  });

  it('removeItem filters out item', () => {
    useClipboardStore.getState().addItem(mockItem);
    useClipboardStore.getState().removeItem(1);
    expect(useClipboardStore.getState().items).toHaveLength(0);
  });
});
