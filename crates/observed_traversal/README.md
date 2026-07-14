# observed_traversal

This crate implements deterministic, fixed-timestep movement for Observed 2. `ArenaSpec` is the stable collision input and `TraversalWorld` is the temporary dual-backend runtime seam used while the game migrates from its characterized AABB controller to raw Rapier 0.34 with `enhanced-determinism`.

## Traversal Mechanics
- **Ground Movement:** Computes gravity, slope deceleration, deceleration dampening, and step-ups over low obstacles.
- **Climbing Traversal:** Handles ladder attachments, ledge-grabbing offsets, ledge pull-ups, and hook grapple selections.
- **AABB Containment:** Resolves body coordinates against rigid wall boundaries using axis-aligned bounding boxes (AABBs).
- **Authored Convex Traversal:** Runs the same `PlayerIntent`/`FpsBody` boundary through Rapier's kinematic character controller over stable cuboid or convex-hull collider IDs. Controller skin, autostep, slope, and ground-snap values come from `FpsConfig`.
- **Gantry Jump Maps:** Provides the pure Phase 40 two-level hallway model, deterministic clean-jump / fall-recover / safe-bypass route runner, and lower-floor solvability check used by `gantry_lab`.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Integrates the movement tick functions, AABB boundary overlaps, ground constraints, and ledge/ladder transitions.
- **[`gantry.rs`](src/gantry.rs):** Authored gantry dimensions, platform thresholds, deterministic bot route simulation, timing targets, and lower-floor flood-fill validation.
- **[`world.rs`](src/world.rs):** Pure arena/collider schema, backend selector, raw Rapier world ownership, and bit-exact replay tests.

## Audit Notes
- **Bloat:** `lib.rs` (619 lines) contains a substantial amount of collision and vector mathematics.
- **Overlap:** None.
