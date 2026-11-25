# Pull Request: Cross-Subnet Peer Discovery, Multi-Peer Sync, IP-First Targets & Primary Failover

## Problems Encountered (3-Node Telemetry: edgebridge v1.0.0-rc.1)
1. **Cross-Subnet Discovery Isolation (`172.21.157.0/24` vs `10.213.54.0/24`):** `Samuel’s MacBook Air` (`172.21.157.163`) scanned only its local `/24` (`Scanned 254 IPs on 172.21.157.0/24 - no peers found`), resulting in `CLUSTER PEERS (0)` even though it received `HEARD PROBE` packets from `plainbox-Inspiron-3881` (`10.213.54.72`) and `samuelphang-HP-Spectre-x360-Convertible` (`10.213.54.197`).
2. **Asymmetric Cluster Peer List & Duplicate Edges:** Primary reported 2 peers while Secondary nodes reported 1 peer, and multiple peers assigned `Edge: left` caused client activation churn.
3. **mDNS / Hostname Resolution Errors:** Non-ASCII apostrophe in `Samuel’s MacBook Air` caused `proto error: unrecognized char`, and `AAAA` DNS lookups for `.local` hostnames failed.
4. **Stale Connection & Portal Session Warnings:** Timeouts on unconnected UDP `4242` targets and D-Bus `Object does not exist` errors on duplicate portal session close calls.
5. **No Primary Failover on Process/Terminal Exit:** Closing the Primary terminal left remaining Secondary nodes without an active Primary or awareness when a newly promoted Primary later disappears.

## Plan & Solution Implemented
- **Cross-Subnet Probe Registration (`src/cluster.rs`):** Register source IPs/hostnames from incoming probes directly into the peer table (`upsert_probed_peer`) even when the sender resides outside the local `/24` subnet, and compute extra `/24` probe prefixes from known peers.
- **Full Cluster Snapshot & Edge Conflict Detection (`src/cluster.rs`):** Broadcast `ClusterSnapshot` (`epoch`, `primary_node_id`, `peers[]`) and detect duplicate `ScreenEdge` assignments deterministically.
- **IP-First Targets & Failover Fencing (`src/cluster.rs`):** Format `lan-mouse` targets with numeric IPs (`format_kvm_target`) and add epoch-fenced heartbeat lease evaluation and deterministic Primary election (`elect_new_primary`).
- **Portable Runtime, Browser Launch & `gh` CLI Support (`src/runtime.rs`):** Resolve binaries (including `lan-mouse` and `gh`) via `PATH` / Homebrew / Cargo without hardcoded user paths or stored secrets, and support detached browser launch (`--open-ui` / `EDGEBRIDGE_OPEN_BROWSER=1`).
