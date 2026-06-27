# Room Lab

The Room Lab proves an authored modular-room pipeline. Logical rooms use stable
`RoomId` and `PortId` values and exist independently from their Bevy rendering
entities. Definitions contain explicit bounds, typed ports, and collision
surfaces.

## Authored vocabulary

- Straight corridor
- Corner
- Junction
- Control room
- Machine chamber
- Freight room
- Shaft
- Platform room

The initial facility contains one instance of every template connected into a
single seven-edge chain.

## Connection rules

A connection is accepted only when:

1. Both rooms and ports exist.
2. Neither port is already occupied.
3. Port types are equal.
4. World positions coincide.
5. Facings are opposite.

Attaching a room computes the required quarter-turn rotation and translation
from those constraints. Replacement aligns the new definition to an existing
neighbor where possible, regenerates collision geometry, and reconnects only
ports that still validate.

## Controls

- `Tab`: select next room
- `P`: select next free port
- `Q` / `E`: rotate selected room left/right
- `C`: spawn and connect a compatible room at the selected free port
- `T`: replace selected room with the next template
- `X`: despawn selected room and all room-owned entities
- `R`: reset the authored eight-room facility
- `F1`: toggle room bounds, ports, connections, and collision debug geometry

## Debug visualization

- Cyan/selected-yellow room bounds
- Port-type-colored circles and facing lines
- Green validated connections
- Red collision rectangles
- Logical room, visual root, owned-room, connection, collision, and lifecycle
  counters

## Success conditions

1. All eight definitions load with bounds, ports, and collision surfaces.
2. Spawning creates one logical room and one room-owned visual hierarchy.
3. Despawning removes its logical instance, connections, and every owned entity.
4. Quarter-turn rotation transforms bounds, ports, facings, and collisions.
5. Attached rooms align ports exactly with opposite facings.
6. Invalid type, position, facing, and occupancy combinations are rejected.
7. Collision geometry is generated from authored surfaces and tagged by room.
8. Replacement updates the same stable `RoomId`, refreshes its visual hierarchy,
   and preserves only connections that remain valid.
9. Repeated reset restores exactly eight logical rooms, eight visual roots,
   seven connections, and one UI root.

## Manual verification

1. Run `cargo run -p room_lab`.
2. Use `Tab` and `P` to inspect each room and its free typed ports.
3. Press `C` to attach a compatible room. Confirm the two port markers coincide
   and the green connection appears.
4. Rotate the selected room with `Q` or `E`. Confirm its bounds, ports, collision
   geometry, and sprites rotate together and invalid connections disappear.
5. Press `T` several times. Confirm the stable room number remains while the
   template, collision count, and visual hierarchy update.
6. Press `X`. Confirm logical/visual counts decrease and no connection references
   the deleted room.
7. Press `R` at least five times. The lifecycle monitor must remain `[PASS]`
   with eight rooms, eight visual roots, seven connections, and eight owned room
   IDs.
