# Phase 64 — Threshold Geometry Integrity

**Objective:** playtesting found hallway thresholds overlapped with walls or
smashed into random corners — which should be impossible by construction, since
door columns come from the maze/WFC generators and are projected into `DoorGap`s.
Find where the projection and the interior walls disagree, fix it, and teach the
map audit to make this class of defect permanently impossible. Closes bug
backlog #3. Read [README.md](README.md) first.

## Read first

- `game/src/teleport/geom.rs` — hallway interior projection: where interior wall
  segments and `DoorGap`s are both produced; the bug lives in their disagreement.
- `game/src/wfc_interior.rs` + `observed_facility::wfc` (behind the `wfc`
  feature) — WFC labyrinth interiors and their door-column choice; the DFS+braid
  maze path for every other role (Phase 47 notes in ROADMAP describe the split:
  WFC renders only on `CorridorRole::Mystery` edges).
- `game/src/map_validation.rs` (Phase 36) — the semantic map audit: room bounds,
  doorway gaps, polygon wall splits; this is where the new gate lands.
- `docs/bug_backlog.md` #3 for the observed symptoms.

## Approach (reproduce first, fix second, gate third)

1. **Reproduce across seeds.** Write the audit check *first*: for every place
   across the seed corpus (rooms and hallway interiors, DFS and WFC, all
   decoherence versions), assert (a) no threshold rectangle intersects an
   interior wall segment, (b) no two thresholds overlap, (c) every threshold's
   walkable approach is clear on both sides. Run it — the failing seeds/places
   are the reproduction set. Report how many seeds fail and where.
2. **Diagnose.** Likely suspects: door-column choice vs wall-segment splitting
   disagreeing at grid boundaries; hallway variation/reroute versions moving
   walls without moving gaps; scale multipliers (Phase 46 liminal dials)
   shifting one but not the other; WFC fallback disagreeing with the recorded
   door columns.
3. **Fix at the source** — the generator/projection must agree by construction;
   do not nudge thresholds post-hoc to escape walls.
4. **Gate.** The new checks join `map_validation` permanently (and the
   `OBSERVED2_CAPTURE_MAP_AUDIT` evidence path), so `cargo test` fails if the
   class returns.

## Files you may edit

`game/src/teleport/geom.rs`, `game/src/wfc_interior.rs`, `game/src/map_validation.rs`,
`crates/observed_facility` (generator-side fix if the defect is there),
`game/src/tests.rs`. Determinism is sacred: the fix must be a pure function of the
seed; pinned-seed tests (Phase 47's WFC convergence set) must stay green or be
consciously re-pinned with justification in the as-landed note.

## Success criteria

- The new audit checks fail on the pre-fix build for the reproduction seeds
  (recorded in the as-landed note) and pass corpus-wide after the fix.
- Captures of two previously-broken hallway interiors show clean thresholds,
  viewed per the falsifiable-evidence rule.
- Bot routing still works through fixed interiors (existing routing tests green).
- Full verification recipe green, including `--features wfc` clippy+tests.
