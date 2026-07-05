# WFC Procedural Generation Lab

**Question:** can Wave Function Collapse generate a liminal-scale (24-32 room)
`MapSpec` that passes the same validator the hand-authored `sector_relay_v1` map
passes — unique rooms/endpoints, every required role, connectivity, redundancy
(no single-edge-cut anywhere), and objective recovery (keystones/relays/anchor
checkpoints/recovery/dual-stations all survive any single cut)? Phase 45 (Arc D)
answers yes: `observed_facility::wfc::generate_liminal_map` (feature `wfc`, off by
default) produces validated `MapSpec`s deterministically from a seed, and this lab
is where that generator is proven before the game ever adopts it (Phase 46).

The lab has two independent modes sharing one window:

1. **2D WFC feasibility demo** (`main.rs`'s original showcase) — an abstract
   void/corridor/entrance/exit grid solved with `ghx_proc_gen`, unrelated to map
   topology. Press `Space` to randomize its seed.
2. **MapSpec topology visualization** (`map_topology.rs`, Phase 45) — drives the
   real generator and renders the resulting `MapSpec`: rooms as boxes colored by
   `RoomRole`, edges as lines styled by `CorridorRole`, Start/Exit outlined in
   white, with an on-screen legend.

## Controls

- `Space` — randomize the 2D WFC feasibility demo's seed (mode 1, always active
  underneath).
- `M` — toggle MapSpec topology mode on/off.
- `N` / `P` — next/previous seed while in topology mode (regenerates and
  re-renders; a failed seed shows the `WfcMapError` in the legend instead of a
  layout).
- `R` — reset topology mode back to seed 0.

Toggling modes, stepping seeds, or resetting despawns every entity the topology
mode owns (`MapTopologyTile`) before respawning, so it never leaks state across
seeds or into the WFC feasibility demo (per `agents.md`'s reset-and-leak rule).

## The generation pipeline

See `crates/observed_facility/src/wfc.rs`'s module docs for the full six-step
pipeline (WFC room/void layout, dense extraction as full grid adjacency, farthest-
point role spread, deterministic corridor roles, a redundancy/objective-recovery
repair pass, then the `validate()` gate with a salted-seed retry budget). In
short: WFC supplies the seeded stochastic *shape* (which grid cells are rooms);
everything that keeps the graph provably fair (redundancy, objective recovery,
tool-role usefulness) is a deterministic repair pass over the resulting grid
graph, with the retry budget as a safety net rather than the whole strategy.

## Corpus tests — this phase's success criteria

`cargo test -p wfc_proc_gen_lab -- --nocapture` runs the 50-seed corpus in
`src/corpus.rs`:

- **Determinism** — regenerating a seed produces a byte-identical `MapSpec`.
- **Range + dense ids** — every seed generates `Ok` within the retry budget, with
  room count in `[24, 32]` and no gaps in `RoomId(0..n)`.
- **Full validation** — `MapSpec::validate()` passes for every seed.
- **Monitor coverage** — every seed has at least `ceil(room_count / 9)` Monitor
  rooms, enough to page every room at 9-panel density.
- **Objective-sequence coherence** — `CompetitiveFacility::for_map_spec` builds
  cleanly, `objective_sequence()` is non-empty and ends at the exit room, and
  `collapse_rooms()` at the opening purge line is empty (the progress/collapse
  model must not choke on a freshly generated map).
- **Summary table** — a `println!` table of room/edge/monitor counts and
  retry-budget context across all 50 seeds, for manual review.

`crates/observed_facility`'s own `#[cfg(all(test, feature = "wfc"))]` tests cover
a smaller 20-seed slice of the same properties directly against `MapSpec`,
independent of this lab's visualization and `observed_match` dependency.

## `hallway_wfc.rs` — archived, now re-exported (Phase 47 / Arc D5)

`hallway_wfc.rs` used to be a full copy of the game's former hallway-*interior*
WFC generator (archived here in refactor Arc G1, 2026-07-02, when nothing in the
game called it). Phase 47 ported that generator onto the current place-geometry
API as `observed_facility::wfc::generate_interior_walls`/`InteriorSeg` (see that
crate's README), and the game adopted it for `CorridorRole::Mystery` hallway
interiors (`game::wfc_interior`, selected in
`game::teleport::geom::hallway_geom_with_slots_and_role`, DFS-maze fallback on
`WfcInteriorError`). `hallway_wfc.rs` is now a thin re-export of the live
`observed_facility::wfc` functions (plus the old `generate_wfc_interior_walls`
name, for any lab code still spelling it) — its smoke test exercises the real
production code rather than a second, drifting copy.
