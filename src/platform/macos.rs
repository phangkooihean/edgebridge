use crate::models::{AppState, DisplayLayout, FirewallStatus, HardwareSpecs, MonitorInfo, KVM_PORT, WEB_PORT};
use crate::platform::{detect_lan_mouse_fingerprint, get_local_ip, subnet_cidr};
use crate::power::is_laptop;
use crate::scripts::write_config_files;
use serde_json::Value;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;

pub fn get_hostname() -> String {
    std::process::Command::new("scutil").args(["--get", "ComputerName"]).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty()).unwrap_or_else(|| "macos-box".into())
}

pub fn get_hardware_specs() -> HardwareSpecs {
    let ver = std::process::Command::new("sw_vers").arg("-productVersion").output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_else(|| "macOS".into());
    HardwareSpecs {
        device_class: if is_laptop() { "macOS MacBook" } else { "macOS Desktop" }.into(),
        model: "Mac".into(), cpu: "Apple/Intel".into(), memory_gb: "RAM".into(),
        os_pretty: format!("macOS {}", ver), desktop_env: "Aqua".into(),
    }
}

pub async fn check_firewall_status() -> FirewallStatus {
    FirewallStatus { ufw_active: false, summary: format!("macOS LAN TCP {} / UDP {}", WEB_PORT, KVM_PORT) }
}

pub fn os_permission_notice(both: bool, step3: bool) -> String {
    if both { "OS Good-Citizen Status: macOS Accessibility & peer input backends active.".into() }
    else if step3 { "Heads-up: Enable Terminal & lan-mouse under System Settings -> Privacy & Security -> Accessibility (use Cmd+Shift+. for ~/.cargo).".into() }
    else { "OS Policy: macOS requires Accessibility permission at Step 3.".into() }
}

pub fn send_notification(title: &str, body: &str) {
    let s = format!("display notification \"{}\" with title \"{}\"", body.replace('"', "'"), title.replace('"', "'"));
    let _ = tokio::process::Command::new("osascript").args(["-e", &s]).spawn();
}

pub async fn detect_display_layout() -> DisplayLayout {
    let Ok(out) = Command::new("system_profiler").args(["SPDisplaysDataType", "-json"]).output().await else { return DisplayLayout::default(); };
    let Ok(root) = serde_json::from_slice::<Value>(&out.stdout) else { return DisplayLayout::default(); };
    let mut mons = Vec::new();
    if let Some(gpus) = root.get("SPDisplaysDataType").and_then(|v| v.as_array()) {
        for d in gpus.iter().flat_map(|g| g.get("spdisplays_ndrvs").and_then(|v| v.as_array()).into_iter().flatten()) {
            let name = d.get("_name").and_then(|v| v.as_str()).unwrap_or("Mac Display").to_string();
            let nums: Vec<u32> = d.get("_spdisplays_resolution").or_else(|| d.get("_spdisplays_pixels")).and_then(|v| v.as_str()).unwrap_or("1440 x 900").split(|c: char| !c.is_ascii_digit()).filter_map(|t| t.parse().ok()).collect();
            let (w, h) = (nums.first().copied().unwrap_or(1440).max(1), nums.get(1).copied().unwrap_or(900).max(1));
            mons.push(MonitorInfo { connector: name, is_primary: mons.is_empty(), x: 0, y: 0, physical_width: w, physical_height: h, logical_width: w, logical_height: h, scale: 1.0, refresh_rate: 60.0, transform: 0, orientation: "Landscape".into() });
        }
    }
    if mons.is_empty() { return DisplayLayout::default(); }
    let tw = mons.iter().map(|m| m.logical_width).max().unwrap_or(1440);
    let th = mons.iter().map(|m| m.logical_height).max().unwrap_or(900);
    DisplayLayout { monitors: mons, display_mode: "Active".into(), total_logical_width: tw, total_logical_height: th }
}

pub fn spawn_display_watcher(state: Arc<RwLock<AppState>>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(4));
        loop {
            tick.tick().await;
            let (layout, fp, ip) = (detect_display_layout().await, detect_lan_mouse_fingerprint().await, get_local_ip());
            let mut g = state.write().await;
            if g.displays != layout { g.displays = layout; let _ = write_config_files(&g); }
            if g.fingerprint.is_none() && fp.is_some() { g.fingerprint = fp; }
            if ip != "127.0.0.1" && ip != g.ip { g.subnet_cidr = subnet_cidr(&ip); g.ip = ip; }
        }
    });
}
