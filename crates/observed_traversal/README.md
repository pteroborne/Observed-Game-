# `observed_traversal`

The **`observed_traversal`** production crate provides deterministic, fixed-timestep kinematic character movement for Observed 2.

It decouples input hardware from character physics, running fixed substeps over Rapier 3D kinematic capsule controllers and AABB collision boundaries.

---

## Core Components & Mechanics

- **`step_body()`**: Advances `FpsBody` by fixed timestep (`FIXED_DT = 1/60s`) given an abstract `PlayerIntent`, evaluating ground acceleration, air control, gravity, jump cooldowns, and collision contacts.
- **`FpsArena` / `ArenaSpec`**: Pure collision schemas for room boundaries, convex hulls (`ColliderSpec`), and static solids (`Aabb3`).
- **`rapier_controller`**: Production Rapier 0.34 kinematic capsule controller with `enhanced-determinism` lockstep guarantees.
- **[`gantry`](src/gantry.rs)**: Phase 40 two-level gantry jump-map model, jump/fall commitment solvers, and understory landing checks.

---

## Testing

Run unit tests and determinism replays:
```bash
cargo test -p observed_traversal
```
