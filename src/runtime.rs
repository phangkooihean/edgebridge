use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Tracks whether OS permission / setup wizards have been started on this node
/// so telemetry clearly reflects wizard activation state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WizardTelemetry {
    pub ubuntu_wayland_wizard_started: bool,
    pub macos_network_wizard_started: bool,
    pub last_wizard_action: Option<String>,
    pub last_wizard_note: Option<String>,
}

impl WizardTelemetry {
    pub fn mark_ubuntu_wayland_wizard(&mut self, note: impl Into<String>) {
        self.ubuntu_wayland_wizard_started = true;
        self.last_wizard_action = Some("ubuntu_wayland_permissions".to_string());
        self.last_wizard_note = Some(note.into());
    }

    pub fn mark_macos_network_wizard(&mut self, note: impl Into<String>) {
        self.macos_network_wizard_started = true;
        self.last_wizard_action = Some("macos_local_network_firewall".to_string());
        self.last_wizard_note = Some(note.into());
    }

    pub fn summary_line(&self) -> String {
        let ubuntu_state = if self.ubuntu_wayland_wizard_started {
            "STARTED"
        } else {
            "NOT_STARTED"
        };
        let macos_state = if self.macos_network_wizard_started {
            "STARTED"
        } else {
            "NOT_STARTED"
        };
        let detail = self
            .last_wizard_note
            .as_deref()
            .unwrap_or("no wizard launched yet");
        format!("WizardStarted[UbuntuWayland={ubuntu_state}, macOSNet={macos_state}] | {detail}")
    }
}

/// Concise guidance explaining why `sudo` is not used for Wayland input capture
/// and how to keep the user session & engine running to avoid repeated prompts.
pub fn ubuntu_wayland_guidance() -> &'static str {
    "Ubuntu Wayland remote input capture uses your user desktop session portal (Allow/Share). \
Do NOT run edgebridge/lan-mouse with sudo; keep the session/engine running instead of restarting repeatedly."
}

/// Concise guidance for macOS Local Network / Firewall and shared subnet/hotspot setup.
pub fn macos_network_guidance() -> &'static str {
    "Connect all nodes to the same network/hotspot and enable Local Network + Firewall access \
for your terminal/edgebridge in macOS System Settings (no silent OS security bypass)."
}

/// Portable config/state directory resolution for fresh clones and multi-user setups.
pub fn resolve_config_dir() -> PathBuf {
    if let Ok(custom) = env::var("EDGEBRIDGE_CONFIG_DIR") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        if !xdg.trim().is_empty() {
            return PathBuf::from(xdg).join("edgebridge");
        }
    }
    if let Ok(home) = env::var("HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home).join(".config").join("edgebridge");
        }
    }
    PathBuf::from(".edgebridge-config")
}

/// Resolve an external tool binary via `PATH` first, then standard locations
/// without hardcoding usernames or storing credentials.
pub fn resolve_binary(bin_name: &str) -> PathBuf {
    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    for sys_dir in ["/opt/homebrew/bin", "/usr/local/bin"] {
        let candidate = PathBuf::from(sys_dir).join(bin_name);
        if candidate.is_file() {
            return candidate;
        }
    }
    if let Ok(home) = env::var("HOME") {
        let cargo_bin = PathBuf::from(home).join(".cargo").join("bin").join(bin_name);
        if cargo_bin.is_file() {
            return cargo_bin;
        }
    }
    PathBuf::from(bin_name)
}

pub fn is_gh_available() -> bool {
    let gh_bin = resolve_binary("gh");
    Command::new(gh_bin)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Prompt/start the appropriate OS setup wizard and update `WizardTelemetry`.
/// On Ubuntu, opens GNOME Remote Desktop / Privacy settings if available and prints
/// non-`sudo` Wayland guidance. On macOS, opens Privacy & Security / Firewall settings
/// if available and prints Local Network / hotspot guidance.
pub fn prompt_os_setup_wizard(wizard: &mut WizardTelemetry) -> String {
    #[cfg(target_os = "linux")]
    {
        wizard.mark_ubuntu_wayland_wizard(ubuntu_wayland_guidance());
        if env::var_os("DISPLAY").is_some() || env::var_os("WAYLAND_DISPLAY").is_some() {
            let opener = resolve_binary("gnome-control-center");
            if opener.is_file() {
                let _ = Command::new(opener)
                    .arg("remote-desktop")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
            }
        }
        eprintln!("wizard[ubuntu-wayland]: {}", ubuntu_wayland_guidance());
    }

    #[cfg(target_os = "macos")]
    {
        wizard.mark_macos_network_wizard(macos_network_guidance());
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_LocalNetwork")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        eprintln!("wizard[macos-network]: {}", macos_network_guidance());
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        wizard.mark_ubuntu_wayland_wizard(ubuntu_wayland_guidance());
        eprintln!("wizard[info]: {}", ubuntu_wayland_guidance());
    }

    wizard.summary_line()
}

pub fn should_open_browser(cli_open_ui: bool) -> bool {
    if cli_open_ui {
        return true;
    }
    matches!(
        env::var("EDGEBRIDGE_OPEN_BROWSER").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

pub fn launch_webapp_browser(url: &str, cli_open_ui: bool) {
    if !should_open_browser(cli_open_ui) {
        return;
    }

    #[cfg(target_os = "linux")]
    let has_display = env::var_os("DISPLAY").is_some() || env::var_os("WAYLAND_DISPLAY").is_some();
    #[cfg(target_os = "linux")]
    if !has_display {
        eprintln!("info: skipping browser launch for {url} (no DISPLAY or WAYLAND_DISPLAY)");
        return;
    }

    #[cfg(target_os = "linux")]
    let (opener, args): (&str, Vec<&str>) = ("xdg-open", vec![url]);
    #[cfg(target_os = "macos")]
    let (opener, args): (&str, Vec<&str>) = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let (opener, args): (&str, Vec<&str>) = ("cmd", vec!["/C", "start", "", url]);

    let result = Command::new(opener)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    if let Err(err) = result {
        eprintln!("info: could not launch browser via {opener} ({err}); open {url} manually");
    }
}
