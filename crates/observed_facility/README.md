# observed_facility

This crate defines the topology and layouts of individual rooms and hallways. It manages room spawning templates, transform mapping, and geometric overlap constraints.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Crate entry point and test boundary.
- **[`room_def.rs`](src/room_def.rs):** Defines authored room templates (`RoomTemplate`), portal/socket classifications (`PortType`), 4-way direction indicators (`Cardinal`), quarter turns (`QuarterTurn`), and boundary bounding boxes.
- **[`room_world.rs`](src/room_world.rs):** Computes actual transforms for spawning, rotation alignments, attachment points, and overlapping bounds checks.
- **[`constraints.rs`](src/constraints.rs):** Verifies that active transitions do not break connectivity rules across the mutable graph.

## Audit Notes
- **Bloat:** `room_world.rs` contains the ASCII topology parser and world-placement validation. Future additions to layout rules should split standard math helpers from map parsing.
- **Overlap:** None. `constraints.rs` uses `observed_core::SplitMix`, and `Cardinal` is an alias for `observed_core::Direction`.
