use crate::models::{AppState, DisplayLayout, FirewallStatus, HardwareSpecs, MonitorInfo, KVM_PORT, WEB_PORT};
use crate::platform::{detect_lan_mouse_fingerprint, get_local_ip, subnet_cidr};
use crate::power::is_laptop;
use crate::scripts::write_config_files;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;

pub fn get_hostname() -> String { std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "linux-box".into()) }
pub fn get_hardware_specs() -> HardwareSpecs {
    let os_pretty = std::fs::read_to_string("/etc/os-release").ok().and_then(|c| c.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|r| r.trim_matches('"').to_string()))).unwrap_or_else(|| "Linux".into());
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "GNOME".into());
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "wayland".into());
    HardwareSpecs { device_class: if is_laptop() { "Linux Laptop" } else { "Linux Desktop" }.into(), model: "Linux PC".into(), cpu: "CPU".into(), memory_gb: "RAM".into(), os_pretty, desktop_env: format!("{} ({})", desktop, session) }
}
pub async fn check_firewall_status() -> FirewallStatus {
    let ufw = std::fs::read_to_string("/etc/ufw/ufw.conf").map(|c| c.lines().any(|l| l.trim() == "ENABLED=yes")).unwrap_or(false);
    FirewallStatus { ufw_active: ufw, summary: if ufw { format!("UFW ENABLED - allow {}/tcp & {}/udp", WEB_PORT, KVM_PORT) } else { format!("TCP {} & UDP {} ready", WEB_PORT, KVM_PORT) } }
}
pub fn os_permission_notice(both: bool, step3: bool) -> String {
    if both { "OS Good-Citizen Status: Wayland InputCapture & libei active.".into() }
    else if step3 { "Heads-up: Toggle 'Allow Remote Input Capturing' ON and click 'Share' in GNOME's dialog.".into() }
    else { "OS Policy: EdgeBridge requests Wayland input permission at Step 3.".into() }
}
pub fn send_notification(title: &str, body: &str) { let _ = tokio::process::Command::new("notify-send").args([title, body]).spawn(); }

pub async fn detect_display_layout() -> DisplayLayout {
    let Ok(out) = Command::new("busctl").args(["--user", "call", "org.gnome.Mutter.DisplayConfig", "/org/gnome/Mutter/DisplayConfig", "org.gnome.Mutter.DisplayConfig", "GetCurrentState", "--json=short"]).output().await else { return DisplayLayout::default(); };
    if !out.status.success() { return DisplayLayout::default(); }
    let Some(data) = serde_json::from_slice::<Value>(&out.stdout).ok().and_then(|r| r.get("data")?.as_array().cloned()) else { return DisplayLayout::default(); };
    let (Some(phys), Some(logical)) = (data.get(1).and_then(|v| v.as_array()), data.get(2).and_then(|v| v.as_array())) else { return DisplayLayout::default(); };
    let mut pmap: HashMap<String, (u32, u32, f64)> = HashMap::new();
    for m in phys {
        let c = m.get(0).and_then(|s| s.get(0)).and_then(|v| v.as_str()).unwrap_or("Display").to_string();
        if let Some(md) = m.get(1).and_then(|v| v.as_array()).and_then(|ms| ms.iter().find(|x| x.get(6).and_then(|p| p.get("is-current")).and_then(|v| v.get("data")).and_then(|b| b.as_bool()) == Some(true)).or_else(|| ms.first())) {
            pmap.insert(c, (md.get(1).and_then(|v| v.as_u64()).unwrap_or(1920) as u32, md.get(2).and_then(|v| v.as_u64()).unwrap_or(1080) as u32, (md.get(3).and_then(|v| v.as_f64()).unwrap_or(60.0) * 100.0).round() / 100.0));
        }
    }
    let mut res = Vec::new();
    for lm in logical {
        let (x, y) = (lm.get(0).and_then(|v| v.as_i64()).unwrap_or(0) as i32, lm.get(1).and_then(|v| v.as_i64()).unwrap_or(0) as i32);
        let sc = lm.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0).max(0.5);
        let tr = lm.get(3).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let prim = lm.get(4).and_then(|v| v.as_bool()).unwrap_or(false);
        if let Some(conn) = lm.get(5).and_then(|v| v.as_array()).and_then(|a| a.first()).and_then(|s| s.get(0)).and_then(|c| c.as_str()) {
            let (pw, ph, hz) = pmap.get(conn).copied().unwrap_or((1920, 1080, 60.0));
            let port = matches!(tr, 1 | 3 | 5 | 7);
            let (ew, eh) = if port { (ph, pw) } else { (pw, ph) };
            res.push(MonitorInfo { connector: conn.into(), is_primary: prim, x, y, physical_width: pw, physical_height: ph, logical_width: (ew as f64 / sc).round() as u32, logical_height: (eh as f64 / sc).round() as u32, scale: sc, refresh_rate: hz, transform: tr, orientation: if port { "Portrait".into() } else { "Landscape".into() } });
        }
    }
    if res.is_empty() { return DisplayLayout::default(); }
    let tw = res.iter().map(|m| (m.x.max(0) as u32) + m.logical_width).max().unwrap_or(1920);
    let th = res.iter().map(|m| (m.y.max(0) as u32) + m.logical_height).max().unwrap_or(1080);
    DisplayLayout { monitors: res, display_mode: "Active".into(), total_logical_width: tw, total_logical_height: th }
}
pub fn spawn_display_watcher(state: Arc<RwLock<AppState>>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
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
