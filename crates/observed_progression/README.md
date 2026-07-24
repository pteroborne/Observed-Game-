# `observed_progression`

The **`observed_progression`** production crate provides profile progression, cosmetic unlocks, matchmaking queue management, and session lobby state machines.

All progression and cosmetic data is strictly orthogonal to simulation mechanics: profiles and cosmetics never alter gameplay stats or collision boundaries.

---

## Submodules

- **[`progression`](src/progression.rs)**: Character profile state (`Profile`), level calculations, XP accrual, cosmetic unlock criteria, slot loadouts, and string serialization.
- **[`session`](src/session/mod.rs)**:
  - **[`lobby`](src/session/lobby.rs)**: Session lifecycle state machine (`SessionPhase`: Lobby, Countdown, InMatch, ReconnectGrace, PostMatch, Closed), readiness checks, host migration, and launch manifests (`LaunchManifest`).
  - **[`matchmaking`](src/session/matchmaking.rs)**: Rating calculations, skill-window matchmaking queues, region pairing (`East`/`West`), and ticket pairing.
  - **[`connection`](src/session/connection.rs)**: Client connection states and account mapping.
  - **[`lan`](src/session/lan.rs)**: Pure four-seat 2v2 LAN lobby with stable `PlayerId`/`TeamId` ownership, team requests, bot fill/takeover, ready countdown, reconnect reservations, late joins, and post-match return.

---

## Testing

Run unit tests:
```bash
cargo test -p observed_progression
```
