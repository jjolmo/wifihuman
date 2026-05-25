<script lang="ts">
  import Radar from '$lib/components/Radar.svelte';
  import SettingsPanel from '$lib/components/SettingsPanel.svelte';
  import { scannerStore } from '$lib/stores/scanner.svelte';
  import { onMount } from 'svelte';

  onMount(() => { scannerStore.init(); });

  let showDeviceList = $state(false);
  let showSettings = $state(false);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && showSettings) showSettings = false;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="app">
  <!-- Header -->
  <div class="header">
    <div class="header-left">
      <span class="app-title">WiFi Human</span>
      <span class="app-subtitle">Probe Request Scanner</span>
    </div>
    <div class="header-right">
      {#if scannerStore.result}
        <div class="stat">
          <span class="stat-value">{scannerStore.result.estimated_humans}</span>
          <span class="stat-label">Humans (est.)</span>
        </div>
        <div class="stat">
          <span class="stat-value">{scannerStore.result.total_devices}</span>
          <span class="stat-label">Devices</span>
        </div>
      {/if}
      <button class="settings-btn" onclick={() => showSettings = true}>{'\u2699'}</button>
    </div>
  </div>

  <!-- Main area -->
  <div class="main">
    <!-- Radar -->
    <div class="radar-area">
      {#if scannerStore.isScanning}
        <div class="scanning-label">
          <span class="scan-pulse"></span>
          Scanning...
        </div>
      {:else if scannerStore.result?.interface === 'simulated'}
        <div class="simulated-label">DEMO MODE — simulated data</div>
      {/if}
      <Radar dots={scannerStore.radarDots} isScanning={scannerStore.isScanning} />
    </div>

    <!-- Controls panel -->
    <div class="panel">
      <div class="panel-section">
        <h3>Interface</h3>
        <select class="select" bind:value={scannerStore.selectedInterface}>
          {#each scannerStore.interfaces as iface}
            <option value={iface.name}>{iface.name} ({iface.mode})</option>
          {/each}
          {#if scannerStore.interfaces.length === 0}
            <option value="">No interfaces found</option>
          {/if}
        </select>
        <button class="btn btn-small" onclick={() => scannerStore.enableMonitor()}>
          Enable Monitor Mode
        </button>
      </div>

      <div class="panel-section">
        <h3>Duration</h3>
        <div class="duration-row">
          <input
            type="range"
            min="3" max="30" step="1"
            bind:value={scannerStore.scanDuration}
            class="slider"
          />
          <span class="duration-val">{scannerStore.scanDuration}s</span>
        </div>
      </div>

      <button
        class="btn btn-scan"
        onclick={() => scannerStore.scan()}
        disabled={scannerStore.isScanning}
      >
        {#if scannerStore.isScanning}
          <span class="scan-spinner"></span>
          Scanning...
        {:else}
          SCAN
        {/if}
      </button>

      {#if scannerStore.error && !scannerStore.error.includes('sudo')}
        <div class="error">{scannerStore.error}</div>
      {/if}

      <!-- Legend -->
      <div class="legend">
        <div class="legend-item">
          <span class="legend-dot real"></span>
          <span>Real MAC (identified)</span>
        </div>
        <div class="legend-item">
          <span class="legend-dot random"></span>
          <span>Randomized MAC</span>
        </div>
      </div>

      <!-- Dependencies -->
      {#if Object.keys(scannerStore.deps).length > 0}
        <div class="panel-section">
          <h3>Dependencies</h3>
          {#each Object.entries(scannerStore.deps) as [name, installed]}
            <div class="dep-item">
              <span class="dep-status" class:ok={installed}>{installed ? '\u2713' : '\u2717'}</span>
              <span>{name}</span>
              {#if !installed}
                <span class="dep-install">
                  {#if name === 'tshark'}(brew install wireshark){:else}(brew install aircrack-ng){/if}
                </span>
              {/if}
            </div>
          {/each}
          <p class="dep-note">Without tshark + monitor mode, scan runs in demo mode with simulated data.</p>
        </div>
      {/if}

      <!-- Device list toggle -->
      {#if scannerStore.result}
        <button class="btn btn-small" onclick={() => showDeviceList = !showDeviceList}>
          {showDeviceList ? 'Hide' : 'Show'} Device List
        </button>
      {/if}
    </div>
  </div>

  <!-- Device list -->
  {#if showDeviceList && scannerStore.result}
    <div class="device-list">
      <div class="dl-header">
        <span class="dl-col dl-mac">MAC Address</span>
        <span class="dl-col dl-rssi">Signal</span>
        <span class="dl-col dl-vendor">Vendor</span>
        <span class="dl-col dl-ssid">Probing</span>
        <span class="dl-col dl-type">Type</span>
      </div>
      {#each scannerStore.result.devices.sort((a, b) => b.rssi - a.rssi) as device}
        <div class="dl-row">
          <span class="dl-col dl-mac mono">{device.mac}</span>
          <span class="dl-col dl-rssi">
            <span class="rssi-bar" style="width: {Math.max(5, 100 + device.rssi)}%"></span>
            {device.rssi} dBm
          </span>
          <span class="dl-col dl-vendor">{device.vendor}</span>
          <span class="dl-col dl-ssid mono">{device.ssid_probed || '—'}</span>
          <span class="dl-col dl-type" class:random={device.is_randomized}>
            {device.is_randomized ? 'Random' : 'Real'}
          </span>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if showSettings}
  <SettingsPanel onclose={() => showSettings = false} />
{/if}

<style>
  .app { display: flex; flex-direction: column; height: 100vh; overflow: hidden; }

  /* Header */
  .header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 12px 20px; background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border); flex-shrink: 0;
  }
  .header-left { display: flex; align-items: baseline; gap: 10px; }
  .app-title { font-size: 16px; font-weight: 700; color: var(--color-accent); }
  .app-subtitle { font-size: 12px; color: var(--color-text-muted); }
  .header-right { display: flex; gap: 24px; }
  .stat { display: flex; flex-direction: column; align-items: center; }
  .stat-value { font-size: 28px; font-weight: 700; color: var(--color-accent); font-family: var(--font-mono); line-height: 1; }
  .stat-label { font-size: 10px; color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 1px; margin-top: 2px; }
  .settings-btn {
    background: none; border: none; color: var(--color-text-secondary);
    font-size: 20px; cursor: pointer; padding: 4px; margin-left: 8px;
  }
  .settings-btn:hover { color: var(--color-text-primary); }

  /* Main */
  .main { display: flex; flex: 1; overflow: hidden; }
  .radar-area {
    flex: 1; display: flex; align-items: center; justify-content: center;
    position: relative; padding: 20px;
  }
  .scanning-label {
    position: absolute; top: 20px; left: 50%; transform: translateX(-50%);
    display: flex; align-items: center; gap: 8px;
    font-size: 13px; color: var(--color-accent); font-weight: 600;
    letter-spacing: 2px; text-transform: uppercase;
  }
  .scan-pulse {
    width: 10px; height: 10px; border-radius: 50%; background: var(--color-accent);
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse { 0%, 100% { opacity: 1; transform: scale(1); } 50% { opacity: 0.4; transform: scale(0.8); } }
  .simulated-label {
    position: absolute; top: 20px; left: 50%; transform: translateX(-50%);
    font-size: 11px; color: #ff6b35; font-weight: 600;
    letter-spacing: 2px; text-transform: uppercase;
    background: #ff6b3515; border: 1px solid #ff6b3530;
    padding: 4px 14px; border-radius: 20px;
  }

  /* Panel */
  .panel {
    width: 260px; background: var(--color-bg-secondary);
    border-left: 1px solid var(--color-border);
    padding: 16px; overflow-y: auto; flex-shrink: 0;
    display: flex; flex-direction: column; gap: 14px;
  }
  .panel-section { display: flex; flex-direction: column; gap: 6px; }
  h3 { font-size: 11px; color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 1.5px; font-weight: 600; }

  .select {
    padding: 6px 8px; background: var(--color-bg-tertiary); border: 1px solid var(--color-border);
    border-radius: 6px; color: var(--color-text-primary); font-size: 12px; width: 100%;
  }
  .select:focus { outline: none; border-color: var(--color-accent); }

  .duration-row { display: flex; align-items: center; gap: 10px; }
  .slider { flex: 1; accent-color: var(--color-accent); }
  .duration-val { font-family: var(--font-mono); font-size: 13px; color: var(--color-accent); min-width: 30px; }

  .btn {
    padding: 10px 20px; border: none; border-radius: 8px;
    font-size: 13px; font-weight: 600; cursor: pointer; transition: all 0.15s;
  }
  .btn-scan {
    background: var(--color-accent); color: #0a0e14; font-size: 16px;
    letter-spacing: 3px; padding: 14px; border-radius: 10px;
    display: flex; align-items: center; justify-content: center; gap: 10px;
  }
  .btn-scan:hover { background: var(--color-accent-hover); }
  .btn-scan:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-small {
    background: var(--color-bg-tertiary); color: var(--color-text-secondary);
    font-size: 11px; padding: 6px 12px; border: 1px solid var(--color-border);
  }
  .btn-small:hover { background: var(--color-bg-hover); }

  .scan-spinner {
    width: 16px; height: 16px; border: 2px solid #0a0e1440;
    border-top-color: #0a0e14; border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .error {
    background: #ff6b3515; border: 1px solid #ff6b3540; border-radius: 6px;
    padding: 8px 10px; font-size: 12px; color: #ff6b35; line-height: 1.4;
  }

  .legend { display: flex; flex-direction: column; gap: 6px; }
  .legend-item { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--color-text-secondary); }
  .legend-dot { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }
  .legend-dot.real { background: var(--color-dot-human); }
  .legend-dot.random { background: #ff6b35; }

  .dep-item { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--color-text-secondary); }
  .dep-status { font-size: 14px; color: #ff6b35; }
  .dep-status.ok { color: #2ea043; }
  .dep-install { font-size: 10px; color: var(--color-text-muted); font-family: var(--font-mono); }
  .dep-note { font-size: 11px; color: var(--color-text-muted); line-height: 1.4; margin-top: 4px; }

  /* Device list */
  .device-list {
    max-height: 200px; overflow-y: auto; background: var(--color-bg-secondary);
    border-top: 1px solid var(--color-border); flex-shrink: 0;
  }
  .dl-header, .dl-row { display: flex; padding: 6px 16px; gap: 8px; font-size: 12px; }
  .dl-header { background: var(--color-bg-tertiary); color: var(--color-text-muted); font-weight: 600; position: sticky; top: 0; text-transform: uppercase; letter-spacing: 0.5px; font-size: 10px; }
  .dl-row { border-bottom: 1px solid var(--color-border); color: var(--color-text-secondary); }
  .dl-row:hover { background: var(--color-bg-hover); }
  .dl-col { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dl-mac { flex: 2; }
  .dl-rssi { flex: 1.5; display: flex; align-items: center; gap: 6px; }
  .dl-vendor { flex: 1; }
  .dl-ssid { flex: 1.5; }
  .dl-type { flex: 0.7; font-weight: 600; color: var(--color-accent); }
  .dl-type.random { color: #ff6b35; }
  .mono { font-family: var(--font-mono); font-size: 11px; }
  .rssi-bar { height: 3px; background: var(--color-accent); border-radius: 2px; min-width: 4px; }
</style>
