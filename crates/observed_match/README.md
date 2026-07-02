# observed_match

This production crate manages competitive matches, team standing metrics, and the spatial maze layout generators.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Crate entry and exports.
- **[`competition.rs`](src/competition.rs):** Team scoring and standing logs, exit checkpoint lists, and team-specific modifiers.
- **[`director.rs`](src/director.rs):** AI Adversarial Director. Sets dynamic pressure, collapse zones, and scales catch-up speeds for trailing teams.
- **[`facility.rs`](src/facility.rs):** Integrates room graph configurations with team escape points and standing metrics.
- **[`mutable.rs`](src/mutable.rs):** Constrains level mutations during live play to prevent breaking spatial paths.
- **[`maze.rs`](src/maze.rs):** Seeded labyrinth corridor builder that dynamically turns graph connections into physical walls and corridors.
- **[`hybrid/`](src/hybrid/):** First-person hybrid match state, round stepping, local-remote player state mappings, and input tape recording.

## Audit Notes
- **Bloat:** `maze.rs` is still large and should be split if corridor generation grows further.
- **Overlap:** None. `maze.rs` uses `observed_core::SplitMix`.
