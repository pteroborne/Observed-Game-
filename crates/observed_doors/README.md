# `observed_doors`

The **`observed_doors`** production crate provides pure logic for doors acting as observation gates in the megastructure.

It governs how opening and closing player-operated doors modifies the level's connection graph:
- **Closed Doors**: Hide the connection, making the doorway free to decohere and rewire to a different room.
- **Open Doors**: Observe and freeze both doorways, preventing connection rewiring while open.
- **Protected Spine**: Designates key path connections that remain pinned constantly so the exit stays reachable regardless of random rewires.

---

## Core Engine: `DoorWorld`

- **`authored()`**: Spawns the default 3×3 room grid with a protected spine (`[0, 1, 2, 5, 8]`).
- **`open(door)`**: Opens a door, freezing its current link. Detects if the destination changed since it was last open.
- **`close(door)`**: Closes a door ("slam"), freeing the connection for future decoherence.
- **`traverse(side)`**: Player movement check (blocks traversal through closed or sealed doors).
- **`decohere()`**: Deterministically rematches unpinned, closed doorways using `observed_core::SplitMix`.

---

## Testing

Run unit tests:
```bash
cargo test -p observed_doors
```
