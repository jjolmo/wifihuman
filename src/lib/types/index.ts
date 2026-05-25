export interface DetectedDevice {
  mac: string;
  rssi: number;
  is_randomized: boolean;
  last_seen: number;
  ssid_probed: string;
  vendor: string;
}

export interface ScanResult {
  devices: DetectedDevice[];
  total_devices: number;
  estimated_humans: number;
  scan_duration_secs: number;
  interface: string;
}

export interface WiFiInterface {
  name: string;
  mode: string;
}

export interface RadarDot {
  x: number;
  y: number;
  device: DetectedDevice;
  angle: number;
  distance: number; // 0-1 normalized
}
