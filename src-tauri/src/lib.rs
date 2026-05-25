use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use tauri::{Manager, State};

#[derive(Debug, Default)]
pub struct AppState {
    pub settings: Mutex<HashMap<String, String>>,
}

#[tauri::command]
fn get_setting(key: String, state: State<'_, Arc<AppState>>) -> Option<String> {
    state.settings.lock().get(&key).cloned()
}

#[tauri::command]
fn set_setting(key: String, value: String, state: State<'_, Arc<AppState>>) {
    state.settings.lock().insert(key, value);
}

// --- WiFi types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedDevice {
    pub mac: String,
    pub rssi: i32,
    pub is_randomized: bool,
    pub last_seen: u64, // epoch seconds
    pub ssid_probed: String,
    pub vendor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub devices: Vec<DetectedDevice>,
    pub total_devices: usize,
    pub estimated_humans: usize,
    pub scan_duration_secs: u64,
    pub interface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WiFiInterface {
    pub name: String,
    pub mode: String,
}

// --- Detect available WiFi interfaces ---

#[tauri::command]
fn list_wifi_interfaces() -> Result<Vec<WiFiInterface>, String> {
    #[cfg(target_os = "linux")]
    {
        let output = Command::new("iw")
            .args(["dev"])
            .output()
            .map_err(|e| format!("Failed to run iw: {}. Is iw installed?", e))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut interfaces = Vec::new();
        let mut current_name = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Interface ") {
                current_name = trimmed.strip_prefix("Interface ").unwrap_or("").to_string();
            }
            if trimmed.starts_with("type ") && !current_name.is_empty() {
                let mode = trimmed.strip_prefix("type ").unwrap_or("unknown").to_string();
                interfaces.push(WiFiInterface {
                    name: current_name.clone(),
                    mode,
                });
            }
        }
        Ok(interfaces)
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: list the main WiFi interface
        let output = Command::new("networksetup")
            .args(["-listallhardwareports"])
            .output()
            .map_err(|e| format!("Failed: {}", e))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut interfaces = Vec::new();
        let mut is_wifi = false;
        for line in text.lines() {
            if line.contains("Wi-Fi") || line.contains("AirPort") {
                is_wifi = true;
            } else if line.starts_with("Device:") && is_wifi {
                let name = line.strip_prefix("Device: ").unwrap_or("").trim().to_string();
                interfaces.push(WiFiInterface {
                    name,
                    mode: "managed".to_string(),
                });
                is_wifi = false;
            } else if line.starts_with("Hardware Port:") {
                is_wifi = false;
            }
        }
        Ok(interfaces)
    }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("netsh")
            .args(["wlan", "show", "interfaces"])
            .output()
            .map_err(|e| format!("Failed: {}", e))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut interfaces = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Name") {
                if let Some(name) = trimmed.split(':').nth(1) {
                    interfaces.push(WiFiInterface {
                        name: name.trim().to_string(),
                        mode: "managed".to_string(),
                    });
                }
            }
        }
        Ok(interfaces)
    }
}

// --- Enable monitor mode (Linux only, requires sudo) ---

#[tauri::command]
fn enable_monitor_mode(interface: String) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        // Try airmon-ng first
        let _ = Command::new("sudo")
            .args(["airmon-ng", "check", "kill"])
            .output();

        let output = Command::new("sudo")
            .args(["airmon-ng", "start", &interface])
            .output()
            .map_err(|e| format!("Failed to start monitor mode: {}", e))?;

        let text = String::from_utf8_lossy(&output.stdout).to_string();

        // Detect the monitor interface name (usually wlan0mon or similar)
        let mon_iface = if text.contains("mon") {
            format!("{}mon", interface)
        } else {
            // Fallback: try manual method
            let _ = Command::new("sudo")
                .args(["ip", "link", "set", &interface, "down"])
                .output();
            let _ = Command::new("sudo")
                .args(["iw", "dev", &interface, "set", "type", "monitor"])
                .output();
            let _ = Command::new("sudo")
                .args(["ip", "link", "set", &interface, "up"])
                .output();
            interface.clone()
        };

        Ok(mon_iface)
    }

    #[cfg(target_os = "macos")]
    {
        // macOS can do monitor mode via airport or native API but doesn't need airmon-ng
        Ok(interface)
    }

    #[cfg(target_os = "windows")]
    {
        let _ = interface;
        Err("Monitor mode is not easily supported on Windows. Use a Linux VM or compatible USB adapter.".to_string())
    }
}

// --- Scan for probe requests using tshark ---

#[tauri::command]
fn scan_wifi(interface: String, duration_secs: u64) -> Result<ScanResult, String> {
    // Use tshark to capture probe requests
    // Check if tshark exists first
    let tshark_check = Command::new("which").arg("tshark").output();
    if tshark_check.map(|o| !o.status.success()).unwrap_or(true) {
        return scan_wifi_simulated(duration_secs);
    }

    // Try without sudo first (macOS dumpcap may have permissions), then with sudo
    let output = Command::new("tshark")
        .args([
            "-i", &interface,
            "-a", &format!("duration:{}", duration_secs),
            "-Y", "wlan.fc.type_subtype == 0x04",
            "-T", "fields",
            "-e", "wlan.sa",
            "-e", "wlan_radio.signal_dbm",
            "-e", "wlan.ssid",
            "-E", "separator=|",
        ])
        .output()
        .or_else(|_| {
            Command::new("sudo")
                .args([
                    "tshark",
                    "-i", &interface,
                    "-a", &format!("duration:{}", duration_secs),
                    "-Y", "wlan.fc.type_subtype == 0x04",
                    "-T", "fields",
                    "-e", "wlan.sa",
                    "-e", "wlan_radio.signal_dbm",
                    "-e", "wlan.ssid",
                    "-E", "separator=|",
                ])
                .output()
        })
        .map_err(|e| format!("Failed to run tshark: {}", e))?;

    if !output.status.success() {
        // Fallback to simulated data on any error
        return scan_wifi_simulated(duration_secs);
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut devices_map: HashMap<String, DetectedDevice> = HashMap::new();

    for line in text.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.is_empty() || parts[0].is_empty() {
            continue;
        }

        let mac = parts[0].trim().to_lowercase();
        let rssi = parts.get(1)
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(-80);
        let ssid = parts.get(2).unwrap_or(&"").trim().to_string();

        let is_randomized = is_mac_randomized(&mac);
        let vendor = lookup_vendor(&mac);

        devices_map.entry(mac.clone()).or_insert_with(|| DetectedDevice {
            mac,
            rssi,
            is_randomized,
            last_seen: now,
            ssid_probed: ssid,
            vendor,
        });
    }

    let devices: Vec<DetectedDevice> = devices_map.into_values().collect();
    let total = devices.len();
    // Estimate: non-random MACs = 1 person each, random MACs grouped ~= 70% are unique people
    let non_random = devices.iter().filter(|d| !d.is_randomized).count();
    let random = devices.iter().filter(|d| d.is_randomized).count();
    let estimated = non_random + (random as f64 * 0.7).ceil() as usize;

    Ok(ScanResult {
        devices,
        total_devices: total,
        estimated_humans: estimated,
        scan_duration_secs: duration_secs,
        interface,
    })
}

// --- Simulated scan for demo/testing when no monitor mode available ---

fn scan_wifi_simulated(duration_secs: u64) -> Result<ScanResult, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let seed = now;

    // Generate 5-15 simulated devices
    let count = 5 + (seed % 11) as usize;
    let mut devices = Vec::new();

    let vendors = ["Apple", "Samsung", "Google", "Xiaomi", "OnePlus", "Huawei", "Sony", "LG", "Motorola", "Nokia", "OPPO", "Realme"];
    let ssids = ["HomeWiFi", "Starbucks", "Airport_Free", "eduroam", "", "AndroidAP", "iPhone", "DIRECT-xx", "FreeWiFi", ""];

    for i in 0..count {
        let is_random = (seed + i as u64) % 3 != 0; // ~66% random
        let mac = if is_random {
            // Random MAC: locally-administered bit set
            format!(
                "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                0x02 | ((seed + i as u64 * 7) % 256) as u8,
                ((seed + i as u64 * 13) % 256) as u8,
                ((seed + i as u64 * 23) % 256) as u8,
                ((seed + i as u64 * 37) % 256) as u8,
                ((seed + i as u64 * 47) % 256) as u8,
                ((seed + i as u64 * 59) % 256) as u8,
            )
        } else {
            // "Real" MAC from known vendor
            let prefixes = ["00:1a:2b", "ac:de:48", "f0:18:98", "34:ab:37", "d4:f5:47"];
            let prefix = prefixes[i % prefixes.len()];
            format!(
                "{}:{:02x}:{:02x}:{:02x}",
                prefix,
                ((seed + i as u64 * 31) % 256) as u8,
                ((seed + i as u64 * 41) % 256) as u8,
                ((seed + i as u64 * 53) % 256) as u8,
            )
        };

        let rssi = -30 - ((seed + i as u64 * 17) % 50) as i32;
        let vendor = if is_random {
            "Unknown".to_string()
        } else {
            vendors[i % vendors.len()].to_string()
        };
        let ssid = ssids[(seed as usize + i) % ssids.len()].to_string();

        devices.push(DetectedDevice {
            mac,
            rssi,
            is_randomized: is_random,
            last_seen: now - (i as u64 * 3),
            ssid_probed: ssid,
            vendor,
        });
    }

    let total = devices.len();
    let non_random = devices.iter().filter(|d| !d.is_randomized).count();
    let random = devices.iter().filter(|d| d.is_randomized).count();
    let estimated = non_random + (random as f64 * 0.7).ceil() as usize;

    // Simulate scan time
    std::thread::sleep(std::time::Duration::from_secs(duration_secs.min(3)));

    Ok(ScanResult {
        devices,
        total_devices: total,
        estimated_humans: estimated,
        scan_duration_secs: duration_secs,
        interface: "simulated".to_string(),
    })
}

fn is_mac_randomized(mac: &str) -> bool {
    // A MAC is randomized if the locally-administered bit (bit 1 of first octet) is set
    if let Some(first_byte_str) = mac.split(':').next() {
        if let Ok(byte) = u8::from_str_radix(first_byte_str, 16) {
            return byte & 0x02 != 0;
        }
    }
    false
}

fn lookup_vendor(mac: &str) -> String {
    let prefix = mac.split(':').take(3).collect::<Vec<_>>().join(":");
    let vendor = match prefix.as_str() {
        p if p.starts_with("00:1a:2b") => "Ayecom",
        p if p.starts_with("ac:de:48") => "Apple",
        p if p.starts_with("f0:18:98") => "Apple",
        p if p.starts_with("34:ab:37") => "Apple",
        p if p.starts_with("d4:f5:47") => "Google",
        p if p.starts_with("8c:85:90") => "Samsung",
        p if p.starts_with("fc:a1:83") => "Samsung",
        p if p.starts_with("b4:ce:f6") => "HTC",
        _ => "Unknown",
    };
    vendor.to_string()
}

// --- Check if tshark is installed ---

#[tauri::command]
fn check_dependencies() -> Result<HashMap<String, bool>, String> {
    let mut deps = HashMap::new();

    let tshark = Command::new("which").arg("tshark").output().map(|o| o.status.success()).unwrap_or(false);
    deps.insert("tshark".to_string(), tshark);

    let airmon = Command::new("which").arg("airmon-ng").output().map(|o| o.status.success()).unwrap_or(false);
    deps.insert("airmon-ng".to_string(), airmon);

    Ok(deps)
}

// --- Update checker ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
}

#[tauri::command]
fn check_for_updates() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION");
    let url = "https://api.github.com/repos/jjolmo/wifihuman/releases/latest";
    let output = Command::new("curl")
        .args(["-sL", "-H", "Accept: application/vnd.github.v3+json", url])
        .output()
        .map_err(|e| e.to_string())?;
    let body = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("Failed to parse: {}", e))?;
    let tag = json["tag_name"].as_str().unwrap_or("v0.0.0");
    let latest = tag.trim_start_matches('v');
    let release_url = json["html_url"].as_str().unwrap_or("").to_string();
    Ok(UpdateInfo {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        has_update: latest != current,
        release_url,
    })
}

pub fn run() {
    let state = Arc::new(AppState::default());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_setting,
            set_setting,
            list_wifi_interfaces,
            enable_monitor_mode,
            scan_wifi,
            check_dependencies,
            check_for_updates,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_title("WiFi Human").ok();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
