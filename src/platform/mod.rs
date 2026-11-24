#[cfg(target_os = "linux")]
pub mod linux_wayland;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use std::net::UdpSocket;
use std::path::PathBuf;
use tokio::process::Command;

#[cfg(target_os = "linux")]
pub use linux_wayland::{
    check_firewall_status, detect_display_layout, get_hardware_specs, get_hostname,
    os_permission_notice, send_notification, spawn_display_watcher,
};

#[cfg(target_os = "macos")]
pub use macos::{
    check_firewall_status, detect_display_layout, get_hardware_specs, get_hostname,
    os_permission_notice, send_notification, spawn_display_watcher,
};

#[cfg(target_os = "windows")]
pub use windows::{
    check_firewall_status, detect_display_layout, get_hardware_specs, get_hostname,
    os_permission_notice, send_notification, spawn_display_watcher,
};

pub fn get_local_ip() -> String {
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = sock.local_addr() { return addr.ip().to_string(); }
        }
    }
    "127.0.0.1".to_string()
}

/// Derives IPv4 /24 subnet CIDR for LAN probing.
///
/// ```
/// use edgebridge::platform::subnet_cidr;
/// assert_eq!(subnet_cidr("10.213.54.75"), "10.213.54.0/24");
/// assert_eq!(subnet_cidr("bad"), "unknown");
/// ```
pub fn subnet_cidr(ip: &str) -> String {
    let p: Vec<&str> = ip.split('.').collect();
    if p.len() == 4 { format!("{}.{}.{}.0/24", p[0], p[1], p[2]) } else { "unknown".into() }
}

pub async fn detect_lan_mouse_fingerprint() -> Option<String> {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()?;
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from(&home).join(".config/lan-mouse/lan-mouse.pem"),
        PathBuf::from(&home).join(".local/share/lan-mouse/lan-mouse.pem"),
        PathBuf::from(&home).join("Library/Application Support/lan-mouse/lan-mouse.pem"),
        PathBuf::from(&home).join("Library/Application Support/de.feschber.LanMouse/lan-mouse.pem"),
    ];
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidates.push(PathBuf::from(&appdata).join("lan-mouse/lan-mouse.pem"));
        candidates.push(PathBuf::from(&appdata).join("de.feschber.LanMouse/lan-mouse.pem"));
    }
    for path in candidates {
        if path.exists() {
            if let Ok(out) = Command::new("openssl")
                .args(["x509", "-noout", "-fingerprint", "-sha256", "-in"])
                .arg(&path).output().await
            {
                if out.status.success() {
                    let txt = String::from_utf8_lossy(&out.stdout);
                    if let Some((_, fp)) = txt.split_once('=') {
                        let c = fp.trim().to_lowercase();
                        if !c.is_empty() { return Some(c); }
                    }
                }
            }
        }
    }
    None
}
