# EdgeBridge (`v1.0.0-rc.1`) — Multi-Platform LAN KVM Orchestrator

**EdgeBridge** is an MIT-licensed, local-first Rust software KVM orchestrator for **macOS**, **Ubuntu/Debian Linux**, **Fedora Linux**, and **native Windows**. It lets you move your mouse and keyboard across paired machines via an embedded browser UI (`http://127.0.0.1:48720`) with magnetic display alignment, pointer/keyboard responsiveness health, and **Shared Cross-Node Telemetry** that informs the other diagnostic and multi-device tools I am building.

Builder profile: [https://builder-hub.rakyatdigital.gov.my/profile](https://builder-hub.rakyatdigital.gov.my/profile)  
GitHub repository: [https://github.com/phangkooihean/edgebridge](https://github.com/phangkooihean/edgebridge)

## AI-Assisted Learning Disclosure
> *This project was built with extensive assistance from **Google Neon**. Because AI made rapid iteration and rebuilding from scratch inexpensive, I rebuilt repeatedly to learn how systems fit together—focusing on Rust's module boundaries, `///` documentation tests coupled with invariant assertions, cross-platform OS integration, and practical system-design thinking. It works locally with zero cloud or GitHub dependency; future changes are planned through Pull Requests so other builders can pull the code, take it apart to learn, and contribute.*

## Why Rust & an Embedded Web Stack?
- **Single self-contained binary & OS reach:** Rust compiles cleanly into a single native binary with `#[cfg(target_os = "...")]` platform modules for macOS, Linux, and Windows, coordinating an external `lan-mouse` KVM engine over encrypted DTLS (`4242/udp`).
- **Executable documentation (`///` doctests):** Rust bundles unit tests inside `///` doc comments so invariants are verified directly by `cargo test` alongside the docs you read.
- **Lean embedded browser UI:** Instead of shipping a heavy Electron/Node runtime, EdgeBridge embeds a single-file HTML/CSS/JS frontend (`include_str!`) served by **`axum`** on **`tokio`**, with **`serde` / `serde_json`** handling typed peer state and shared telemetry.

## Native GitHub Wish List & Bug Tracker
- Use GitHub's native **Issues** tab as the project **wish list** and **bug tracker**, and **Pull Requests** for proposed changes or critiques. No separate external issue tracker is required.

## Local Build & Setup for Novices (No Pre-Installed Git or Rust Required)

### macOS (Built-in Terminal)
  brew list git >/dev/null 2>&1 || brew install git
  command -v cargo >/dev/null 2>&1 || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
  command -v lan-mouse >/dev/null 2>&1 || cargo install lan-mouse
  "$HOME/edgebridge/build.sh"

* macOS Permission Tip: Open System Settings -> Privacy & Security -> Accessibility (and Input Monitoring), press Cmd+Shift+. in the + picker to reveal ~/.cargo/bin/lan-mouse, toggle Terminal and lan-mouse ON, quit Terminal (Cmd+Q), and restart EdgeBridge.

### Ubuntu / Debian (Default Terminal - apt)
  sudo apt update && sudo apt install -y git curl openssl build-essential libnotify-bin pkg-config libadwaita-1-dev libgtk-4-dev libx11-dev libxtst-dev
  command -v cargo >/dev/null 2>&1 || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
  command -v lan-mouse >/dev/null 2>&1 || cargo install lan-mouse
  "$HOME/edgebridge/build.sh"

### Fedora (Default Terminal - dnf)
  sudo dnf install -y git curl openssl gcc gcc-c++ make libnotify pkgconf-pkg-config libadwaita-devel gtk4-devel libX11-devel libXtst-devel
  command -v cargo >/dev/null 2>&1 || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
  command -v lan-mouse >/dev/null 2>&1 || cargo install lan-mouse
  "$HOME/edgebridge/build.sh"

* Ubuntu & Fedora GNOME Pop-Up Tip (CLI & Webapp): When Step 3 triggers GNOME's Wayland permission prompt, toggle "Allow Remote Input Capturing" ON and click "Share". Locking the screen revokes the portal until you click Stop Sharing and Next.

### Native Windows (PowerShell + winget, No WSL)
  winget install --id Git.Git -e --source winget
  winget install --id Rustlang.Rustup -e --source winget
  winget install --id Microsoft.VisualStudio.2022.BuildTools -e --source winget --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  cargo install lan-mouse
  & "$HOME\edgebridge\build.ps1"

## Publishing to GitHub (phangkooihean)
  cd ~/edgebridge
  gh auth login
  gh repo create phangkooihean/edgebridge --public --source=. --remote=origin --push --tags
