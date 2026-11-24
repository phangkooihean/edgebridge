use std::fs::OpenOptions;
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Child;

fn log_path() -> PathBuf { std::env::temp_dir().join("edgebridge-lan-mouse.log") }

pub struct KvmProcessManager {
    child: Option<Child>, pub binary_path: Option<String>,
    last_toml: Option<String>, last_edge: Option<String>,
}

impl KvmProcessManager {
    pub fn new() -> Self { Self { child: None, binary_path: locate_lan_mouse(), last_toml: None, last_edge: None } }
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut c) = self.child {
            match c.try_wait() { Ok(None) => return true, _ => { self.child = None; return false; } }
        }
        false
    }
    pub async fn stop(&mut self) -> String {
        self.last_toml = None; self.last_edge = None;
        if let Some(mut c) = self.child.take() {
            let pid = c.id().unwrap_or(0); let _ = c.kill().await; let _ = c.wait().await;
            format!("Stopped KVM engine child (PID {}).", pid)
        } else { "KVM engine was not running.".into() }
    }
    pub async fn apply_and_restart(&mut self, config_toml: &str, edge: &str, force_restart: bool) -> Result<String, String> {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into());
        let xdg = PathBuf::from(&home).join(".config/lan-mouse");
        std::fs::create_dir_all(&xdg).map_err(|e| e.to_string())?;
        std::fs::write(xdg.join("config.toml"), config_toml).map_err(|e| e.to_string())?;
        #[cfg(target_os = "windows")]
        if let Ok(app) = std::env::var("APPDATA") {
            let ad = PathBuf::from(app).join("lan-mouse");
            let _ = std::fs::create_dir_all(&ad);
            let _ = std::fs::write(ad.join("config.toml"), config_toml);
        }
        #[cfg(target_os = "macos")]
        {
            let md = PathBuf::from(&home).join("Library/Application Support/lan-mouse");
            let _ = std::fs::create_dir_all(&md);
            let _ = std::fs::write(md.join("config.toml"), config_toml);
        }
        if self.binary_path.is_none() { self.binary_path = locate_lan_mouse(); }
        let Some(bin) = self.binary_path.clone() else { return Err("lan-mouse binary not found.".into()); };
        if self.is_running() && !force_restart && self.last_toml.as_deref() == Some(config_toml) {
            return Ok("KVM engine already active (OS session preserved).".into());
        }
        self.stop().await;
        let mut std_cmd = std::process::Command::new(&bin);
        std_cmd.arg("daemon").env("RUST_LOG", "info").env("XDG_CONFIG_HOME", PathBuf::from(&home).join(".config"));
        if let Ok(f) = OpenOptions::new().create(true).write(true).truncate(true).open(log_path()) {
            if let Ok(fe) = f.try_clone() { std_cmd.stdout(Stdio::from(f)); std_cmd.stderr(Stdio::from(fe)); }
        }
        #[cfg(target_os = "linux")]
        unsafe { std_cmd.pre_exec(|| { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL); Ok(()) }); }
        let mut tc = tokio::process::Command::from(std_cmd);
        tc.kill_on_drop(true);
        let child = tc.spawn().map_err(|e| format!("Failed launching '{}': {}", bin, e))?;
        let pid = child.id().unwrap_or(0);
        self.child = Some(child); self.last_toml = Some(config_toml.into()); self.last_edge = Some(edge.into());
        Ok(format!("KVM engine started (PID {}).", pid))
    }
}

pub fn locate_lan_mouse() -> Option<String> {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
    for p in [
        format!("{}/.cargo/bin/lan-mouse.exe", home),
        format!("{}/.cargo/bin/lan-mouse", home),
        format!("{}/.local/bin/lan-mouse", home),
        "/opt/homebrew/bin/lan-mouse".into(),
        "/usr/local/bin/lan-mouse".into(),
    ] {
        if std::path::Path::new(&p).is_file() { return Some(p); }
    }
    let checker = if cfg!(target_os = "windows") { "where" } else { "which" };
    std::process::Command::new(checker).arg("lan-mouse").output().ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(|l| l.trim().to_string()))
        .filter(|s| !s.is_empty())
}

pub fn read_log_tail(max_lines: usize) -> Vec<String> {
    let Ok(t) = std::fs::read_to_string(log_path()) else { return Vec::new(); };
    let v: Vec<String> = t.lines().map(|l| l.to_string()).collect();
    v[v.len().saturating_sub(max_lines)..].to_vec()
}

pub fn extract_fingerprint_from_log() -> Option<String> {
    let t = std::fs::read_to_string(log_path()).ok()?;
    for tok in t.split(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
        if tok.len() == 95 && tok.matches(':').count() == 31 && tok.chars().all(|c| c.is_ascii_hexdigit() || c == ':') {
            return Some(tok.to_lowercase());
        }
    }
    None
}

/// Validates OS input backend status from engine logs across macOS, Linux & Windows.
///
/// ```
/// use edgebridge::kvm::collector::portal_status_from_log;
/// assert!(portal_status_from_log(true, &["using emulation backend: libei".into()]).0);
/// assert!(!portal_status_from_log(true, &["using emulation backend: dummy".into()]).0);
/// assert!(!portal_status_from_log(false, &[]).0);
/// ```
pub fn portal_status_from_log(running: bool, lines: &[String]) -> (bool, String) {
    if !running { return (false, "Ready - click Next to start KVM input engine.".into()); }
    let j = lines.join("\n").to_lowercase();
    if j.contains("using emulation backend: dummy") || j.contains("cancelled") || j.contains("canceled")
        || j.contains("denied") || j.contains("not trusted") || j.contains("accessibility permission")
    {
        return (false, "OS input permission pending/denied - approve permission and click Next.".into());
    }
    if j.contains("using emulation backend:") || j.contains("using capture backend:") || j.contains("activated client") {
        return (true, "OS input backend active.".into());
    }
    (false, "Waiting for OS input permission (GNOME Share / macOS Accessibility / Windows).".into())
}
