# `observed_interaction`

The **`observed_interaction`** production crate provides pure, engine-independent state machines for equipment management and interactive mechanisms in Observed 2.

It models carried items (batteries, structural jacks, light spools) and deterministic tick-based interactions (activations, hold channels, item contention, co-op quorum gates, and interruptions).

---

## Submodules

- **[`equipment`](src/equipment.rs)**: Persistent equipment models (`EquipmentItem`, `EquipmentState`: Carried, Deployed, Socketed, Ground), inventory slots, cable spools, and drop/pickup physics bounds.
- **[`interaction`](src/interaction/mod.rs)**:
  - **[`model`](src/interaction/model.rs)**: Data types for interaction targets, sockets, hold policies (`Instant`, `Hold`, `Quorum`), and actor states.
  - **[`engine`](src/interaction/engine.rs)**: Deterministic tick resolution engine for player interactions, handling exclusive target contention, hold timers, and interruptions.

---

## Testing

Run unit tests:
```bash
cargo test -p observed_interaction
```
