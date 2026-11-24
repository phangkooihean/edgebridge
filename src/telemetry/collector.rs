use crate::models::{AppState, SignalHealth, StepColor, WizardStep, KVM_PORT, WEB_PORT};
use crate::scripts::format_display_summary;
use std::collections::VecDeque;

/// Computes signal score, jitter, and mouse/keyboard responsiveness rating from RTT samples.
///
/// ```
/// use std::collections::VecDeque;
/// use edgebridge::telemetry::collector::compute_signal;
/// let fast = compute_signal(&VecDeque::from([Some(8), Some(9), Some(10)]), 0, true);
/// assert_eq!(fast.responsiveness_rating, "ULTRA-RESPONSIVE");
/// assert!(fast.score_pct >= 90);
/// let down = compute_signal(&VecDeque::from([None, None, None]), 3, false);
/// assert_eq!(down.status, "DISCONNECTED");
/// assert_eq!(down.score_pct, 0);
/// ```
pub fn compute_signal(samples: &VecDeque<Option<u64>>, misses: u32, reachable: bool) -> SignalHealth {
    let total = samples.len().max(1);
    let ok: Vec<u64> = samples.iter().filter_map(|x| *x).collect();
    let loss_pct = (((total - ok.len()) * 100) / total) as u8;
    let avg_ms = if ok.is_empty() { 0 } else { ok.iter().sum::<u64>() / (ok.len() as u64) };
    let jitter_ms = if ok.len() < 2 { 0 }
        else { ok.windows(2).map(|w| w[0].abs_diff(w[1])).sum::<u64>() / ((ok.len() - 1) as u64) };

    let mut score: i32 = 100 - (loss_pct as i32) * 2;
    if avg_ms > 20 { score -= ((avg_ms - 20) / 4) as i32; }
    if jitter_ms > 10 { score -= ((jitter_ms - 10) / 3) as i32; }
    if !reachable { score = 0; }
    let score_pct = score.clamp(0, 100) as u8;

    let (status, rating, diagnosis) = if !reachable {
        ("DISCONNECTED".into(), "OFFLINE".into(), format!("{} timeouts - peer disconnected or sleeping.", misses))
    } else if misses > 0 {
        ("FAIR".into(), "JITTERY".into(), format!("Transient miss ({}) - holding via 3-miss grace.", misses))
    } else if avg_ms <= 25 && jitter_ms <= 10 && loss_pct == 0 {
        ("STRONG".into(), "ULTRA-RESPONSIVE".into(), format!("Healthy pointer & key path ({} ms avg, {} ms jitter, 0% loss).", avg_ms, jitter_ms))
    } else if score_pct >= 45 {
        ("FAIR".into(), "MODERATE-LAG".into(), format!("Moderate latency ({} ms avg, {} ms jitter, {}% loss).", avg_ms, jitter_ms, loss_pct))
    } else {
        ("POOR".into(), "STUTTER-RISK".into(), format!("High latency/jitter ({} ms avg, {} ms jitter) - cursor may stutter.", avg_ms, jitter_ms))
    };

    SignalHealth {
        transport: "Wi-Fi / LAN TCP+DTLS".into(), status, score_pct,
        avg_ping_ms: avg_ms, jitter_ms, packet_loss_pct: loss_pct,
        consecutive_misses: misses, est_kbps: if reachable { 2.4 } else { 0.0 },
        diagnosis, sample_count: ok.len(), responsiveness_rating: rating,
    }
}

/// Builds local + multi-peer (1, 2, or N guests) Unified & Shared Cross-Node Telemetry.
pub fn build_telemetry(
    state: &AppState, kvm_running: bool, kvm_bin: Option<&str>,
    engine_lines: &[String], steps: &[WizardStep],
) -> String {
    let mut out = String::new();
    out.push_str("=== EDGEBRIDGE v1.0.0-rc.1 UNIFIED & SHARED TELEMETRY ===\n");
    out.push_str(&format!(
        "MACHINE    : {} ({}) | Role: {} | IP: {} ({})\n",
        state.hostname, state.hardware.device_class, state.role.label(), state.ip, state.subnet_cidr
    ));
    out.push_str(&format!(
        "SYSTEM     : {} | {} | {} | {} | AwakeLock: {}\n",
        state.hardware.model, state.hardware.memory_gb, state.hardware.os_pretty,
        state.hardware.desktop_env, if state.lid_inhibit_active { "ON" } else { "OFF" }
    ));
    out.push_str(&format!("DISPLAYS   : {}\n", format_display_summary(&state.displays)));
    out.push_str(&format!(
        "SIGNAL/PTR : {} ({}%) | Rating: {} | Ping {}ms | Jitter {}ms | Loss {}% | Samples {} | {}\n",
        state.signal.status, state.signal.score_pct, state.signal.responsiveness_rating,
        state.signal.avg_ping_ms, state.signal.jitter_ms, state.signal.packet_loss_pct,
        state.signal.sample_count, state.signal.diagnosis
    ));
    out.push_str(&format!("PORTS/FW   : TCP {} | UDP {} | {}\n", WEB_PORT, KVM_PORT, state.firewall.summary));
    out.push_str(&format!(
        "KVM ENGINE : Running={} | Bin={} | LocalKey={} | ReclaimDevs={}\n",
        kvm_running, kvm_bin.unwrap_or("MISSING"),
        state.fingerprint.as_deref().unwrap_or("NONE"), state.watched_devices
    ));
    out.push_str(&format!(
        "OWNERSHIP  : {:?} | Conflict={} ({})\n",
        state.control_state, state.conflict.active,
        if state.conflict.summary.is_empty() { "none" } else { &state.conflict.summary }
    ));

    out.push_str(&format!("\n-- CLUSTER PEERS ({}) --\n", state.peers.len()));
    if state.peers.is_empty() {
        out.push_str(&format!("  None | {}\n", state.last_probe_summary));
    } else {
        for (ip, p) in &state.peers {
            let sel = if state.selected_peer_ip.as_deref() == Some(ip.as_str()) { "*" } else { " " };
            out.push_str(&format!(
                "  {} {} ({}, {}) | Edge: {} | Reachable: {} | KVM: {} | Portal: {} | Key: {}\n",
                sel, p.hostname, p.hardware.device_class, ip, p.edge.as_str(),
                p.reachable, p.kvm_running, p.portal_ready, p.fingerprint.as_deref().unwrap_or("NONE")
            ));
            out.push_str(&format!("    DISP: {}\n", format_display_summary(&p.displays)));
        }
    }

    out.push_str("\n-- WIZARD PROGRESS --\n");
    for s in steps {
        let tag = match s.color { StepColor::Green => "GREEN ", StepColor::Yellow => "YELLOW", StepColor::Grey => "GREY  " };
        out.push_str(&format!("  [{}] {}. {}: {}\n", tag, s.num, s.title, s.detail));
    }
    out.push_str("\n-- ENGINE LOG TAIL --\n");
    if engine_lines.is_empty() { out.push_str("  (engine standby)\n"); }
    else { for l in engine_lines { out.push_str(&format!("  {}\n", l)); } }
    out.push_str("\n-- RECENT EVENTS --\n");
    for l in state.logs.iter().take(6) { out.push_str(&format!("  {}\n", l)); }

    for (ip, p) in &state.peers {
        if let Some(rt) = &p.remote_telemetry {
            out.push_str(&format!("\n=== SHARED PEER TELEMETRY: {} ({}) ===\n{}\n", p.hostname, ip, rt));
        }
    }
    out.push_str("===============================================");
    out
}
