# observed_facility

This crate defines the topology and layouts of individual rooms and hallways. It manages room spawning templates, transform mapping, and geometric overlap constraints.

## Module Structure
- **[`lib.rs`](src/lib.rs):** Crate entry point and test boundary.
- **[`room_def.rs`](src/room_def.rs):** Defines authored room templates (`RoomTemplate`), portal/socket classifications (`PortType`), 4-way direction indicators (`Cardinal`), quarter turns (`QuarterTurn`), and boundary bounding boxes.
- **[`room_world.rs`](src/room_world.rs):** Computes actual transforms for spawning, rotation alignments, attachment points, and overlapping bounds checks.
- **[`constraints.rs`](src/constraints.rs):** Verifies that active transitions do not break connectivity rules across the mutable graph.
- **[`map_spec.rs`](src/map_spec.rs):** `MapSpec` — the semantic racecourse graph (rooms, roles, edges) and its validator (`validate()`: unique rooms/endpoints, required roles, connectivity, redundancy, objective recovery, tool-role usefulness). `sector_relay_v1()` is the one hand-authored map.
- **[`wfc.rs`](src/wfc.rs):** *(feature `wfc`, off by default)* Wave Function Collapse liminal-map generation — produces validated `MapSpec` output at 24-32 rooms. See below.

## `wfc` feature (Phase 45 / Arc D)

- **Problem:** `sector_relay_v1` is a hand-authored 14-room proof-of-concept map. Arc D
  needs liminal-scale facilities (24-32 rooms) and hand-authoring at that scale doesn't
  hold up; the map needs to be generated, deterministically, per seed.
- **Smallest adapter:** one module (`src/wfc.rs`), one optional dependency
  (`ghx_proc_gen`), gated behind the `wfc` Cargo feature (not in `default`). It plugs
  into the same `MapSpec`/`validate()` contract every other map already satisfies —
  nothing downstream needs to know the map was generated rather than authored.
- **Fallback intact:** the `wfc` feature is off by default, so every existing
  labs/game consumer is unaffected. `sector_relay_v1()` remains the only map the game
  builds against until Phase 46 explicitly flips the default in `game::map_catalog`.
- **Proof:** `wfc_proc_gen_lab` visualizes generated maps and runs the corpus tests
  (50 seeds: determinism, room-count range, role coverage, `validate()` passing,
  objective-sequence coherence against `observed_match::CompetitiveFacility`). This
  crate's own `#[cfg(all(test, feature = "wfc"))]` tests cover a smaller 20-seed
  corpus plus determinism/diversity checks directly against `MapSpec`.

## Audit Notes
- **Bloat:** `room_world.rs` contains the ASCII topology parser and world-placement validation. Future additions to layout rules should split standard math helpers from map parsing.
- **Overlap:** None. `constraints.rs` uses `observed_core::SplitMix`, and `Cardinal` is an alias for `observed_core::Direction`. `wfc.rs` also uses `observed_core::SplitMix` for every tie-break/shuffle (never `HashMap`/`HashSet` iteration order), so it stays deterministic alongside the rest of the crate.
