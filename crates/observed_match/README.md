# `observed_match`

The **`observed_match`** production crate contains the core competitive match logic, team standings, AI director pressure systems, elimination series rules, and spatial maze generation.

---

## Submodules

- **[`competition`](src/competition.rs)**: Team standings, race progress tracking, capacity-limited exit checkpoints, and team interference.
- **[`director`](src/director.rs)**: AI Director model managing dynamic pressure, encroaching collapse zones, and catch-up scaling for trailing teams.
- **[`elimination`](src/elimination.rs)**: Multi-round elimination series rules, first-escape countdown timers, and adversary escalation.
- **[`facility`](src/facility.rs)**: Competitive facility state mapping team progress along the route spine.
- **[`mutable`](src/mutable.rs)**: Validates level mutations during live play to guarantee path traversability.
- **[`maze`](src/maze.rs)**: Seeded spatial labyrinth generator translating graph edges into walkable corridor geometry.
- **[`hex_wfc`](src/hex_wfc.rs)**: Canonical hexagonal match adapter managing cell projections, Rapier physics deltas, caged anchors, Guardian setbacks, and bot-POV telemetry.
- **[`full_wfc`](src/full_wfc.rs)**: Legacy square WFC match adapter (regression fixture).
- **[`hybrid`](src/hybrid/)**: First-person hybrid match state machine, round stepping, local/remote player mapping, and replay tape recording.
- **[`teamplay`](src/teamplay.rs)**: Seeded two-member bot teamplay over procedural co-op room beats and tool usage.

---

## Testing

Run unit tests:
```bash
cargo test -p observed_match
```
