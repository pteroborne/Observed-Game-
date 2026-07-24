# `observed_facility`

The **`observed_facility`** production crate defines the physical topology, room/corridor layouts, and Wave Function Collapse (WFC) generation systems for the continuous megastructure.

---

## Submodules

- **[`room_def`](src/room_def.rs)**: Authored room templates (`RoomTemplate`), typed port/socket classifications (`PortType`), quarter-turn rotations, and AABB bounds.
- **[`room_world`](src/room_world.rs)**: Computes physical spawning transforms, portal rotation alignments, auto-attachment math, and spatial collision checks.
- **[`map_spec`](src/map_spec.rs)**: Graph definitions (`MapSpec`) for racecourses, roles (`Start`, `Key`, `Exit`), and topology validation rules (`validate()`).
- **[`hex_wfc`](src/hex_wfc.rs)**: Canonical hexagonal Wave Function Collapse solver (`HexWfcWorld`), handling multi-level cell collapses, blueprint/ramp/shaft pinning, and bounded mutation.
- **[`full_wfc`](src/full_wfc.rs)** *(feature `wfc`)*: Legacy square lattice (8×5×3) WFC solver, retained as a regression fixture.
- **[`wfc`](src/wfc.rs)** *(feature `wfc`)*: Procedural liminal map topology generator (`generate_liminal_map_v2`) and hallway interior wall generator.
- **[`constraints`](src/constraints.rs)**: Validates route-spine constraints over changing level graphs.
- **[`junction`](src/junction.rs)**: Intersection and threshold junction alignment logic.

---

## Verification & Tests

Run standard facility tests:
```bash
cargo test -p observed_facility
```

Run extended WFC collapse tests:
```bash
cargo test -p observed_facility --features wfc extended_default_corpus -- --ignored
```
