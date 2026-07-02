# observed_traversal

This crate implements the deterministic, fixed-timestep movement physics simulation for Observed 2. It resolves movement velocities, ground collisions, step-ups, and climbing traversal mechanics.

## Traversal Mechanics
- **Ground Movement:** Computes gravity, slope deceleration, deceleration dampening, and step-ups over low obstacles.
- **Climbing Traversal:** Handles ladder attachments, ledge-grabbing offsets, ledge pull-ups, and hook grapple selections.
- **AABB Containment:** Resolves body coordinates against rigid wall boundaries using axis-aligned bounding boxes (AABBs).

## Module Structure
- **[`lib.rs`](src/lib.rs):** Integrates the movement tick functions, AABB boundary overlaps, ground constraints, and ledge/ladder transitions.

## Audit Notes
- **Bloat:** `lib.rs` (619 lines) contains a substantial amount of collision and vector mathematics.
- **Overlap:** None.
