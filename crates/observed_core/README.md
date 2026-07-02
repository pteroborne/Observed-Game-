# observed_core

This foundational production crate establishes the stable newtype identifiers used throughout the entire workspace. It decouples the simulation state from Bevy's internal transient `Entity` values so simulation results can be reliably serialized, replayed, and networked.

## Re-exports
- Re-exports `PlayerId` and `PlayerIntent` from the `player_input` crate.

## Stable Domain Identifiers
- **`RoomId(pub u32)`:** Logical room instances.
- **`PortId(pub u32)`:** Authored connection points (ports/sockets) unique within a room.
- **`EquipmentId(pub u32)`:** Persistent equipment items carried or deployed.
- **`TeamId(pub u8)`:** Player groupings for cooperative coordination and item ownership.

## Audit Notes
- **Bloat:** None (66 lines). Extremely focused.
- **Overlap:** Re-exports `player_input` types to simplify workspace dependencies. A future refactor could centralize shared utilities like the duplicated `SplitMix` PRNG here.
