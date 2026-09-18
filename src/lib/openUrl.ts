import { invoke } from '@tauri-apps/api/core';

/** 经后端白名单校验后打开外部链接(更新下载页) */
export function openUrl(url: string) {
  invoke('open_external_url', { url }).catch((err) => console.error('Open URL failed:', err));
}
