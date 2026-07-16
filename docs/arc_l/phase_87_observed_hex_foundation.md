# Phase 87 — `observed_hex` Foundation

**Wave 1 (serial — every later phase depends on this crate).**
Arc context: [../hex_tile_arc_plan.md](../hex_tile_arc_plan.md).

## Goal

Create `crates/observed_hex`: the single source of truth for the hex-prism grid —
coordinates, faces, port classes, quantized metrics, and world mapping. Pure data
and math; no Bevy, no Rapier, no I/O.

## Deliverables

- `crates/observed_hex/src/lib.rs` (+ `coords.rs`, `faces.rs`, `ports.rs`,
  `metrics.rs`), added to the workspace `Cargo.toml`.
- `HexCoord { q: u16, r: u16, level: u8 }` — axial, rhombic bounds. Index math
  `level*cols*rows + r*cols + q` with `index/coord` round-trips (mirror the API
  shape of `full_wfc::config`). Hex distance `(|dq|+|dr|+|dq+dr|)/2` plus a
  weighted level term (vertical steps cost 2× — mirror the square heuristic).
- `HexFace { East=0, NorthEast, NorthWest, West, SouthWest, SouthEast, Up=6,
  Down=7 }` with `ALL`, `opposite()` (lateral `(f+3)%6`, Up↔Down), `delta()`:
  E=(+1,0,0), NE=(+1,−1,0), NW=(0,−1,0), W=(−1,0,0), SW=(−1,+1,0), SE=(0,+1,0),
  Up/Down=(0,0,±1). `is_lateral()` / `is_vertical()`.
- `PortClass { Sealed=0, Door=1, RampOpen=2, ShaftOpen=3 }` and
  `PortSignature(u16)` (2 bits/face, face index × 2 shift). Constructors from
  `[PortClass; 8]`, accessors, validity check (`Door` lateral-only; `RampOpen`/
  `ShaftOpen` vertical-only), and the symmetric per-face compatibility relation
  `ports_compatible(a: PortClass, b: PortClass) -> bool` (equal-class matching:
  Sealed↔Sealed, Door↔Door, RampOpen↔RampOpen, ShaftOpen↔ShaftOpen).
- Quantized metrics in `metrics.rs`: canonical corners
  `(7,-4),(7,4),(0,8),(-7,4),(-7,-4),(0,-8)` (meters, plan view), neighbor pitch
  E=(+14,0) / NE=(+7,−12) / SE=(+7,+12), `ACROSS_FLATS=14.0`,
  `ACROSS_CORNERS=16.0`, `TILE_LEVEL_HEIGHT=8.0`, and
  `hex_origin(HexCoord) -> [f32; 3]` = `(q*14 + r*7, level*8, r*12)`.
  Document the 0.8% diagonal-edge irregularity as intentional.

## Constraints

- No dependency on any other workspace crate except (optionally) `observed_core`
  if SplitMix or shared IDs are genuinely needed — prefer zero deps.
- Keep every file under the 600-line ratchet; no `pub use x::*`.

## Success criteria (tests, all in-crate)

1. `opposite` is an involution over `HexFace::ALL`; `delta(f) + delta(opposite(f)) == 0`.
2. Neighbor round-trip: for interior coords, stepping `f` then `opposite(f)`
   returns the origin; boundary steps return `None`.
3. Tiling identity: for each lateral face, the shared edge midpoint computed from
   a cell's corners equals the same edge midpoint computed from the neighbor's
   corners after `hex_origin` translation — exactly (integer arithmetic), not
   within epsilon.
4. `hex_origin` is injective over a full grid; `index/coord` round-trips.
5. `PortSignature` pack/unpack round-trips all `4^8` signatures or a structured
   sample; invalid class/face combinations are rejected.

## Evidence

Test run output (this phase is pure math — no captures). Note test count in the
hand-back.
