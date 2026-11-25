use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Portable config/state directory resolution for fresh clones and multi-user setups.
/// Avoids hardcoded `/home/<username>` paths; falls back to `$HOME/.config/edgebridge`
/// or current working directory if `$HOME` is unset.
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

/// Resolve an external tool binary (such as `lan-mouse`) via `PATH` first, then
/// `$HOME/.cargo/bin` if present, so fresh clones work without hardcoded paths.
pub fn resolve_binary(bin_name: &str) -> PathBuf {
    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return candidate;
            }
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

/// Determines whether the webapp should open automatically in a browser.
pub fn should_open_browser(cli_open_ui: bool) -> bool {
    if cli_open_ui {
        return true;
    }
    matches!(
        env::var("EDGEBRIDGE_OPEN_BROWSER").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

/// Best-effort detached browser launch once the webapp server is bound.
/// Never fails the server or fresh-clone runs if no graphical session or opener exists.
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
