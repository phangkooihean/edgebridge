use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const WEB_PORT: u16 = 48720; // Embedded Web UI, Subnet Probe & Peer/Telemetry API
pub const KVM_PORT: u16 = 4242;  // Encrypted DTLS KVM transport (lan-mouse)

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole { Listening, Primary, Guest }

impl NodeRole {
    /// Returns the display/telemetry label for each node role.
    ///
    /// ```
    /// use edgebridge::models::NodeRole;
    /// assert_eq!(NodeRole::Listening.label(), "LISTENING");
    /// assert_eq!(NodeRole::Primary.label(), "PRIMARY (HOST)");
    /// assert_eq!(NodeRole::Guest.label(), "GUEST (SECONDARY)");
    /// ```
    pub fn label(self) -> &'static str {
        match self {
            NodeRole::Listening => "LISTENING",
            NodeRole::Primary => "PRIMARY (HOST)",
            NodeRole::Guest => "GUEST (SECONDARY)",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StepColor { Grey, Yellow, Green }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WizardStep { pub num: u8, pub title: String, pub color: StepColor, pub detail: String }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HardwareSpecs {
    pub device_class: String, pub model: String, pub cpu: String,
    pub memory_gb: String, pub os_pretty: String, pub desktop_env: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FirewallStatus { pub ufw_active: bool, pub summary: String }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonitorInfo {
    pub connector: String, pub is_primary: bool, pub x: i32, pub y: i32,
    pub physical_width: u32, pub physical_height: u32,
    pub logical_width: u32, pub logical_height: u32,
    pub scale: f64, pub refresh_rate: f64, pub transform: u32, pub orientation: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DisplayLayout {
    pub monitors: Vec<MonitorInfo>, pub display_mode: String,
    pub total_logical_width: u32, pub total_logical_height: u32,
}

impl Default for DisplayLayout {
    fn default() -> Self {
        Self {
            monitors: vec![MonitorInfo {
                connector: "Display-1".into(), is_primary: true, x: 0, y: 0,
                physical_width: 1920, physical_height: 1080,
                logical_width: 1920, logical_height: 1080,
                scale: 1.0, refresh_rate: 60.0, transform: 0, orientation: "Landscape".into(),
            }],
            display_mode: "Single".into(), total_logical_width: 1920, total_logical_height: 1080,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EdgeSide { Right, Left, Top, Bottom }

impl EdgeSide {
    /// Involution invariant: mirroring any edge twice restores the original edge.
    ///
    /// ```
    /// use edgebridge::models::EdgeSide;
    /// for s in [EdgeSide::Right, EdgeSide::Left, EdgeSide::Top, EdgeSide::Bottom] {
    ///     assert_eq!(s.opposite().opposite(), s);
    /// }
    /// ```
    pub fn opposite(self) -> Self {
        match self {
            EdgeSide::Right => EdgeSide::Left, EdgeSide::Left => EdgeSide::Right,
            EdgeSide::Top => EdgeSide::Bottom, EdgeSide::Bottom => EdgeSide::Top,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeSide::Right => "right", EdgeSide::Left => "left",
            EdgeSide::Top => "top", EdgeSide::Bottom => "bottom",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ControlState {
    Local,
    ControllingPeer { peer_hostname: String, peer_ip: String },
    RemoteControlled { controller_hostname: String, controller_ip: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalHealth {
    pub transport: String, pub status: String, pub score_pct: u8,
    pub avg_ping_ms: u64, pub jitter_ms: u64, pub packet_loss_pct: u8,
    pub consecutive_misses: u32, pub est_kbps: f64, pub diagnosis: String,
    pub sample_count: usize, pub responsiveness_rating: String,
}

impl Default for SignalHealth {
    fn default() -> Self {
        Self {
            transport: "Wi-Fi / LAN TCP+DTLS".into(), status: "IDLE".into(),
            score_pct: 0, avg_ping_ms: 0, jitter_ms: 0, packet_loss_pct: 0,
            consecutive_misses: 0, est_kbps: 0.0,
            diagnosis: "No peer paired yet - click Probe Network on Primary.".into(),
            sample_count: 0, responsiveness_rating: "STANDBY".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeRequest {
    pub from_node_id: String, pub from_hostname: String, pub from_ip: String,
    pub from_hardware: HardwareSpecs, pub from_displays: DisplayLayout,
    pub from_fingerprint: Option<String>, pub is_laptop: bool,
    pub lid_inhibit_active: bool, pub received_epoch: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredHost {
    pub node_id: String, pub hostname: String, pub ip: String,
    pub hardware: HardwareSpecs, pub is_laptop: bool, pub lid_inhibit_active: bool,
    pub fingerprint: Option<String>, pub displays: DisplayLayout,
    pub role: NodeRole, pub ping_ms: Option<u64>, pub accepted: bool,
    #[serde(default = "default_edge_right")] pub edge: EdgeSide,
    #[serde(default)] pub align_offset_pct: i32,
    #[serde(default)] pub kvm_running: bool,
    #[serde(default)] pub portal_ready: bool,
    #[serde(default = "default_true")] pub reachable: bool,
    #[serde(default)] pub remote_telemetry: Option<String>,
}

fn default_edge_right() -> EdgeSide { EdgeSide::Right }
fn default_true() -> bool { true }

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct InputConflictState {
    pub active: bool, pub last_host_input_epoch: u64,
    pub last_guest_input_epoch: u64, pub summary: String,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub node_id: String, pub hostname: String, pub ip: String, pub subnet_cidr: String,
    pub hardware: HardwareSpecs, pub firewall: FirewallStatus,
    pub is_laptop: bool, pub lid_inhibit_active: bool, pub fingerprint: Option<String>,
    pub displays: DisplayLayout, pub role: NodeRole, pub probe_in_progress: bool,
    pub last_probe_summary: String, pub discovered: Vec<DiscoveredHost>,
    pub incoming_probe: Option<ProbeRequest>, pub active_peer: Option<DiscoveredHost>,
    pub peers: BTreeMap<String, DiscoveredHost>, pub selected_peer_ip: Option<String>,
    pub edge: EdgeSide, pub align_offset_pct: i32, pub layout_synced: bool,
    pub peer_kvm_running: bool, pub peer_portal_ready: bool, pub peer_reachable: bool,
    pub signal: SignalHealth, pub control_state: ControlState, pub conflict: InputConflictState,
    pub input_reclaim_ready: bool, pub watched_devices: usize, pub logs: Vec<String>,
}

impl AppState {
    pub fn new_listening(node_id: String, hostname: String, ip: String, subnet_cidr: String) -> Self {
        Self {
            node_id, hostname, ip, subnet_cidr,
            hardware: HardwareSpecs { device_class: "Node".into(), model: "Generic".into(), cpu: "CPU".into(), memory_gb: "0 GiB".into(), os_pretty: "OS".into(), desktop_env: "Desktop".into() },
            firewall: FirewallStatus { ufw_active: false, summary: "Default".into() },
            is_laptop: false, lid_inhibit_active: false, fingerprint: None,
            displays: DisplayLayout::default(), role: NodeRole::Listening,
            probe_in_progress: false, last_probe_summary: "Idle".into(),
            discovered: Vec::new(), incoming_probe: None, active_peer: None,
            peers: BTreeMap::new(), selected_peer_ip: None,
            edge: EdgeSide::Right, align_offset_pct: 0, layout_synced: false,
            peer_kvm_running: false, peer_portal_ready: false, peer_reachable: false,
            signal: SignalHealth::default(), control_state: ControlState::Local,
            conflict: InputConflictState::default(), input_reclaim_ready: false,
            watched_devices: 0, logs: Vec::new(),
        }
    }

    pub fn push_log(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        println!("[EdgeBridge] {}", text);
        self.logs.insert(0, format!("[{}] {}", now_epoch(), text));
        self.logs.truncate(40);
    }

    /// Synchronizes `active_peer` with `peers` across 1, 2, or N paired guests.
    pub fn upsert_peer(&mut self, mut peer: DiscoveredHost) {
        if let Some(ex) = self.peers.get(&peer.ip) {
            if peer.fingerprint.is_none() { peer.fingerprint = ex.fingerprint.clone(); }
            if peer.remote_telemetry.is_none() { peer.remote_telemetry = ex.remote_telemetry.clone(); }
        }
        let ip = peer.ip.clone();
        self.peers.insert(ip.clone(), peer.clone());
        if self.selected_peer_ip.is_none() { self.selected_peer_ip = Some(ip.clone()); }
        if self.selected_peer_ip.as_deref() == Some(ip.as_str()) {
            self.edge = peer.edge; self.align_offset_pct = peer.align_offset_pct;
            self.peer_kvm_running = peer.kvm_running; self.peer_portal_ready = peer.portal_ready;
            self.peer_reachable = peer.reachable; self.active_peer = Some(peer);
        } else if self.active_peer.is_none() { self.active_peer = Some(peer); }
    }
}

pub fn now_epoch() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
