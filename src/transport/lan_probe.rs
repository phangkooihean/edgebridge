use crate::models::{now_epoch, AppState, DiscoveredHost, DisplayLayout, NodeRole, ProbeRequest, SignalHealth, WEB_PORT};
use crate::telemetry::collector::compute_signal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{RwLock, Semaphore};

#[derive(Serialize, Deserialize)]
pub struct PeerStatusPayload {
    pub hostname: String, pub ip: String, pub role: NodeRole, pub is_laptop: bool,
    pub lid_inhibit_active: bool, pub fingerprint: Option<String>, pub displays: DisplayLayout,
    pub kvm_running: bool, pub portal_ready: bool,
    #[serde(default)] pub telemetry_text: Option<String>,
}

pub async fn scan_subnet_and_probe(state: Arc<RwLock<AppState>>) -> (Vec<DiscoveredHost>, String) {
    let (local_ip, cidr, body) = {
        let mut g = state.write().await; g.probe_in_progress = true;
        let cidr = g.subnet_cidr.clone(); g.last_probe_summary = format!("Scanning {} on TCP {}...", cidr, WEB_PORT);
        let req = ProbeRequest {
            from_node_id: g.node_id.clone(), from_hostname: g.hostname.clone(), from_ip: g.ip.clone(),
            from_hardware: g.hardware.clone(), from_displays: g.displays.clone(),
            from_fingerprint: g.fingerprint.clone(), is_laptop: g.is_laptop,
            lid_inhibit_active: g.lid_inhibit_active, received_epoch: now_epoch(),
        };
        (g.ip.clone(), cidr, serde_json::to_string(&req).unwrap_or_else(|_| "{}".into()))
    };
    let parts: Vec<&str> = local_ip.split('.').collect();
    if parts.len() != 4 || parts[0] == "127" {
        let msg = format!("No active LAN/Hotspot IP ({}).", local_ip);
        let mut g = state.write().await; g.probe_in_progress = false; g.last_probe_summary = msg.clone();
        return (Vec::new(), msg);
    }
    let (prefix, my_last) = (format!("{}.{}.{}.", parts[0], parts[1], parts[2]), parts[3].parse::<u8>().unwrap_or(0));
    let sem = Arc::new(Semaphore::new(64));
    let mut handles = Vec::new();
    for oct in 1u8..=254u8 {
        if oct == my_last { continue; }
        let (ip, psem, b) = (format!("{}{}", prefix, oct), sem.clone(), body.clone());
        handles.push(tokio::spawn(async move {
            let _p = psem.acquire().await.ok()?;
            let start = std::time::Instant::now();
            let s = tokio::time::timeout(std::time::Duration::from_millis(420), TcpStream::connect(format!("{}:{}", ip, WEB_PORT))).await.ok()?.ok()?;
            drop(s);
            let ms = start.elapsed().as_millis() as u64;
            let resp = post_json(&ip, WEB_PORT, "/api/probe/hello", &b).await.ok()?;
            let mut host: DiscoveredHost = serde_json::from_str(&resp).ok()?;
            host.ip = ip; host.ping_ms = Some(ms);
            Some(host)
        }));
    }
    let mut found = Vec::new();
    for h in handles { if let Ok(Some(host)) = h.await { found.push(host); } }
    found.sort_by(|a, b| a.ip.cmp(&b.ip));
    let summary = if found.is_empty() { format!("Scanned 254 IPs on {} - no peers found.", cidr) } else { format!("Found {} peer(s) on {} - waiting for consent.", found.len(), cidr) };
    let mut g = state.write().await; g.probe_in_progress = false; g.discovered = found.clone(); g.last_probe_summary = summary.clone();
    (found, summary)
}

pub fn spawn_peer_health_monitor(state: Arc<RwLock<AppState>>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(1500));
        let mut samples: VecDeque<Option<u64>> = VecDeque::with_capacity(20);
        let mut misses: u32 = 0;
        loop {
            tick.tick().await;
            let peer_ip = state.read().await.active_peer.as_ref().map(|p| p.ip.clone());
            let Some(ip) = peer_ip else { samples.clear(); misses = 0; state.write().await.signal = SignalHealth::default(); continue; };
            match get_json(&ip, WEB_PORT, "/api/peer/status", 1500).await {
                Ok((body, ms)) => {
                    misses = 0;
                    if samples.len() == 20 { samples.pop_front(); }
                    samples.push_back(Some(ms));
                    let ps = serde_json::from_str::<PeerStatusPayload>(&body).ok();
                    let sig = compute_signal(&samples, misses, true);
                    let mut g = state.write().await;
                    g.peer_reachable = true; g.signal = sig;
                    if let Some(p) = ps {
                        g.peer_kvm_running = p.kvm_running; g.peer_portal_ready = p.portal_ready;
                        if let Some(ref mut cur) = g.active_peer {
                            cur.displays = p.displays.clone(); cur.lid_inhibit_active = p.lid_inhibit_active;
                            if p.fingerprint.is_some() { cur.fingerprint = p.fingerprint.clone(); }
                            if p.telemetry_text.is_some() { cur.remote_telemetry = p.telemetry_text.clone(); }
                            cur.ping_ms = Some(ms); cur.kvm_running = p.kvm_running; cur.portal_ready = p.portal_ready;
                        }
                        if let Some(map_p) = g.peers.get_mut(&ip) {
                            map_p.displays = p.displays; map_p.kvm_running = p.kvm_running; map_p.portal_ready = p.portal_ready;
                            if p.telemetry_text.is_some() { map_p.remote_telemetry = p.telemetry_text; }
                        }
                    }
                }
                Err(_) => {
                    misses += 1;
                    if samples.len() == 20 { samples.pop_front(); }
                    samples.push_back(None);
                    let ok = misses < 3;
                    let sig = compute_signal(&samples, misses, ok);
                    let mut g = state.write().await; g.peer_reachable = ok; g.signal = sig;
                }
            }
        }
    });
}

pub async fn get_json(ip: &str, port: u16, path: &str, timeout_ms: u64) -> Result<(String, u64), String> {
    let start = std::time::Instant::now();
    let addr = format!("{}:{}", ip, port);
    let mut s = tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), TcpStream::connect(&addr)).await.map_err(|_| "timeout".to_string())?.map_err(|e| e.to_string())?;
    let req = format!("GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", path, addr);
    s.write_all(req.as_bytes()).await.map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let _ = tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), s.read_to_end(&mut buf)).await;
    let text = String::from_utf8_lossy(&buf).to_string();
    Ok((text.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or(text), start.elapsed().as_millis() as u64))
}

pub async fn post_json(ip: &str, port: u16, path: &str, json_body: &str) -> Result<String, String> {
    let addr = format!("{}:{}", ip, port);
    let mut s = tokio::time::timeout(std::time::Duration::from_secs(3), TcpStream::connect(&addr)).await.map_err(|_| "timeout".to_string())?.map_err(|e| e.to_string())?;
    let req = format!("POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", path, addr, json_body.len(), json_body);
    s.write_all(req.as_bytes()).await.map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), s.read_to_end(&mut buf)).await;
    let text = String::from_utf8_lossy(&buf).to_string();
    Ok(text.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or(text))
}
