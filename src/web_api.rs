use crate::kvm::collector::{
    extract_fingerprint_from_log, portal_status_from_log, read_log_tail, KvmProcessManager,
};
use crate::models::{
    now_epoch, AppState, ControlState, DiscoveredHost, DisplayLayout, EdgeSide, FirewallStatus,
    HardwareSpecs, InputConflictState, NodeRole, ProbeRequest, SignalHealth, StepColor, WizardStep,
    WEB_PORT,
};
use crate::platform;
use crate::power::LidInhibitorManager;
use crate::reclaim::release_local_capture;
use crate::scripts::write_config_files;
use crate::telemetry::collector::build_telemetry;
use crate::transport::lan_probe::{post_json, scan_subnet_and_probe, PeerStatusPayload};
use axum::{extract::State, response::Html, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

const INDEX_HTML: &str = include_str!("../web/index.html");

#[derive(Clone)]
pub struct WebCtx {
    pub state: Arc<RwLock<AppState>>,
    pub kvm: Arc<Mutex<KvmProcessManager>>,
    pub power: Arc<Mutex<LidInhibitorManager>>,
}

#[derive(Serialize)]
struct StateSnapshot {
    hostname: String, ip: String, subnet_cidr: String,
    hardware: HardwareSpecs, firewall: FirewallStatus,
    role: NodeRole, is_laptop: bool, lid_inhibit_active: bool,
    fingerprint: Option<String>, displays: DisplayLayout,
    probe_in_progress: bool, last_probe_summary: String,
    incoming_probe: Option<ProbeRequest>, active_peer: Option<DiscoveredHost>,
    peers: BTreeMap<String, DiscoveredHost>, selected_peer_ip: Option<String>,
    edge: EdgeSide, align_offset_pct: i32, signal: SignalHealth,
    control_state: ControlState, conflict: InputConflictState,
    kvm_running: bool, portal_ready: bool, os_permission_notice: String,
    steps: Vec<WizardStep>, next_action: String, next_label: String, telemetry_text: String,
}

#[derive(Deserialize)] struct RespondProbeReq { accept: bool }
#[derive(Deserialize)] struct EdgeReq { edge: EdgeSide, align_offset_pct: i32 }
#[derive(Deserialize)] struct SelectPeerReq { peer_ip: String }
#[derive(Serialize, Deserialize)]
struct PeerSyncReq {
    from_hostname: String, from_ip: String, from_fingerprint: Option<String>,
    edge: EdgeSide, align_offset_pct: i32, start_kvm: bool, force_restart: bool,
}
#[derive(Serialize, Deserialize)] struct ToggleLidReq { enable: bool, peer_ip: Option<String> }
#[derive(Serialize, Deserialize)] struct HookReq { peer_ip: String, peer_hostname: String }
#[derive(Serialize, Deserialize, Default)] struct ReclaimReq { source: Option<String> }
#[derive(Serialize)] struct ActionResp { ok: bool, message: String }

/// Wizard progression invariant across paired, layout-synced, portal & key states.
///
/// ```
/// use edgebridge::web_api::next_wizard_action;
/// assert_eq!(next_wizard_action(false, false, false, false).0, "probe");
/// assert_eq!(next_wizard_action(true, false, false, false).0, "sync");
/// assert_eq!(next_wizard_action(true, true, false, false).0, "permissions");
/// assert_eq!(next_wizard_action(true, true, true, true).0, "live");
/// ```
pub fn next_wizard_action(
    paired: bool, layout_synced: bool, both_portals: bool, keys_ready: bool,
) -> (&'static str, &'static str) {
    if !paired { ("probe", "1. Probe Network (Scan Subnet)") }
    else if !layout_synced { ("sync", "Next: Sync Layout & Keys ->") }
    else if !both_portals { ("permissions", "Next: Grant Input Permission ->") }
    else if !keys_ready { ("sync_keys", "Next: Finalize TLS Key Pairing ->") }
    else { ("live", "Bridge LIVE - Re-Sync") }
}

pub fn build_router(ctx: WebCtx) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/state", get(get_state))
        .route("/api/telemetry", get(get_telemetry))
        .route("/api/telemetry/raw", get(get_raw_telemetry))
        .route("/api/peer/status", get(peer_status))
        .route("/api/peer/select", post(select_peer))
        .route("/api/probe/scan", post(trigger_probe_scan))
        .route("/api/probe/hello", post(receive_probe_hello))
        .route("/api/probe/respond", post(respond_to_probe))
        .route("/api/probe/accepted", post(receive_probe_accepted))
        .route("/api/step/sync", post(step_sync_layout))
        .route("/api/step/permissions", post(step_grant_permissions))
        .route("/api/layout/edge", post(update_edge))
        .route("/api/peer/sync", post(receive_peer_sync))
        .route("/api/kvm/stop", post(stop_kvm_both))
        .route("/api/power/lid", post(toggle_lid))
        .route("/api/reset", post(reset_session))
        .route("/api/hook/enter", post(hook_enter))
        .route("/api/hook/leave", post(hook_leave))
        .route("/api/peer/remote_enter", post(peer_remote_enter))
        .route("/api/peer/remote_leave", post(peer_remote_leave))
        .route("/api/reclaim", post(reclaim_ownership))
        .with_state(ctx)
}

async fn serve_index() -> Html<&'static str> { Html(INDEX_HTML) }

async fn get_raw_telemetry(State(ctx): State<WebCtx>) -> String {
    compute_snapshot(&ctx).await.telemetry_text
}

async fn peer_status(State(ctx): State<WebCtx>) -> Json<PeerStatusPayload> {
    let snap = compute_snapshot(&ctx).await;
    Json(PeerStatusPayload {
        hostname: snap.hostname, ip: snap.ip, role: snap.role,
        is_laptop: snap.is_laptop, lid_inhibit_active: snap.lid_inhibit_active,
        fingerprint: snap.fingerprint, displays: snap.displays,
        kvm_running: snap.kvm_running, portal_ready: snap.portal_ready,
        telemetry_text: Some(snap.telemetry_text),
    })
}

async fn select_peer(State(ctx): State<WebCtx>, Json(req): Json<SelectPeerReq>) -> Json<ActionResp> {
    let mut g = ctx.state.write().await;
    if let Some(p) = g.peers.get(&req.peer_ip).cloned() {
        g.selected_peer_ip = Some(req.peer_ip.clone());
        g.edge = p.edge; g.align_offset_pct = p.align_offset_pct;
        g.peer_kvm_running = p.kvm_running; g.peer_portal_ready = p.portal_ready;
        g.peer_reachable = p.reachable; g.active_peer = Some(p.clone());
        Json(ActionResp { ok: true, message: format!("Switched active peer to {} ({}).", p.hostname, p.ip) })
    } else {
        Json(ActionResp { ok: false, message: format!("Peer {} not in cluster.", req.peer_ip) })
    }
}

async fn get_state(State(ctx): State<WebCtx>) -> Json<StateSnapshot> { Json(compute_snapshot(&ctx).await) }
async fn get_telemetry(State(ctx): State<WebCtx>) -> String { compute_snapshot(&ctx).await.telemetry_text }
async fn trigger_probe_scan(State(ctx): State<WebCtx>) -> Json<ActionResp> {
    let (_, s) = scan_subnet_and_probe(ctx.state.clone()).await;
    Json(ActionResp { ok: true, message: s })
}

fn build_wizard_steps(
    g: &AppState, paired: bool, keys_ready: bool,
    portal_ready: bool, portal_msg: String, kvm_running: bool,
) -> (Vec<WizardStep>, String) {
    let both_portals = portal_ready && g.peer_portal_ready;
    let s1 = if paired {
        WizardStep { num: 1, title: "Probe & Role Handshake".into(), color: StepColor::Green, detail: format!("Paired ({} ms, {} peer(s)) as {}", g.signal.avg_ping_ms, g.peers.len().max(1), g.role.label()) }
    } else if g.probe_in_progress || !g.discovered.is_empty() || g.incoming_probe.is_some() {
        WizardStep { num: 1, title: "Probe & Role Handshake".into(), color: StepColor::Yellow, detail: if g.incoming_probe.is_some() { "Incoming probe - click Accept above.".into() } else { g.last_probe_summary.clone() } }
    } else {
        WizardStep { num: 1, title: "Probe & Role Handshake".into(), color: StepColor::Grey, detail: format!("Listening on {}:{} - click Probe Network on Primary.", g.ip, WEB_PORT) }
    };
    let s2 = if !paired {
        WizardStep { num: 2, title: "Layout & Key Sync".into(), color: StepColor::Grey, detail: "Complete Step 1 first.".into() }
    } else if g.layout_synced && keys_ready {
        WizardStep { num: 2, title: "Layout & Key Sync".into(), color: StepColor::Green, detail: format!("Edge '{}' synced & TLS keys paired.", g.edge.as_str().to_uppercase()) }
    } else {
        WizardStep { num: 2, title: "Layout & Key Sync".into(), color: StepColor::Yellow, detail: if !g.layout_synced { "Drag peer monitors magnetically and click Next: Sync Layout.".into() } else { "Keys pending - Step 3 will pair them.".into() } }
    };
    let s3 = if !paired || !g.layout_synced {
        WizardStep { num: 3, title: "OS Input Permissions".into(), color: StepColor::Grey, detail: "Waiting for Step 2.".into() }
    } else if both_portals {
        WizardStep { num: 3, title: "OS Input Permissions".into(), color: StepColor::Green, detail: "Granted on BOTH Primary and Guest.".into() }
    } else {
        WizardStep { num: 3, title: "OS Input Permissions".into(), color: StepColor::Yellow, detail: if !portal_ready { portal_msg } else { "Granted here - approve permission on peer & click Next.".into() } }
    };
    let live = paired && g.layout_synced && keys_ready && both_portals && kvm_running && g.peer_kvm_running;
    let s4 = if live {
        WizardStep { num: 4, title: "Bridge LIVE".into(), color: StepColor::Green, detail: format!("Move mouse {} onto Guest - keyboard & mouse active!", g.edge.as_str().to_uppercase()) }
    } else if both_portals && !keys_ready {
        WizardStep { num: 4, title: "Bridge LIVE".into(), color: StepColor::Yellow, detail: "Permissions granted! Click Next once to finalize TLS keys.".into() }
    } else {
        WizardStep { num: 4, title: "Bridge LIVE".into(), color: StepColor::Grey, detail: "Activates once Steps 1-3 are green.".into() }
    };
    let notice = platform::os_permission_notice(both_portals, paired && g.layout_synced);
    (vec![s1, s2, s3, s4], notice)
}

async fn compute_snapshot(ctx: &WebCtx) -> StateSnapshot {
    let (kvm_running, kvm_bin) = { let mut m = ctx.kvm.lock().await; (m.is_running(), m.binary_path.clone()) };
    let (_, lid_active, _) = ctx.power.lock().await.status_summary();
    let full_log = read_log_tail(usize::MAX);
    let (portal_ready, portal_msg) = portal_status_from_log(kvm_running, &full_log);
    let tail = full_log[full_log.len().saturating_sub(12)..].to_vec();
    let log_fp = extract_fingerprint_from_log();
    {
        let mut g = ctx.state.write().await;
        g.lid_inhibit_active = lid_active;
        if g.fingerprint.is_none() && log_fp.is_some() { g.fingerprint = log_fp; }
    }
    let g = ctx.state.read().await;
    let paired = g.active_peer.is_some() && g.peer_reachable;
    let keys_ready = g.fingerprint.is_some() && g.active_peer.as_ref().and_then(|p| p.fingerprint.as_ref()).is_some();
    let both_portals = portal_ready && g.peer_portal_ready;
    let (steps, os_permission_notice) = build_wizard_steps(&g, paired, keys_ready, portal_ready, portal_msg, kvm_running);
    let (next_act, next_lbl) = next_wizard_action(paired, g.layout_synced, both_portals, keys_ready);
    let telemetry_text = build_telemetry(&g, kvm_running, kvm_bin.as_deref(), &tail, &steps);
    StateSnapshot {
        hostname: g.hostname.clone(), ip: g.ip.clone(), subnet_cidr: g.subnet_cidr.clone(),
        hardware: g.hardware.clone(), firewall: g.firewall.clone(), role: g.role,
        is_laptop: g.is_laptop, lid_inhibit_active: g.lid_inhibit_active,
        fingerprint: g.fingerprint.clone(), displays: g.displays.clone(),
        probe_in_progress: g.probe_in_progress, last_probe_summary: g.last_probe_summary.clone(),
        incoming_probe: g.incoming_probe.clone(), active_peer: g.active_peer.clone(),
        peers: g.peers.clone(), selected_peer_ip: g.selected_peer_ip.clone(),
        edge: g.edge, align_offset_pct: g.align_offset_pct, signal: g.signal.clone(),
        control_state: g.control_state.clone(), conflict: g.conflict.clone(),
        kvm_running, portal_ready, os_permission_notice, steps,
        next_action: next_act.into(), next_label: next_lbl.into(), telemetry_text,
    }
}

async fn receive_probe_hello(State(ctx): State<WebCtx>, Json(req): Json<ProbeRequest>) -> Json<DiscoveredHost> {
    let mut g = ctx.state.write().await;
    g.push_log(format!("HEARD PROBE from '{}' ({}) asking to become Primary.", req.from_hostname, req.from_ip));
    let mut st = req.clone(); st.received_epoch = now_epoch(); g.incoming_probe = Some(st);
    platform::send_notification("EdgeBridge Probe", &format!("{} ({}) wants Primary.", req.from_hostname, req.from_ip));
    Json(DiscoveredHost {
        node_id: g.node_id.clone(), hostname: g.hostname.clone(), ip: g.ip.clone(),
        hardware: g.hardware.clone(), is_laptop: g.is_laptop, lid_inhibit_active: g.lid_inhibit_active,
        fingerprint: g.fingerprint.clone(), displays: g.displays.clone(), role: g.role,
        ping_ms: Some(0), accepted: false, edge: g.edge, align_offset_pct: g.align_offset_pct,
        kvm_running: false, portal_ready: false, reachable: true, remote_telemetry: None,
    })
}

async fn respond_to_probe(State(ctx): State<WebCtx>, Json(req): Json<RespondProbeReq>) -> Json<ActionResp> {
    let Some(p) = ctx.state.write().await.incoming_probe.take() else {
        return Json(ActionResp { ok: false, message: "No pending probe request.".into() });
    };
    if !req.accept { return Json(ActionResp { ok: true, message: "Probe declined.".into() }); }
    let reply = {
        let mut g = ctx.state.write().await;
        g.role = NodeRole::Guest; g.edge = EdgeSide::Left; g.peer_reachable = true;
        g.upsert_peer(DiscoveredHost {
            node_id: p.from_node_id.clone(), hostname: p.from_hostname.clone(), ip: p.from_ip.clone(),
            hardware: p.from_hardware.clone(), is_laptop: p.is_laptop, lid_inhibit_active: p.lid_inhibit_active,
            fingerprint: p.from_fingerprint.clone(), displays: p.from_displays.clone(),
            role: NodeRole::Primary, ping_ms: Some(1), accepted: true,
            edge: EdgeSide::Left, align_offset_pct: 0, kvm_running: false, portal_ready: false,
            reachable: true, remote_telemetry: None,
        });
        DiscoveredHost {
            node_id: g.node_id.clone(), hostname: g.hostname.clone(), ip: g.ip.clone(),
            hardware: g.hardware.clone(), is_laptop: g.is_laptop, lid_inhibit_active: g.lid_inhibit_active,
            fingerprint: g.fingerprint.clone(), displays: g.displays.clone(), role: NodeRole::Guest,
            ping_ms: Some(1), accepted: true, edge: EdgeSide::Left, align_offset_pct: 0,
            kvm_running: false, portal_ready: false, reachable: true, remote_telemetry: None,
        }
    };
    let body = serde_json::to_string(&reply).unwrap_or_else(|_| "{}".into());
    match post_json(&p.from_ip, WEB_PORT, "/api/probe/accepted", &body).await {
        Ok(_) => Json(ActionResp { ok: true, message: format!("Accepted {} as Primary!", p.from_hostname) }),
        Err(e) => Json(ActionResp { ok: false, message: format!("Notify error: {}", e) }),
    }
}

async fn receive_probe_accepted(State(ctx): State<WebCtx>, Json(mut guest): Json<DiscoveredHost>) -> Json<ActionResp> {
    guest.accepted = true; guest.role = NodeRole::Guest; guest.reachable = true;
    let mut g = ctx.state.write().await;
    g.role = NodeRole::Primary; g.peer_reachable = true;
    g.push_log(format!("Guest '{}' ({}) paired!", guest.hostname, guest.ip));
    g.upsert_peer(guest);
    Json(ActionResp { ok: true, message: "Guest paired.".into() })
}

async fn stop_kvm_both(State(ctx): State<WebCtx>) -> Json<ActionResp> {
    let msg = ctx.kvm.lock().await.stop().await;
    ctx.state.write().await.push_log(msg.clone());
    Json(ActionResp { ok: true, message: msg })
}

async fn toggle_lid(State(ctx): State<WebCtx>, Json(req): Json<ToggleLidReq>) -> Json<ActionResp> {
    if let Some(ip) = req.peer_ip.filter(|s| !s.is_empty()) {
        let b = serde_json::json!({ "enable": req.enable, "peer_ip": null }).to_string();
        return match post_json(&ip, WEB_PORT, "/api/power/lid", &b).await {
            Ok(_) => Json(ActionResp { ok: true, message: format!("Updated {} awake lock.", ip) }),
            Err(e) => Json(ActionResp { ok: false, message: e }),
        };
    }
    let (ok, msg, active) = {
        let mut p = ctx.power.lock().await;
        if req.enable { match p.enable().await { Ok(m) => (true, m, true), Err(e) => (false, e, false) } }
        else { (true, p.disable().await, false) }
    };
    let mut g = ctx.state.write().await; g.lid_inhibit_active = active;
    Json(ActionResp { ok, message: msg })
}

async fn push_sync_to_both(ctx: &WebCtx, start_kvm: bool, force_restart: bool) -> ActionResp {
    let (peer_ip, edge_str, toml, sync_req) = {
        let mut g = ctx.state.write().await;
        let Some(peer) = g.active_peer.clone() else {
            return ActionResp { ok: false, message: "Complete Step 1 (Probe & Accept) first.".into() };
        };
        g.layout_synced = true;
        let cur_edge = g.edge;
        let cur_off = g.align_offset_pct;
        if let Some(ref mut p) = g.active_peer { p.edge = cur_edge; p.align_offset_pct = cur_off; }
        if let Some(mp) = g.peers.get_mut(&peer.ip) { mp.edge = cur_edge; mp.align_offset_pct = cur_off; }
        let toml = write_config_files(&g).unwrap_or_default();
        let req = PeerSyncReq {
            from_hostname: g.hostname.clone(), from_ip: g.ip.clone(), from_fingerprint: g.fingerprint.clone(),
            edge: g.edge.opposite(), align_offset_pct: -g.align_offset_pct, start_kvm, force_restart,
        };
        (peer.ip, g.edge.as_str().to_string(), toml, req)
    };
    let mut msg = "Synced layout & keys locally.".to_string();
    if start_kvm {
        msg = match ctx.kvm.lock().await.apply_and_restart(&toml, &edge_str, force_restart).await { Ok(m) | Err(m) => m };
        ctx.state.write().await.push_log(msg.clone());
    }
    let body = serde_json::to_string(&sync_req).unwrap_or_else(|_| "{}".into());
    match post_json(&peer_ip, WEB_PORT, "/api/peer/sync", &body).await {
        Ok(_) => ActionResp { ok: true, message: format!("{} | Peer {} synced.", msg, peer_ip) },
        Err(e) => ActionResp { ok: false, message: format!("{} | Sync error: {}", msg, e) },
    }
}

async fn step_sync_layout(State(ctx): State<WebCtx>) -> Json<ActionResp> { Json(push_sync_to_both(&ctx, false, false).await) }
async fn step_grant_permissions(State(ctx): State<WebCtx>) -> Json<ActionResp> { Json(push_sync_to_both(&ctx, true, false).await) }
async fn update_edge(State(ctx): State<WebCtx>, Json(req): Json<EdgeReq>) -> Json<ActionResp> {
    let start_kvm = ctx.kvm.lock().await.is_running();
    { let mut g = ctx.state.write().await; g.edge = req.edge; g.align_offset_pct = req.align_offset_pct; }
    Json(push_sync_to_both(&ctx, start_kvm, false).await)
}
async fn receive_peer_sync(State(ctx): State<WebCtx>, Json(req): Json<PeerSyncReq>) -> Json<ActionResp> {
    let edge_str = req.edge.as_str().to_string();
    let toml = {
        let mut g = ctx.state.write().await;
        g.edge = req.edge; g.align_offset_pct = req.align_offset_pct; g.layout_synced = true;
        if let Some(ref mut p) = g.active_peer { if req.from_fingerprint.is_some() { p.fingerprint = req.from_fingerprint.clone(); } p.edge = req.edge; }
        if let Some(mp) = g.peers.get_mut(&req.from_ip) { if req.from_fingerprint.is_some() { mp.fingerprint = req.from_fingerprint; } mp.edge = req.edge; }
        write_config_files(&g).unwrap_or_default()
    };
    let msg = if req.start_kvm {
        let m = match ctx.kvm.lock().await.apply_and_restart(&toml, &edge_str, req.force_restart).await { Ok(s) | Err(s) => s };
        ctx.state.write().await.push_log(m.clone()); m
    } else { "Mirrored config saved.".into() };
    Json(ActionResp { ok: true, message: msg })
}

async fn reset_session(State(ctx): State<WebCtx>) -> Json<ActionResp> {
    let _ = ctx.kvm.lock().await.stop().await;
    let mut g = ctx.state.write().await;
    g.role = NodeRole::Listening; g.active_peer = None; g.peers.clear(); g.selected_peer_ip = None;
    g.incoming_probe = None; g.discovered.clear(); g.layout_synced = false; g.peer_reachable = false;
    g.control_state = ControlState::Local; g.conflict.active = false; g.signal = SignalHealth::default();
    Json(ActionResp { ok: true, message: "Reset to passive LISTENING mode.".into() })
}
async fn hook_enter(State(ctx): State<WebCtx>, Json(req): Json<HookReq>) -> Json<ActionResp> {
    let (my_ip, my_host) = {
        let mut g = ctx.state.write().await;
        g.control_state = ControlState::ControllingPeer { peer_hostname: req.peer_hostname.clone(), peer_ip: req.peer_ip.clone() };
        (g.ip.clone(), g.hostname.clone())
    };
    let _ = post_json(&req.peer_ip, WEB_PORT, "/api/peer/remote_enter", &serde_json::json!({ "peer_ip": my_ip, "peer_hostname": my_host }).to_string()).await;
    Json(ActionResp { ok: true, message: "OK".into() })
}
async fn hook_leave(State(ctx): State<WebCtx>, Json(req): Json<HookReq>) -> Json<ActionResp> {
    { ctx.state.write().await.control_state = ControlState::Local; }
    let _ = post_json(&req.peer_ip, WEB_PORT, "/api/peer/remote_leave", &serde_json::json!({ "peer_ip": req.peer_ip, "peer_hostname": req.peer_hostname }).to_string()).await;
    Json(ActionResp { ok: true, message: "OK".into() })
}
async fn peer_remote_enter(State(ctx): State<WebCtx>, Json(req): Json<HookReq>) -> Json<ActionResp> {
    ctx.state.write().await.control_state = ControlState::RemoteControlled { controller_hostname: req.peer_hostname, controller_ip: req.peer_ip };
    Json(ActionResp { ok: true, message: "OK".into() })
}
async fn peer_remote_leave(State(ctx): State<WebCtx>, Json(_): Json<HookReq>) -> Json<ActionResp> {
    ctx.state.write().await.control_state = ControlState::Local;
    Json(ActionResp { ok: true, message: "OK".into() })
}
async fn reclaim_ownership(State(ctx): State<WebCtx>, req: Option<Json<ReclaimReq>>) -> Json<ActionResp> {
    let now = now_epoch();
    let src = req.and_then(|r| r.0.source).unwrap_or_else(|| "manual".into());
    let peer_ip = {
        let mut g = ctx.state.write().await;
        let target = match &g.control_state { ControlState::RemoteControlled { controller_ip, .. } => Some(controller_ip.clone()), _ => None };
        g.control_state = ControlState::Local;
        if src == "guest" { g.conflict.active = true; g.conflict.last_guest_input_epoch = now; g.conflict.summary = "CONFLICT: Guest input reclaimed.".into(); }
        target
    };
    let msg = release_local_capture().await;
    if let Some(ip) = peer_ip { let _ = post_json(&ip, WEB_PORT, "/api/reclaim", "{\"source\":\"guest\"}").await; }
    Json(ActionResp { ok: true, message: msg })
}
