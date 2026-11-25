mod cluster;
mod runtime;

use cluster::{
    discovery_ipv4_prefixes, elect_new_primary, find_edge_conflicts, format_kvm_target,
    upsert_probed_peer, ClusterSnapshot, FailoverState, PeerInfo, PrimaryStatus,
    ScreenEdge,
};
use runtime::{
    launch_webapp_browser, macos_network_guidance, prompt_os_setup_wizard, resolve_binary,
    resolve_config_dir, should_open_browser, ubuntu_wayland_guidance, WizardTelemetry,
};

use edgebridge::eb_log;
use edgebridge::kvm::collector::KvmProcessManager;
use edgebridge::models::{
    now_epoch, AppState, ControlState, EdgeSide, InputConflictState, NodeRole, SignalHealth,
    KVM_PORT, WEB_PORT,
};
use edgebridge::platform;
use edgebridge::power::LidInhibitorManager;
use edgebridge::reclaim;
use edgebridge::scripts;
use edgebridge::transport;
use edgebridge::web_api::{self, WebCtx};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

fn pid_file_path() -> PathBuf { scripts::config_dir_path().join("edgebridge.pid") }

#[cfg(target_os = "linux")]
fn process_is_edgebridge(pid: i32) -> bool {
    std::fs::read_to_string(format!("/proc/{}/comm", pid))
        .map(|s| s.contains("edgebridge")).unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn process_is_edgebridge(pid: i32) -> bool {
    std::process::Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="])
        .output().ok().filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("edgebridge")).unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn process_is_edgebridge(pid: i32) -> bool {
    if pid <= 1 { return false; }
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {} -ErrorAction SilentlyContinue).ProcessName -like '*edgebridge*'", pid)])
        .output().ok().filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("true")).unwrap_or(false)
}

async fn replace_existing_instance() {
    let pid_path = pid_file_path();
    let my_pid = std::process::id() as i32;
    if let Ok(text) = std::fs::read_to_string(&pid_path) {
        if let Ok(old_pid) = text.trim().parse::<i32>() {
            if old_pid > 1 && old_pid != my_pid && process_is_edgebridge(old_pid) {
                println!("[EdgeBridge] Replacing previous instance (PID {})...", old_pid);
                #[cfg(unix)]
                unsafe { libc::kill(old_pid, libc::SIGTERM); }
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("taskkill").args(["/PID", &old_pid.to_string(), "/F"]).output();
                tokio::time::sleep(std::time::Duration::from_millis(350)).await;
            }
        }
    }
    let _ = std::fs::create_dir_all(scripts::config_dir_path());
    let _ = std::fs::write(&pid_path, my_pid.to_string());
}

#[tokio::main]
async fn main() {
    replace_existing_instance().await;

    let hostname = platform::get_hostname();
    let ip = platform::get_local_ip();
    let subnet_cidr = platform::subnet_cidr(&ip);
    let hardware = platform::get_hardware_specs();
    let firewall = platform::check_firewall_status().await;
    let initial_displays = platform::detect_display_layout().await;
    let initial_fp = platform::detect_lan_mouse_fingerprint().await;
    let node_id = format!("{}-{}", hostname, now_epoch() % 100_000);

    let mut power_mgr = LidInhibitorManager::new();
    let is_laptop = power_mgr.is_laptop;
    let mut lid_active = false;
    let lid_msg = if is_laptop {
        match power_mgr.enable().await { Ok(m) => { lid_active = true; m } Err(e) => format!("Awake lock failed: {}", e) }
    } else { "Desktop detected - awake lock on standby.".into() };

    println!("\n================================================================");
    println!("  EdgeBridge v1.0.0-rc.1 - Multi-Platform Multi-Peer Rust LAN KVM");
    println!("================================================================");
    println!("  [OK] Mode      : PASSIVE LISTENING");
    println!("  [OK] Machine   : {} ({}) | IP: {} ({})", hostname, hardware.device_class, ip, subnet_cidr);
    println!("  [OK] Displays  : {}", scripts::format_display_summary(&initial_displays));
    println!("  [OK] AwakeLock : {}", lid_msg);
    println!("  [OK] Ports     : TCP {} (Web/Probe) | UDP {} (KVM)", WEB_PORT, KVM_PORT);
    println!("  [OK] Web-app   : http://127.0.0.1:{}", WEB_PORT);
    println!("================================================================\n");

    let mut st = AppState {
        node_id, hostname, ip, subnet_cidr, hardware, firewall,
        is_laptop, lid_inhibit_active: lid_active, fingerprint: initial_fp,
        displays: initial_displays, role: NodeRole::Listening, probe_in_progress: false,
        last_probe_summary: "Listening quietly - click Probe Network on Primary.".into(),
        discovered: Vec::new(), incoming_probe: None, active_peer: None,
        peers: BTreeMap::new(), selected_peer_ip: None,
        edge: EdgeSide::Right, align_offset_pct: 0, layout_synced: false,
        peer_kvm_running: false, peer_portal_ready: false, peer_reachable: false,
        signal: SignalHealth::default(), control_state: ControlState::Local,
        conflict: InputConflictState::default(), input_reclaim_ready: false,
        watched_devices: 0, logs: Vec::new(),
    };
    eb_log!(st, "Listening on TCP {} ({}).", WEB_PORT, st.subnet_cidr);
    let _ = scripts::write_config_files(&st);

    let state = Arc::new(RwLock::new(st));
    let kvm = Arc::new(Mutex::new(KvmProcessManager::new()));
    let power = Arc::new(Mutex::new(power_mgr));

    platform::spawn_display_watcher(state.clone());
    transport::lan_probe::spawn_peer_health_monitor(state.clone());
    reclaim::spawn_input_reclaim_watcher(state.clone());

    let ctx = WebCtx { state: state.clone(), kvm: kvm.clone(), power: power.clone() };
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", WEB_PORT)).await.expect("Failed binding port 48720");

    let kvm_sd = kvm.clone();
    let pwr_sd = power.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        println!("\n[EdgeBridge] Caught Ctrl+C - stopping KVM & awake lock...");
        println!("[EdgeBridge] {}", kvm_sd.lock().await.stop().await);
        println!("[EdgeBridge] {}", pwr_sd.lock().await.disable().await);
        std::process::exit(0);
    });

    axum::serve(listener, web_api::build_router(ctx)).await.expect("Webapp error");
}

#[allow(dead_code)]
fn init_release2_runtime(cli_open_ui: bool, webapp_url: Option<&str>) -> (WizardTelemetry, FailoverState) {
    let config_dir = resolve_config_dir();
    let _ = std::fs::create_dir_all(&config_dir);
    let mut wizard = WizardTelemetry::default();
    let failover = FailoverState::new(1, String::new(), 0, 15_000);
    if let Some(url) = webapp_url {
        launch_webapp_browser(url, cli_open_ui);
    }
    (wizard, failover)
}
