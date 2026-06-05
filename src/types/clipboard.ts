export enum ClipboardType {
  Text = 0,
  Link = 1,
  Image = 2,
  File = 3,
}

export interface ClipboardItem {
  id: number;
  type: ClipboardType;
  content: string;
  content_hash: string;
  file_path: string | null;
  preview: string;
  copy_count: number;
  is_favorite: boolean;
  group_id: number | null;
  created_at: string;
  last_used_at: string;
}

export interface ClipboardChangedPayload {
  item: ClipboardItem;
  action: 'new' | 'updated';
}
