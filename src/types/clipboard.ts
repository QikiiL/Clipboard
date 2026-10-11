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
  is_favorite: boolean;
  group_id: number | null;
  created_at: string;
  last_used_at: string;
  /** 富文本格式文件路径(formats 目录下的 CF_HTML / RTF 原始字节),
   *  仅文本类条目可能携带;为空即无富格式,预览弹窗按纯文本处理 */
  html_path?: string | null;
  rtf_path?: string | null;
}
