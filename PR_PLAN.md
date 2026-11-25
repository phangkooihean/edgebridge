# Pull Request: Cross-Subnet Peer Discovery, OS Setup Wizards, Wayland/macOS Guidance & Primary Failover

## Problems Encountered (3-Node Telemetry: edgebridge v1.0.0-rc.1)
1. **Cross-Subnet Discovery & UDP `4242` Timeouts:** `Samuel’s MacBook Air` (`172.21.157.163`) vs Ubuntu nodes (`10.213.54.72` and `10.213.54.197`) caused `172.21.157.0/24`-only scans and UDP `4242` timeouts. Connecting all three nodes to a shared network/hotspot aligns subnets cleanly.
2. **Duplicate `Edge: left` & Asymmetric Peer Lists:** Assigning `Edge: left` to multiple peers caused edge/ownership churn, while Secondary nodes lacked full cluster peer visibility.
3. **Ubuntu Wayland Input Capture Prompts & `sudo` Misconception:** GNOME Wayland remote input capture (`xdg-desktop-portal` / `libei`) requires user-session consent ("Allow" / "Share"). Running `edgebridge` or `lan-mouse` with `sudo` does not bypass Wayland portal consent and can break user D-Bus/runtime session alignment; keeping the KVM engine running instead of repeatedly tearing it down reduces re-prompting.
4. **macOS Local Network & Firewall Access:** macOS enforces Local Network and Firewall permissions at the OS level rather than via silent programmatic bypasses, so clear OS wizard/settings prompts and telemetry are needed.
5. **No Primary Failover on Process/Terminal Exit:** Closing a Primary terminal or process lacked deterministic lease-tracked failover among surviving peers.

## Streamlined Experience & Changes Implemented
- **Cross-Subnet Probe Registration & Peer Sync (`src/cluster.rs`):** Retains probed peers across subnets (`upsert_probed_peer`), computes `/24` scan prefixes (`discovery_ipv4_prefixes`), detects duplicate screen edges (`find_edge_conflicts`), formats numeric IP targets (`format_kvm_target`), and supports epoch-fenced failover (`FailoverState`, `elect_new_primary`).
- **OS Setup Wizard Telemetry & Guided Prompts (`src/runtime.rs`):** Tracks whether Ubuntu Wayland/portal or macOS Local Network/Firewall setup wizards were started (`WizardTelemetry`), formats clear non-`sudo` Wayland guidance and macOS System Settings steps, and launches OS settings/wizard helpers safely on demand.
- **Portable Paths & Optional Browser Launch (`src/runtime.rs`):** Resolves config/state via `EDGEBRIDGE_CONFIG_DIR` / `XDG_CONFIG_HOME` / `HOME` and resolves binaries (`lan-mouse`, `gh`, `xdg-open`, `open`) without hardcoded user paths or stored secrets.
