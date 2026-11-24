# EdgeBridge (`v1.0.0-rc.1`)

![License: MIT](https://img.shields.io/badge/License-MIT-34d399.svg)
![Rust](https://img.shields.io/badge/Rust-2021_Edition-f97316.svg)
![Platforms](https://img.shields.io/badge/Platforms-macOS_%7C_Ubuntu_%7C_Fedora_%7C_Windows-38bdf8.svg)
![CI](https://github.com/phangkooihean/edgebridge/actions/workflows/ci.yml/badge.svg)

**EdgeBridge** is a local-first, single-binary Rust software KVM orchestrator for **macOS**, **Ubuntu/Debian**, **Fedora**, and **native Windows**. It unifies multiple computers on the same LAN or phone hotspot into one magnetic multi-monitor workspace—complete with real-time pointer/keyboard responsiveness diagnostics and **Shared Cross-Node Telemetry**.

Repository: <https://github.com/phangkooihean/edgebridge>

---

## Why EdgeBridge Exists & AI-Assisted Learning Disclosure

Multi-machine desks break down when cursor crossing stutters on congested Wi-Fi, HiDPI display layouts clash, or debugging requires jumping between terminals on separate machines. EdgeBridge solves this with a **20-sample jitter/latency responsiveness monitor** and **Shared Cross-Node Telemetry** (`/api/telemetry/raw`) that pulls peer diagnostics into a single view—a pattern that informs the other diagnostic and multi-device tools I am building.

> **AI-Assisted Development Note:** This project was built with extensive assistance from **Google Neon**. Because AI made rapid iteration and rebuilding from scratch inexpensive, I rebuilt repeatedly to study how systems fit together—focusing on Rust's module boundaries, `///` documentation tests coupled with invariant assertions (`cargo test`), cross-platform OS integration, and systems-design trade-offs. EdgeBridge runs 100% locally with zero cloud dependency; all future changes are developed through feature branches and Pull Requests so other builders can clone the repo, take it apart to learn, and contribute.

---

## Architecture & Rust Stack

| Layer | Crate / Technology | Role in EdgeBridge |
| :--- | :--- | :--- |
| **Web & Control Plane** | `axum` + `tokio` | Async HTTP/JSON API, `/24` LAN subnet scanner, and peer state orchestrator on TCP `48720` |
| **Embedded Browser UI** | `include_str!("web/index.html")` | Single-file HTML/CSS/JS magnetic display canvas & live telemetry console (zero Node/Electron bloat) |
| **Serialization & Contracts** | `serde` + `serde_json` | Typed peer handshakes, layout mirroring, and shared cross-node telemetry payloads |
| **OS Platform Backends** | `#[cfg(target_os)]`, `windows-sys`, `libc` | Native multi-monitor layout, awake locks, firewall checks, and OS permission guidance |
| **KVM Data Plane** | External `lan-mouse` binary | Encrypted DTLS pointer/keyboard transport over UDP `4242` (`[A+S+D+F]` emergency release) |
| **Executable Docs** | `cargo test` (`///` doctests) | Unit tests embedded directly in module doc comments to enforce invariants |

---

## Quick Start: Build Locally from Scratch

Each platform block below installs missing prerequisites (`git`, Rust `cargo`, and `lan-mouse`), clones the repo, runs `cargo test`, and compiles a standalone release binary.

### 1. macOS (Built-in Terminal — `zsh`)

```zsh
# Install Homebrew & prerequisites if missing
command -v brew >/dev/null 2>&1 || /bin/bash -c "$(curl -fsSL [https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh](https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh))"
brew list git >/dev/null 2>&1 || brew install git openssl
command -v cargo >/dev/null 2>&1 || curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh -s -- -y
. "$HOME/.cargo/env"
command -v lan-mouse >/dev/null 2>&1 || cargo install lan-mouse

# Clone, test & build
git clone [https://github.com/phangkooihean/edgebridge.git](https://github.com/phangkooihean/edgebridge.git) "$HOME/edgebridge"
"$HOME/edgebridge/build.sh"
"$HOME/edgebridge/target/release/edgebridge"
```

> **macOS Accessibility Tip:** At Step 3, open **System Settings -> Privacy & Security -> Accessibility** (and **Input Monitoring**), press `Cmd + Shift + .` inside the `+` file picker to reveal `~/.cargo/bin/lan-mouse`, toggle both **Terminal** and **`lan-mouse`** ON, quit Terminal (`Cmd + Q`), and restart EdgeBridge.

---

### 2. Ubuntu / Debian (Default Terminal — `apt`)

```bash
# Install build dependencies, Rust & lan-mouse
sudo apt update && sudo apt install -y git curl openssl build-essential libnotify-bin pkg-config libadwaita-1-dev libgtk-4-dev libx11-dev libxtst-dev
command -v cargo >/dev/null 2>&1 || curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh -s -- -y
. "$HOME/.cargo/env"
command -v lan-mouse >/dev/null 2>&1 || cargo install lan-mouse

# Clone, test & build
git clone [https://github.com/phangkooihean/edgebridge.git](https://github.com/phangkooihean/edgebridge.git) "$HOME/edgebridge"
"$HOME/edgebridge/build.sh"
"$HOME/edgebridge/target/release/edgebridge"
```

---

### 3. Fedora Workstation (Default Terminal — `dnf`)

```bash
# Install build dependencies, Rust & lan-mouse
sudo dnf install -y git curl openssl gcc gcc-c++ make libnotify pkgconf-pkg-config libadwaita-devel gtk4-devel libX11-devel libXtst-devel
command -v cargo >/dev/null 2>&1 || curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh -s -- -y
. "$HOME/.cargo/env"
command -v lan-mouse >/dev/null 2>&1 || cargo install lan-mouse

# Clone, test & build
git clone [https://github.com/phangkooihean/edgebridge.git](https://github.com/phangkooihean/edgebridge.git) "$HOME/edgebridge"
"$HOME/edgebridge/build.sh"
"$HOME/edgebridge/target/release/edgebridge"
```

> **Ubuntu & Fedora GNOME Wayland Tip (CLI & Web UI):** When Step 3 starts `lan-mouse`, GNOME displays a system consent pop-up. Toggle **"Allow Remote Input Capturing"** ON and click **"Share"**. Locking the screen revokes Wayland portals until you click **Stop Sharing** and **Next**.

---

### 4. Native Windows 10 / 11 (Windows Terminal / PowerShell — `winget`)

```powershell
# Install Git, Rust, MSVC C++ Build Tools & lan-mouse
winget install --id Git.Git -e --source winget
winget install --id Rustlang.Rustup -e --source winget
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --source winget --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
cargo install lan-mouse

# Clone, test & build
git clone [https://github.com/phangkooihean/edgebridge.git](https://github.com/phangkooihean/edgebridge.git) "$HOME\edgebridge"
& "$HOME\edgebridge\build.ps1"
& "$HOME\edgebridge\target\release\edgebridge.exe"
```

---

## Wish List, Bug Tracker & Contributing

* **Wish List & Bugs:** Open an issue in GitHub's native **[Issues](https://github.com/phangkooihean/edgebridge/issues)** tab using the *Wish List* or *Bug Report* templates.
* **Pull Requests:** Fork the repository, create a feature branch (`git checkout -b feat/your-idea`), verify `cargo test` passes, and open a **[Pull Request](https://github.com/phangkooihean/edgebridge/pulls)**.
