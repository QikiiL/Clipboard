import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react';
import { ContentEditorDialog } from '../ContentEditorDialog';
import { ClipboardType } from '../../types/clipboard';
import type { ClipboardItem } from '../../types/clipboard';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

function item(overrides: Partial<ClipboardItem> = {}): ClipboardItem {
  return {
    id: 42,
    type: ClipboardType.Text,
    content: '纯文本内容',
    content_hash: 'deadbeef',
    file_path: null,
    preview: '预览',
    is_favorite: false,
    group_id: null,
    created_at: '2026-10-22T00:00:00Z',
    last_used_at: '2026-10-22T00:00:00Z',
    html_path: 'D:/clipboard/data/formats/b822feb8084eb063.html',
    rtf_path: null,
    ...overrides,
  };
}

const saveButton = () => screen.getByRole('button', { name: '保存' }) as HTMLButtonElement;
const calledCommands = () => invokeMock.mock.calls.map((c) => c[0] as string);

describe('ContentEditorDialog · 富文本载入中不许保存', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('禁用保存按钮,点下去也不会把富文本格式清掉', async () => {
    // get_rich_preview 迟迟不返回 → richLoading 期间 showTabs=false、iframe 未挂载。
    // 此时若允许保存,handleSave 会落到纯文本分支,把条目的富文本格式清掉。
    let resolvePreview: (v: { html: string | null }) => void = () => {};
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'get_rich_preview') {
        return new Promise((resolve) => {
          resolvePreview = resolve as (v: { html: string | null }) => void;
        });
      }
      return Promise.resolve(null);
    });

    render(<ContentEditorDialog item={item()} onClose={() => {}} />);

    // 载入态:只有 loading 占位,没有富文本/纯文本切换
    await waitFor(() => expect(screen.getByText('正在载入富文本…')).toBeTruthy());
    expect(screen.queryByRole('button', { name: '富文本编辑' })).toBeNull();

    expect(saveButton().disabled).toBe(true);
    fireEvent.click(saveButton());

    // 关键断言:一次写库都没有发生(尤其不能有 update_item_content)
    expect(calledCommands()).toEqual(['get_rich_preview']);

    // 片段到位后按钮恢复可用,切换出现
    await act(async () => {
      resolvePreview({ html: '<p>你好</p>' });
    });
    await waitFor(() => expect(saveButton().disabled).toBe(false));
    expect(screen.getByRole('button', { name: '富文本编辑' })).toBeTruthy();
  });
});
