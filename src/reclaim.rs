use crate::kvm::collector::locate_lan_mouse;
use crate::models::{now_epoch, AppState};
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn spawn_input_reclaim_watcher(state: Arc<RwLock<AppState>>) {
    let st = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            tick.tick().await;
            let now = now_epoch();
            let mut g = st.write().await;
            if g.conflict.active && (now.saturating_sub(g.conflict.last_host_input_epoch) >= 3 || now.saturating_sub(g.conflict.last_guest_input_epoch) >= 3) {
                g.conflict.active = false;
                g.conflict.summary = "Resolved.".into();
            }
        }
    });
    tokio::spawn(async move { state.write().await.input_reclaim_ready = true; });
}

pub async fn release_local_capture() -> String {
    let bin = locate_lan_mouse().unwrap_or_else(|| "lan-mouse".into());
    match tokio::process::Command::new(&bin).args(["cli", "deactivate"]).output().await {
        Ok(o) => format!("Released capture ('{} cli deactivate', exit {})", bin, o.status.code().unwrap_or(-1)),
        Err(e) => format!("Tried '{} cli deactivate': {}", bin, e),
    }
}
