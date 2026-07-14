# observed_net

This production crate implements the deterministic lockstep multiplayer networking protocol. It manages lossy UDP packet repairs, frame validation hashes, simulation-content compatibility, and client-server sync messages.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Entry point.
- **[`protocol.rs`](src/protocol.rs):** Defines Status messages containing inputs (`WireIntent`), the simulation-content SHA-256, and checksum validation. Peers with different movement/collision content are rejected before a frame commits.
- **[`network.rs`](src/network.rs):** Manages packet repair buffers, frame sequencing, duplicate handling, and simulated transport delay/jitter.
- **[`netmatch.rs`](src/netmatch.rs):** Bridges networked lockstep frames into matching client-side simulation updates.

## Audit Notes
- **Bloat:** `netmatch.rs` (688 lines) and `network.rs` (634 lines) are substantial.
- **Overlap:**
  - `PacketError` enum was unified into the crate-level root `lib.rs` and is imported across both `netmatch.rs` and `protocol.rs`.
