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
});
