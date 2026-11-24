use crate::models::{AppState, DisplayLayout, FirewallStatus, HardwareSpecs, MonitorInfo, KVM_PORT, WEB_PORT};
use crate::platform::{detect_lan_mouse_fingerprint, get_local_ip, subnet_cidr};
use crate::power::is_laptop;
use crate::scripts::write_config_files;
use std::sync::Arc;
use tokio::sync::RwLock;
use windows_sys::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW};

pub fn get_hostname() -> String { std::env::var("COMPUTERNAME").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "windows-pc".into()) }
pub fn get_hardware_specs() -> HardwareSpecs {
    let mut kb: u64 = 0;
    unsafe { windows_sys::Win32::System::SystemInformation::GetPhysicallyInstalledSystemMemory(&mut kb); }
    let memory_gb = if kb > 0 { format!("{:.1} GiB", (kb as f64) / 1_048_576.0) } else { "Unknown RAM".into() };
    HardwareSpecs { device_class: if is_laptop() { "Windows Laptop" } else { "Windows Desktop" }.into(), model: "Windows PC".into(), cpu: std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "x86_64 CPU".into()), memory_gb, os_pretty: "Windows 10/11".into(), desktop_env: "Win32".into() }
}
pub async fn check_firewall_status() -> FirewallStatus { FirewallStatus { ufw_active: false, summary: format!("Windows LAN TCP {} / UDP {}", WEB_PORT, KVM_PORT) } }
pub fn os_permission_notice(both: bool, step3: bool) -> String {
    if both { "OS Good-Citizen Status: Windows input injection & peer input active.".into() }
    else if step3 { "Heads-up: Start lan-mouse.exe at Step 3 and allow Windows Firewall if prompted.".into() }
    else { "OS Policy: EdgeBridge starts Windows KVM cleanly at Step 3.".into() }
}
pub fn send_notification(_title: &str, _body: &str) {}

unsafe extern "system" fn mon_enum_proc(hmon: HMONITOR, _hdc: HDC, _rc: *mut RECT, data: LPARAM) -> BOOL {
    let list = &mut *(data as *mut Vec<MonitorInfo>);
    let mut info: MONITORINFOEXW = std::mem::zeroed();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hmon as _, &mut info as *mut _ as _) != 0 {
        let r = info.monitorInfo.rcMonitor;
        let (w, h) = ((r.right - r.left).max(1) as u32, (r.bottom - r.top).max(1) as u32);
        list.push(MonitorInfo { connector: "DISPLAY".into(), is_primary: (info.monitorInfo.dwFlags & 1) != 0, x: r.left, y: r.top, physical_width: w, physical_height: h, logical_width: w, logical_height: h, scale: 1.0, refresh_rate: 60.0, transform: 0, orientation: if h > w { "Portrait".into() } else { "Landscape".into() } });
    }
    1
}
pub async fn detect_display_layout() -> DisplayLayout {
    let mut mons = Vec::new();
    unsafe { EnumDisplayMonitors(0 as _, std::ptr::null(), Some(mon_enum_proc), &mut mons as *mut _ as LPARAM); }
    if mons.is_empty() { return DisplayLayout::default(); }
    let tw = mons.iter().map(|m| m.x.max(0) as u32 + m.logical_width).max().unwrap_or(1920);
    let th = mons.iter().map(|m| m.y.max(0) as u32 + m.logical_height).max().unwrap_or(1080);
    DisplayLayout { monitors: mons, display_mode: "Active".into(), total_logical_width: tw, total_logical_height: th }
}
pub fn spawn_display_watcher(state: Arc<RwLock<AppState>>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3));
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
