<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  let { onclose }: { onclose: () => void } = $props();

  let activeTab = $state('general');
  let updateInfo = $state<any>(null);
  let checkingUpdate = $state(false);
  let updating = $state(false);
  let updateMessage = $state('');
  let desktopMessage = $state('');

  async function checkUpdates() {
    checkingUpdate = true;
    updateMessage = '';
    try {
      updateInfo = await invoke('check_for_updates');
    } catch (e: any) {
      updateInfo = { error: e.toString() };
    } finally {
      checkingUpdate = false;
    }
  }

  async function runUpdate() {
    updating = true;
    updateMessage = '';
    try {
      const msg = await invoke<string>('run_self_update');
      updateMessage = msg;
    } catch (e: any) {
      updateMessage = e.toString();
    } finally {
      updating = false;
    }
  }

  async function createDesktopEntry() {
    try {
      const path = await invoke<string>('create_desktop_entry');
      desktopMessage = `Created: ${path}`;
    } catch (e: any) {
      desktopMessage = e.toString();
    }
  }

  const tabs = [
    { id: 'general', label: 'General', icon: '\u2699' },
    { id: 'about', label: 'About', icon: '\u2139' },
  ];
</script>

<div class="overlay" onclick={onclose} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog">
    <div class="sidebar">
      <div class="sidebar-title">Settings</div>
      {#each tabs as tab}
        <button class="tab-btn" class:active={activeTab === tab.id} onclick={() => activeTab = tab.id}>
          <span class="tab-icon">{tab.icon}</span>
          {tab.label}
        </button>
      {/each}
    </div>

    <div class="content">
      <button class="close-btn" onclick={onclose}>&times;</button>

      {#if activeTab === 'general'}
        <h2>General</h2>
        <div class="field">
          <h3>Desktop Entry (Linux)</h3>
          <p class="description">Create a .desktop entry so WiFi Human appears in your app launcher.</p>
          <button class="primary-btn" onclick={createDesktopEntry}>Create Desktop Entry</button>
          {#if desktopMessage}
            <p class="save-msg" style="margin-top: 8px">{desktopMessage}</p>
          {/if}
        </div>

        <div class="field" style="margin-top: 24px;">
          <h3>Requirements</h3>
          <p class="description">For real WiFi scanning (not demo mode), you need:</p>
          <ul class="req-list">
            <li><strong>tshark</strong> — <code>brew install wireshark</code> (macOS) or <code>apt install tshark</code> (Linux)</li>
            <li><strong>Monitor mode</strong> — USB WiFi adapter with chipset Atheros AR9271, Ralink RT3070, or similar</li>
            <li><strong>airmon-ng</strong> (optional) — <code>brew install aircrack-ng</code> or <code>apt install aircrack-ng</code></li>
          </ul>
        </div>

      {:else if activeTab === 'about'}
        <h2>WiFi Human</h2>
        <p class="description">WiFi-based human presence detector using probe request sniffing</p>
        <div class="about-info">
          <div class="badge-row">
            <span class="tech-badge">Tauri v2</span>
            <span class="tech-badge">SvelteKit</span>
            <span class="tech-badge">Rust</span>
            <span class="tech-badge">tshark</span>
          </div>
          <p class="version">Version {__APP_VERSION__}</p>
        </div>
        <div class="field">
          <button class="primary-btn" onclick={checkUpdates} disabled={checkingUpdate}>
            {checkingUpdate ? 'Checking...' : 'Check for Updates'}
          </button>
          {#if updateInfo}
            {#if updateInfo.error}
              <p class="error">{updateInfo.error}</p>
            {:else if updateInfo.has_update}
              <p class="update-available">New version available: v{updateInfo.latest_version}</p>
              <button class="primary-btn update-btn" onclick={runUpdate} disabled={updating} style="margin-top: 8px">
                {updating ? 'Updating...' : 'Install Update'}
              </button>
              {#if updateMessage}
                <p class="save-msg" style="margin-top: 6px">{updateMessage}</p>
              {/if}
            {:else}
              <p class="save-msg">You're up to date!</p>
            {/if}
          {/if}
        </div>
        <div class="field">
          <a href="https://github.com/jjolmo/wifihuman" class="link" target="_blank">GitHub Repository</a>
        </div>
      {/if}
    </div>
  </div>
</div>

<script lang="ts" module>
  declare const __APP_VERSION__: string;
</script>

<style>
  .overlay {
    position: fixed; inset: 0; background: rgba(0, 0, 0, 0.6);
    display: flex; align-items: center; justify-content: center; z-index: 100;
  }
  .modal {
    display: flex; width: 650px; height: 460px;
    background: var(--color-bg-primary); border: 1px solid var(--color-border);
    border-radius: 10px; overflow: hidden;
  }
  .sidebar {
    width: 170px; background: var(--color-bg-secondary); border-right: 1px solid var(--color-border);
    padding: 16px 0; flex-shrink: 0;
  }
  .sidebar-title {
    padding: 0 16px 12px; font-weight: 600; font-size: 14px; color: var(--color-text-primary);
    border-bottom: 1px solid var(--color-border); margin-bottom: 8px;
  }
  .tab-btn {
    display: flex; align-items: center; gap: 8px; width: 100%;
    padding: 8px 16px; border: none; background: none;
    color: var(--color-text-secondary); font-size: 13px; cursor: pointer; text-align: left;
  }
  .tab-btn:hover { background: var(--color-bg-hover); }
  .tab-btn.active { background: var(--color-bg-selected); color: var(--color-text-primary); }
  .tab-icon { font-size: 14px; }
  .content { flex: 1; padding: 24px; overflow-y: auto; position: relative; }
  .close-btn {
    position: absolute; top: 12px; right: 16px; background: none; border: none;
    color: var(--color-text-secondary); font-size: 20px; cursor: pointer;
  }
  .close-btn:hover { color: var(--color-text-primary); }
  h2 { font-size: 18px; margin-bottom: 8px; }
  h3 { font-size: 14px; margin-bottom: 6px; color: var(--color-accent); }
  .description { color: var(--color-text-secondary); font-size: 13px; margin-bottom: 16px; line-height: 1.5; }
  .field { margin-bottom: 16px; }
  .primary-btn {
    padding: 8px 20px; background: var(--color-accent); color: #0a0e14; border: none;
    border-radius: 6px; font-size: 13px; font-weight: 600; cursor: pointer;
  }
  .primary-btn:hover { opacity: 0.9; }
  .primary-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .save-msg { color: #2ea043; font-size: 12px; margin-top: 6px; }
  .error { color: #ff6b35; font-size: 12px; margin-top: 8px; }
  .update-available { color: var(--color-accent); font-size: 13px; margin-top: 8px; }
  .about-info { margin-bottom: 16px; }
  .badge-row { display: flex; gap: 8px; margin-bottom: 8px; }
  .tech-badge {
    padding: 3px 10px; background: var(--color-bg-tertiary); border: 1px solid var(--color-border);
    border-radius: 4px; font-size: 11px; color: var(--color-text-secondary);
  }
  .version { font-size: 12px; color: var(--color-text-muted); }
  .link { color: var(--color-accent); font-size: 13px; text-decoration: none; }
  .link:hover { text-decoration: underline; }
  .req-list {
    padding-left: 20px; font-size: 13px; color: var(--color-text-secondary); line-height: 1.8;
  }
  .req-list code {
    background: var(--color-bg-tertiary); padding: 1px 6px; border-radius: 3px;
    font-family: var(--font-mono); font-size: 12px;
  }
</style>
