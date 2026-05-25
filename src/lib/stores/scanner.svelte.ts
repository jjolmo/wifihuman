import { invoke } from '@tauri-apps/api/core';
import type { DetectedDevice, ScanResult, WiFiInterface, RadarDot } from '$lib/types';

class ScannerStore {
  result = $state<ScanResult | null>(null);
  isScanning = $state(false);
  error = $state<string | null>(null);
  interfaces = $state<WiFiInterface[]>([]);
  selectedInterface = $state('');
  scanDuration = $state(10);
  deps = $state<Record<string, boolean>>({});

  get radarDots(): RadarDot[] {
    if (!this.result) return [];
    return this.result.devices.map((device, i) => {
      // Map RSSI to distance: -30 = very close (0.1), -90 = far (0.95)
      const dist = Math.max(0.1, Math.min(0.95, (Math.abs(device.rssi) - 25) / 70));
      // Distribute dots around the radar using golden angle for even spread
      const goldenAngle = 137.508;
      const angle = (i * goldenAngle * Math.PI) / 180;
      return {
        x: 0.5 + dist * 0.45 * Math.cos(angle),
        y: 0.5 + dist * 0.45 * Math.sin(angle),
        device,
        angle: (angle * 180) / Math.PI,
        distance: dist
      };
    });
  }

  async init() {
    try {
      this.deps = await invoke<Record<string, boolean>>('check_dependencies');
      this.interfaces = await invoke<WiFiInterface[]>('list_wifi_interfaces');
      if (this.interfaces.length > 0) {
        this.selectedInterface = this.interfaces[0].name;
      }
    } catch (e: any) {
      this.error = e.toString();
    }
  }

  async scan() {
    if (this.isScanning) return;
    this.isScanning = true;
    this.error = null;

    try {
      this.result = await invoke<ScanResult>('scan_wifi', {
        interface: this.selectedInterface || 'wlan0',
        durationSecs: this.scanDuration
      });
    } catch (e: any) {
      this.error = e.toString();
    } finally {
      this.isScanning = false;
    }
  }

  async enableMonitor() {
    if (!this.selectedInterface) return;
    this.error = null;
    try {
      const monIface = await invoke<string>('enable_monitor_mode', {
        interface: this.selectedInterface
      });
      this.selectedInterface = monIface;
      // Refresh interfaces
      this.interfaces = await invoke<WiFiInterface[]>('list_wifi_interfaces');
    } catch (e: any) {
      this.error = e.toString();
    }
  }
}

export const scannerStore = new ScannerStore();
