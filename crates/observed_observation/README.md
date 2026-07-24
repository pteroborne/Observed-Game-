# `observed_observation`

The **`observed_observation`** production crate establishes the foundational observation and decoherence graph model for Observed 2.

It defines how player presence freezes spatial connections ("observe-to-freeze") while unobserved, unpinned doorways undergo deterministic, seeded rewires upon decoherence pulses.

---

## Core Engine: `ObservationWorld`

- **`authored()`**: Constructs the initial 3×3 room grid lattice with 4 default player positions.
- **`is_pinned(door)`**: Checks if a doorway or its connecting partner is frozen by player observation.
- **`decohere()`**: Deterministically rematches unobserved doorways using `SplitMix`.
- **`traverse(player, side)`**: Moves a player along their room's connected doorway.

---

## Submodules

- **[`contention`](src/contention.rs)**: Submodule managing doorway contention, multi-player observation overlaps, and solvability guards.

---

## Testing

Run unit tests:
```bash
cargo test -p observed_observation
```
