export interface AppSettings {
  start_with_windows: boolean;
  retention_days: number;
  max_item_count: number;
  hotkey_modifier: string;
  hotkey_key: string;
  paused: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  start_with_windows: false,
  retention_days: 30,
  max_item_count: 500,
  hotkey_modifier: 'Ctrl+Shift',
  hotkey_key: 'V',
  paused: false,
};
