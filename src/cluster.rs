use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::IpAddr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeRole {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ScreenEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrimaryStatus {
    Healthy,
    Suspect,
    Lost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub node_id: String,
    pub hostname: String,
    pub ip: IpAddr,
    pub role: NodeRole,
    pub edge: Option<ScreenEdge>,
    pub reachable: bool,
    pub kvm_ready: bool,
    pub portal_ready: bool,
    pub pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSnapshot {
    pub epoch: u64,
    pub primary_node_id: String,
    pub primary_ip: IpAddr,
    pub peers: Vec<PeerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverState {
    pub epoch: u64,
    pub primary_node_id: String,
    pub primary_status: PrimaryStatus,
    pub last_heartbeat_ms: u64,
    pub lease_ttl_ms: u64,
}

impl FailoverState {
    pub fn new(epoch: u64, primary_node_id: String, now_ms: u64, lease_ttl_ms: u64) -> Self {
        Self {
            epoch,
            primary_node_id,
            primary_status: PrimaryStatus::Healthy,
            last_heartbeat_ms: now_ms,
            lease_ttl_ms: lease_ttl_ms.max(1),
        }
    }

    pub fn record_heartbeat(&mut self, epoch: u64, primary_node_id: String, now_ms: u64) -> bool {
        if epoch < self.epoch {
            return false;
        }
        if self.primary_status == PrimaryStatus::Lost
            && epoch == self.epoch
            && primary_node_id == self.primary_node_id
        {
            return false;
        }
        self.epoch = epoch;
        self.primary_node_id = primary_node_id;
        self.primary_status = PrimaryStatus::Healthy;
        self.last_heartbeat_ms = now_ms;
        true
    }

    pub fn evaluate_lease(&mut self, now_ms: u64) -> PrimaryStatus {
        let elapsed = now_ms.saturating_sub(self.last_heartbeat_ms);
        if elapsed >= self.lease_ttl_ms {
            self.primary_status = PrimaryStatus::Lost;
        } else if elapsed >= self.lease_ttl_ms / 2 && self.primary_status != PrimaryStatus::Lost {
            self.primary_status = PrimaryStatus::Suspect;
        } else if self.primary_status != PrimaryStatus::Lost {
            self.primary_status = PrimaryStatus::Healthy;
        }
        self.primary_status.clone()
    }

    pub fn begin_failover(&mut self, failed_primary_id: &str) -> u64 {
        self.epoch = self.epoch.saturating_add(1);
        self.primary_node_id = failed_primary_id.to_string();
        self.primary_status = PrimaryStatus::Lost;
        self.epoch
    }
}

pub fn format_kvm_target(peer: &PeerInfo, port: u16) -> String {
    match peer.ip {
        IpAddr::V4(v4) => format!("{v4}:{port}"),
        IpAddr::V6(v6) => format!("[{v6}]:{port}"),
    }
}

pub fn find_edge_conflicts(peers: &[PeerInfo]) -> Vec<(ScreenEdge, Vec<String>)> {
    let mut seen_edges = HashSet::new();
    let mut conflicting_edges = Vec::new();
    for peer in peers {
        if let Some(edge) = &peer.edge {
            if !seen_edges.insert(edge.clone()) && !conflicting_edges.contains(edge) {
                conflicting_edges.push(edge.clone());
            }
        }
    }
    conflicting_edges.sort();
    conflicting_edges
        .into_iter()
        .map(|edge| {
            let mut names: Vec<String> = peers
                .iter()
                .filter(|p| p.edge.as_ref() == Some(&edge))
                .map(|p| format!("{} ({})", p.hostname, p.ip))
                .collect();
            names.sort();
            (edge, names)
        })
        .collect()
}

pub fn elect_new_primary<'a>(peers: &'a [PeerInfo], exclude_node_id: &str) -> Option<&'a PeerInfo> {
    peers
        .iter()
        .filter(|p| p.reachable && p.node_id != exclude_node_id)
        .min_by(|a, b| {
            let a_not_ready = !(a.kvm_ready && a.portal_ready);
            let b_not_ready = !(b.kvm_ready && b.portal_ready);
            a_not_ready
                .cmp(&b_not_ready)
                .then_with(|| a.ip.cmp(&b.ip))
                .then_with(|| a.node_id.cmp(&b.node_id))
        })
}
