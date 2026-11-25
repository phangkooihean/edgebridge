# Pull Request: Multi-Peer Sync, IP-First Targets, Browser UI Launch & Primary Failover

## Problems Encountered (3-Node Telemetry: edgebridge v1.0.0-rc.1)
1. **Asymmetric Cluster Peer List:** Primary (`plainbox-Inspiron-3881`, `10.213.54.72`) reported 2 peers while Secondary (`samuelphang-HP-Spectre-x360-Convertible`, `10.213.54.197`) reported only 1 peer (missing `Samuel’s MacBook Air` at `172.21.157.163`).
2. **Duplicate Edge Assignment:** Multiple peers assigned `Edge: left` on Primary caused client activation/deactivation thrashing.
3. **mDNS / Hostname Resolution Errors:** Non-ASCII/curly apostrophe in `Samuel’s MacBook Air` caused `proto error: unrecognized char`, and `AAAA` queries for `samuelphang-hp-spectre-x360-convertible.` returned no records.
4. **Stale Connection & Portal Session Warnings:** Timeouts to `172.21.157.163:4242`, `not responding` on key release/UDP `4242`, and D-Bus portal session close warnings on already-closed sessions.
5. **No Primary Failover on Process/Terminal Exit:** Closing the Primary terminal killed the Primary engine without promoting a surviving Secondary or alerting remaining peers when the newly promoted Primary also disappears.

## Plan to Fix
- **Git & Workflow Strategy:** Keep `main` clean as the integration branch; implement and commit changes on `release-2` and open a Pull Request into `main`.
- **Cluster Peer Sync & Edge Guard:** Broadcast full `ClusterSnapshot` (`epoch`, `primary`, `peers[]`) from Primary to all peers; deduplicate by `node_id` and surface duplicate edge conflicts.
- **IP-First Targets & Safe Cleanup:** Pass numeric IP addresses for transport targets instead of raw hostnames with special characters; skip `Leave`/modifier calls when no active connection exists; make portal session close idempotent.
- **Heartbeat & Failover (`epoch` Fencing):** Emit periodic heartbeats with lease TTL; when Primary disappears (terminal closed or process killed), remaining Secondary nodes elect a new Primary deterministically and track if that new Primary later disappears.
- **Fresh-Clone & Browser Launch:** Keep fresh git clones portable (no hardcoded `/home/<user>` paths or required GUI/browser during build) while supporting optional browser launch (`--open-ui` / `EDGEBRIDGE_OPEN_BROWSER=1`) once the webapp server is bound.
