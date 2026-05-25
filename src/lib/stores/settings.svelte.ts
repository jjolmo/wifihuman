import { invoke } from '@tauri-apps/api/core';

class SettingsStore {
  private cache = $state<Record<string, string>>({});

  constructor() {
    if (typeof window !== 'undefined') {
      try {
        for (let i = 0; i < localStorage.length; i++) {
          const k = localStorage.key(i);
          if (k?.startsWith('wh:')) {
            this.cache[k.slice(3)] = localStorage.getItem(k)!;
          }
        }
      } catch {}
    }
  }

  getSetting(key: string): string | undefined { return this.cache[key]; }

  async setSetting(key: string, value: string) {
    this.cache[key] = value;
    try { localStorage.setItem(`wh:${key}`, value); } catch {}
    await invoke('set_setting', { key, value }).catch(() => {});
  }

  async init() {}
}

export const settingsStore = new SettingsStore();
