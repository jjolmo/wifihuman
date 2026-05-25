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
    pub scan_method: String,
    pub log: Vec<String>,
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

// --- Scan: tries tshark probe sniffing first, falls back to ARP network scan ---

#[tauri::command]
fn scan_wifi(interface: String, duration_secs: u64) -> Result<ScanResult, String> {
    let mut log = Vec::new();

    // Try tshark probe request capture first
    if let Some(tshark) = find_binary("tshark") {
        log.push(format!("Found tshark at {}", tshark));
        log.push(format!("Scanning on {} for {}s...", interface, duration_secs));

        let output = Command::new(&tshark)
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
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                log.push(format!("tshark captured {} probe requests", lines.len()));

                if !lines.is_empty() {
                    return parse_tshark_output(&text, &interface, duration_secs, log);
                }
                log.push("No probe requests captured. Interface may need monitor mode.".to_string());
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                log.push(format!("tshark failed: {}", stderr.chars().take(200).collect::<String>()));
            }
            Err(e) => {
                log.push(format!("tshark error: {}", e));
            }
        }
        log.push("Falling back to network ARP scan...".to_string());
    } else {
        log.push("tshark not found, using network ARP scan...".to_string());
    }

    // Fallback: ARP scan — detects devices on the local network
    scan_arp(log, duration_secs)
}

fn parse_tshark_output(text: &str, interface: &str, duration_secs: u64, log: Vec<String>) -> Result<ScanResult, String> {
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
        interface: interface.to_string(),
        scan_method: "probe_request".to_string(),
        log,
    })
}

// --- ARP network scan (fallback — detects devices on local network) ---

fn scan_arp(mut log: Vec<String>, _duration_secs: u64) -> Result<ScanResult, String> {
    // Run arp -a to get all devices on the local network
    let output = Command::new("arp")
        .arg("-a")
        .output()
        .map_err(|e| format!("Failed to run arp: {}", e))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut devices = Vec::new();

    for line in text.lines() {
        // Parse arp output: "hostname (IP) at MAC on interface [ifscope]"
        // or on Linux: "hostname (IP) at MAC [ether] on interface"
        let mac = extract_mac_from_arp_line(line);
        if let Some(mac) = mac {
            if mac == "ff:ff:ff:ff:ff:ff" || mac == "(incomplete)" {
                continue;
            }
            let ip = line.split('(').nth(1).and_then(|s| s.split(')').next()).unwrap_or("");
            let is_randomized = is_mac_randomized(&mac);
            let vendor = lookup_vendor(&mac);

            devices.push(DetectedDevice {
                mac,
                rssi: -50, // ARP doesn't give RSSI, use a medium value
                is_randomized,
                last_seen: now,
                ssid_probed: ip.to_string(),
                vendor,
            });
        }
    }

    let total = devices.len();
    let estimated = total; // ARP = connected devices, roughly 1 per person
    log.push(format!("ARP scan found {} devices on local network", total));

    Ok(ScanResult {
        devices,
        total_devices: total,
        estimated_humans: estimated,
        scan_duration_secs: 0,
        interface: "arp".to_string(),
        scan_method: "arp_network".to_string(),
        log,
    })
}

fn extract_mac_from_arp_line(line: &str) -> Option<String> {
    // Look for MAC pattern: xx:xx:xx:xx:xx:xx or xx-xx-xx-xx-xx-xx
    for word in line.split_whitespace() {
        let cleaned = word.replace('-', ":");
        let parts: Vec<&str> = cleaned.split(':').collect();
        if parts.len() == 6 && parts.iter().all(|p| p.len() <= 2 && u8::from_str_radix(p, 16).is_ok()) {
            return Some(cleaned.to_lowercase());
        }
    }
    None
}

// --- Install tshark ---

#[tauri::command]
fn install_tshark() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        // macOS GUI apps don't inherit shell PATH — find brew explicitly
        let brew_path = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .ok_or("Homebrew is not installed. Install it first: https://brew.sh")?;

        let output = Command::new(brew_path)
            .args(["install", "wireshark"])
            .output()
            .map_err(|e| format!("Failed to run brew: {}", e))?;
        if output.status.success() {
            // Fix BPF permissions on macOS so tshark can capture without sudo
            let _ = Command::new("sudo")
                .args(["chmod", "+r", "/dev/bpf0", "/dev/bpf1", "/dev/bpf2", "/dev/bpf3"])
                .output();
            Ok("tshark installed successfully via Homebrew".to_string())
        } else {
            Err(format!("brew install failed: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }

    #[cfg(target_os = "linux")]
    {
        let output = Command::new("sudo")
            .args(["apt-get", "install", "-y", "tshark"])
            .output()
            .or_else(|_| {
                Command::new("sudo")
                    .args(["dnf", "install", "-y", "wireshark-cli"])
                    .output()
            })
            .map_err(|e| format!("Failed to install: {}", e))?;
        if output.status.success() {
            // Add user to wireshark group
            if let Ok(user) = std::env::var("USER") {
                let _ = Command::new("sudo")
                    .args(["usermod", "-aG", "wireshark", &user])
                    .output();
            }
            Ok("tshark installed. You may need to log out and back in for group permissions.".to_string())
        } else {
            Err(format!("Install failed: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }

    #[cfg(target_os = "windows")]
    {
        Err("On Windows, download Wireshark from https://www.wireshark.org/download.html and install with tshark component enabled.".to_string())
    }
}

/// Find a binary by checking common paths (GUI apps don't inherit shell PATH)
fn find_binary(name: &str) -> Option<String> {
    // Try PATH first
    if let Ok(output) = Command::new("which").arg(name).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }
    // Check common locations
    let common_paths = [
        format!("/opt/homebrew/bin/{}", name),
        format!("/usr/local/bin/{}", name),
        format!("/usr/bin/{}", name),
        format!("/usr/sbin/{}", name),
    ];
    for p in &common_paths {
        if std::path::Path::new(p).exists() {
            return Some(p.clone());
        }
    }
    None
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
    deps.insert("tshark".to_string(), find_binary("tshark").is_some());
    // airmon-ng is only relevant on Linux — macOS uses airport for monitor mode
    #[cfg(target_os = "linux")]
    deps.insert("airmon-ng".to_string(), find_binary("airmon-ng").is_some());
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

// --- Self-update (macOS) ---

#[tauri::command]
fn run_self_update() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let script = r#"#!/bin/bash
REPO="jjolmo/wifihuman"
APP_NAME="WifiHuman.app"
INSTALL_DIR="/Applications"

RELEASE_JSON=$(curl -sL "https://api.github.com/repos/$REPO/releases/latest")
PARSED=$(echo "$RELEASE_JSON" | python3 -c "
import sys, json
data = json.load(sys.stdin)
tag = data.get('tag_name', '')
dmg_url = ''
for asset in data.get('assets', []):
    if asset['name'].endswith('.dmg'):
        dmg_url = asset['browser_download_url']
        break
print(f'{tag}|{dmg_url}')
" 2>&1)

TAG=$(echo "$PARSED" | cut -d'|' -f1)
DMG_URL=$(echo "$PARSED" | cut -d'|' -f2)
[ -z "$TAG" ] && exit 1
[ -z "$DMG_URL" ] && exit 1

TMP_DIR=$(mktemp -d)
TMP_DMG="$TMP_DIR/wifihuman.dmg"
MOUNT_POINT="$TMP_DIR/mount"

curl -L --fail -o "$TMP_DMG" "$DMG_URL" || { rm -rf "$TMP_DIR"; exit 1; }
mkdir -p "$MOUNT_POINT"
hdiutil attach "$TMP_DMG" -mountpoint "$MOUNT_POINT" -nobrowse -quiet || { rm -rf "$TMP_DIR"; exit 1; }
[ ! -d "$MOUNT_POINT/$APP_NAME" ] && { hdiutil detach "$MOUNT_POINT" 2>/dev/null; rm -rf "$TMP_DIR"; exit 1; }

osascript -e 'quit app "WifiHuman"' 2>/dev/null || true
sleep 2
pkill -f "WifiHuman" 2>/dev/null || true
sleep 1

rm -rf "$INSTALL_DIR/$APP_NAME"
cp -R "$MOUNT_POINT/$APP_NAME" "$INSTALL_DIR/$APP_NAME"
xattr -cr "$INSTALL_DIR/$APP_NAME"

hdiutil detach "$MOUNT_POINT" 2>/dev/null || true
rm -rf "$TMP_DIR"
open "$INSTALL_DIR/$APP_NAME"
"#;
        let tmp_script = std::env::temp_dir().join("wifihuman_update.sh");
        std::fs::write(&tmp_script, script).map_err(|e| e.to_string())?;
        Command::new("chmod").args(["+x", &tmp_script.to_string_lossy()]).output().map_err(|e| e.to_string())?;
        Command::new("osascript")
            .args(["-e", &format!("tell application \"Terminal\" to do script \"{}\"", tmp_script.to_string_lossy())])
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok("Update started in Terminal".to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Self-update is only supported on macOS. On Linux use your package manager or download the new AppImage/deb.".to_string())
    }
}

// --- Desktop entry (Linux) ---

#[tauri::command]
fn create_desktop_entry(app_handle: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").map_err(|e| e.to_string())?;
        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_str = exe_path.to_string_lossy().to_string();
        let icons_dir = std::path::PathBuf::from(&home).join(".local/share/icons");
        std::fs::create_dir_all(&icons_dir).map_err(|e| e.to_string())?;
        let icon_dest = icons_dir.join("wifihuman.png");
        let resource_path = app_handle.path().resource_dir().map_err(|e| e.to_string())?;
        let icon_src = resource_path.join("icons/128x128.png");
        if icon_src.exists() { std::fs::copy(&icon_src, &icon_dest).map_err(|e| e.to_string())?; }
        let apps_dir = std::path::PathBuf::from(&home).join(".local/share/applications");
        std::fs::create_dir_all(&apps_dir).map_err(|e| e.to_string())?;
        let desktop_path = apps_dir.join("wifihuman.desktop");
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=WiFi Human\nComment=WiFi-based human presence detector\n\
             Exec={}\nIcon=wifihuman\nTerminal=false\nCategories=Utility;Network;\nStartupWMClass=wifihuman\n",
            exe_str
        );
        std::fs::write(&desktop_path, &content).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&desktop_path, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        }
        Ok(desktop_path.to_string_lossy().to_string())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app_handle;
        Err("Desktop entries are only supported on Linux".to_string())
    }
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
            install_tshark,
            check_for_updates,
            run_self_update,
            create_desktop_entry,
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_title("WiFi Human").ok();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
