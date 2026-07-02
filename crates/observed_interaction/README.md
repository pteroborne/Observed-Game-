# observed_interaction

This production crate manages carried equipment and player-world interactions. It models physical inventories, deployment, and timed control inputs.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Defines API entry points.
- **[`equipment.rs`](src/equipment.rs):** Governing portable item slots, cable spools, structural jacks, batteries, carrying coordinates, and drop behaviors.
- **[`src/interaction/model.rs`](src/interaction/model.rs):** Pure data structs representing active actors, target sockets, interaction policies, and event states.
- **[`src/interaction/engine.rs`](src/interaction/engine.rs):** Deterministic state machine resolving ticks for activations, holds, contention over a single target, and quorum overrides.
- **[`src/interaction/mod.rs`](src/interaction/mod.rs):** Module setup and exports.

## Audit Notes
- **Bloat:** `equipment.rs` (876 lines) and `engine.rs` (530 lines) are large, but their segregation of data (`model.rs`) from transitions (`engine.rs`) conforms strictly to pure simulation guidelines.
- **Overlap:** None.
