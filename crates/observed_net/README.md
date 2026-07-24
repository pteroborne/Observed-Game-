# `observed_net`

The **`observed_net`** production crate implements deterministic lockstep multiplayer networking, wire protocol serialization, packet repair buffers, and frame checksum validation.

---

## Submodules

- **[`protocol`](src/protocol.rs)**: Fixed-size binary wire serialization (`WireIntent`), status packet headers, SHA-256 simulation-content validation, and packet checksum checks.
- **[`network`](src/network.rs)**: Lockstep frame synchronizer, managing packet loss repair, out-of-order packet reordering, and simulated transport delay/jitter.
- **[`netmatch`](src/netmatch.rs)**: Client-server match state synchronization and lockstep frame execution over production controller instances.
- **[`lan`](src/lan.rs)**: Versioned/checksummed UDP datagrams, broadcast discovery, direct-address clients, lobby commands/snapshots, redundant input bundles, authoritative frame windows, acknowledgements, reconnect tokens, and deterministic history resync.
- **[`PacketError`](src/lib.rs)**: Unified error enum for network packet validation.

---

## Testing

Run unit tests:
```bash
cargo test -p observed_net
```
