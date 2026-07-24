# `observed_core`

The **`observed_core`** production crate provides fundamental domain identifiers, directional enums, and PRNG logic for the Observed 2 workspace.

By representing game entities with durable newtypes instead of transient engine `Entity` values, simulation state remains fully serializable, testable, and networkable.

---

## Domain Identifiers

- **`RoomId(pub u32)`**: Stable identifier for a logical room.
- **`CorridorId(pub u32)`**: Stable identifier for a corridor instance.
- **`PlaceId`**: Enum distinguishing `Room(RoomId)` from `Corridor(CorridorId)`.
- **`ThresholdId` / `ThresholdSlotId`**: Authored threshold socket identifiers.
- **`PortId(pub u32)`**: Authored connection points unique within a room.
- **`EquipmentId(pub u32)`**: Persistent equipment items (batteries, jacks, spools).
- **`TeamId(pub u8)`**: Team groupings for competitive/cooperative match rules.
- **`PlayerId` & `PlayerIntent`**: Re-exported from `player_input`.

---

## Submodules

- **[`prng`](src/prng.rs)**: Standardized `SplitMix` pseudo-random number generator used across simulation crates.
- **[`direction`](src/direction.rs)**: Unified compass direction enums (`Direction`).

---

## Testing

Run unit tests:
```bash
cargo test -p observed_core
```
