export type CloseBehavior = 'ask' | 'minimize' | 'close';

export interface AppSettings {
  start_with_windows: boolean;
  retention_days: number;
  max_item_count: number;
  hotkey_modifier: string;
  hotkey_key: string;
  paused: boolean;
  close_behavior: CloseBehavior;
  win_v_integration: boolean;
  pinned: boolean;
  excluded_apps: string[];
  excluded_patterns: string[];
  detect_sensitive: boolean;
  // 误伤豁免:用户点过「仍要记录」的内容 hash
  excluded_allowlist: string[];
  // 短信验证码自动捕获(默认关闭,开启需授予系统通知访问权限)
  sms_code_enabled: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  start_with_windows: false,
  retention_days: 30,
  max_item_count: 500,
  hotkey_modifier: 'Ctrl+Shift',
  hotkey_key: 'V',
  paused: false,
  close_behavior: 'ask',
  win_v_integration: false,
  pinned: true,
  excluded_apps: ['keepass.exe', 'keepassxc.exe', '1password.exe', 'bitwarden.exe'],
  excluded_patterns: [],
  detect_sensitive: true,
  excluded_allowlist: [],
  sms_code_enabled: false,
};
