# observed_progression

This production crate manages client cosmetics, profiles, matchmaking status, and multiplayer game lobbies.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Crate exports.
- **[`progression.rs`](src/progression.rs):** Handles profile levels, character cosmetics, equipment unlocks, and serialization formats.
- **[`session.rs`](src/session.rs):** Handles matchmaking queues, local player allocations, team distributions, lobby ready checks, and rematch states.

## Audit Notes
- **Bloat:** `session.rs` is over 1,000 lines long. It concurrently manages connection states, matchmaking errors, lobby assignments, and game configurations. It is recommended to split it into `session/matchmaking.rs` and `session/lobby.rs`.
- **Overlap:** None.
