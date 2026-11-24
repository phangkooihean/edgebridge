#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use tokio::process::Child;

pub fn is_laptop() -> bool {
    #[cfg(target_os = "macos")]
    {
        let b = std::process::Command::new("pmset").args(["-g", "batt"]).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
        if b.contains("InternalBattery") { return true; }
        let m = std::process::Command::new("sysctl").args(["-n", "hw.model"]).output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase()).unwrap_or_default();
        return m.contains("macbook");
    }
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/proc/acpi/button/lid").exists() { return true; }
        if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") {
            for e in entries.flatten() { if e.file_name().to_string_lossy().starts_with("BAT") { return true; } }
        }
        return false;
    }
    #[cfg(target_os = "windows")]
    unsafe {
        let mut pwr: windows_sys::Win32::System::Power::SYSTEM_POWER_STATUS = std::mem::zeroed();
        if windows_sys::Win32::System::Power::GetSystemPowerStatus(&mut pwr) != 0 { return (pwr.BatteryFlag & 128) == 0; }
        return false;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    false
}

pub struct LidInhibitorManager { child: Option<Child>, pub is_laptop: bool, last_status: String }

impl LidInhibitorManager {
    pub fn new() -> Self {
        let laptop = is_laptop();
        Self { child: None, is_laptop: laptop, last_status: if laptop { "Laptop detected - awake lock ready.".into() } else { "Desktop - awake lock on standby.".into() } }
    }
    pub fn is_active(&mut self) -> bool {
        if let Some(ref mut c) = self.child {
            match c.try_wait() { Ok(None) => return true, _ => { self.child = None; return false; } }
        }
        false
    }
    pub fn status_summary(&mut self) -> (bool, bool, String) { (self.is_laptop, self.is_active(), self.last_status.clone()) }
}

impl LidInhibitorManager {
    pub async fn enable(&mut self) -> Result<String, String> {
        if self.is_active() { return Ok(self.last_status.clone()); }
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::System::Threading::{SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED};
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
            self.last_status = "Windows awake state active (SetThreadExecutionState).".into();
            return Ok(self.last_status.clone());
        }
        #[cfg(target_os = "macos")]
        let mut cmd = {
            let mut c = tokio::process::Command::new("caffeinate");
            c.args(["-d", "-i", "-s", "-w", &std::process::id().to_string()]);
            c.stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true); c
        };
        #[cfg(target_os = "linux")]
        let mut cmd = {
            let mut sc = std::process::Command::new("systemd-inhibit");
            sc.args(["--what=handle-lid-switch:sleep:idle", "--who=EdgeBridge", "--why=EdgeBridge KVM", "--mode=block", "sleep", "infinity"]);
            sc.stdout(Stdio::null()).stderr(Stdio::null());
            unsafe { sc.pre_exec(|| { libc::setpgid(0, 0); libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL); Ok(()) }); }
            let mut tc = tokio::process::Command::from(sc); tc.kill_on_drop(true); tc
        };
        #[cfg(not(target_os = "windows"))]
        {
            let child = cmd.spawn().map_err(|e| format!("Awake lock failed: {}", e))?;
            let pid = child.id().unwrap_or(0); self.child = Some(child);
            self.last_status = format!("Sleep/lid-close INHIBITED (PID {}).", pid);
            Ok(self.last_status.clone())
        }
    }
    pub async fn disable(&mut self) -> String {
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::System::Threading::{SetThreadExecutionState, ES_CONTINUOUS};
            SetThreadExecutionState(ES_CONTINUOUS);
        }
        if let Some(mut c) = self.child.take() {
            let _ = c.kill().await; let _ = c.wait().await;
            self.last_status = "Awake lock released.".into();
        }
        self.last_status.clone()
    }
}
