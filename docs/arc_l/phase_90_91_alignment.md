# Phase 90 & Phase 91 Alignment: Room Blueprints & Tile Signatures

This document coordinates the room blueprint definitions between the Phase 90 WFC solver implementation and the Phase 91 Tile Library authoring.

## Resolved port model (Phase 90 implementation, 2026-07-17)

The catalog below is the frozen contract and matches the implementation in
`crates/observed_facility/src/hex_wfc/blueprint.rs`. Two clarifications the
solver now enforces, for Phase 91 to author against:

- **Exterior openings = every lateral face that does not border another
  footprint cell.** A single-hex room therefore presents `Door` on all six
  lateral faces (minus any that leave the grid, which the stamper seals);
  multi-hex rooms present `Door` on their whole outer boundary and `Sealed`
  on the interior faces shared between footprint cells. This is what the
  original catalog meant by "all other lateral faces are `Door`".
- **`named_ports` are a labeled subset of those exterior doors** — used for
  stable `ThresholdKey { room, port }` identity, *not* a restriction of which
  faces open. Phase 91 keys threshold prefabs off these names.
- **Identity:** `StampedBlueprint::generation_key()` = `hash(blueprint role,
  anchor cell)`; stable across relayouts, mirrors
  `full_wfc::catalog::corridor_generation_key` discipline.
- **Solver demand feed:** `observed_facility::hex_wfc::demandable_signatures()`
  returns every `PortSignature` the solver's variant alphabet can require
  (rooms, halls, junctions, ramp halves, shaft segments). This is the input to
  the Phase 91 coverage validator.

## Blueprint Footprints & Named Ports

The WFC solver expects the following relative coordinates, named ports, and port signatures for the room blueprints stamped into the grid:

### 1. `RoomRole::Start`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: All 6 lateral faces are `Door` (`PortClass::Door`). Vertical ports are `Sealed`.
- **Named ports**:
  - `"entrance"` at `(0, 0, 0)` face `West`
  - `"exit"` at `(0, 0, 0)` face `East`

### 2. `RoomRole::Exit`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: All 6 lateral faces are `Door` (`PortClass::Door`). Vertical ports are `Sealed`.
- **Named ports**:
  - `"entrance"` at `(0, 0, 0)` face `West`

### 3. `RoomRole::Decision`
- **Footprint cells**: `[(0, 0, 0), (1, 0, 0), (0, 1, 0)]` (3-hex triangle)
- **Cell connections**:
  - `(0, 0, 0)`: East (toward `(1,0,0)`) and SouthEast (toward `(0,1,0)`) are `Sealed` internally. All other lateral faces are `Door`.
  - `(1, 0, 0)`: West (toward `(0,0,0)`) and SouthWest (toward `(0,1,0)`) are `Sealed` internally. All other lateral faces are `Door`.
  - `(0, 1, 0)`: NorthWest (toward `(0,0,0)`) and NorthEast (toward `(1,0,0)`) are `Sealed` internally. All other lateral faces are `Door`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`
  - `"port_b"` at `(1, 0, 0)` face `East`
  - `"port_c"` at `(0, 1, 0)` face `SouthEast`

### 4. `RoomRole::DecoherenceFork`
- **Footprint cells**: `[(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 0)]` (4-hex diamond)
- **Cell connections**: Internal adjacent faces are `Sealed`. All other lateral faces are `Door`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`
  - `"port_b"` at `(1, 0, 0)` face `East`
  - `"port_c"` at `(0, 1, 0)` face `West`
  - `"port_d"` at `(1, 1, 0)` face `East`

### 5. `RoomRole::AnchorCheckpoint`
- **Footprint cells**: `[(0, 0, 0), (0, 1, 0)]` (2-hex strip)
- **Cell connections**: Internal face between them is `Sealed`. All other lateral faces are `Door`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`
  - `"port_b"` at `(0, 1, 0)` face `East`

### 6. `RoomRole::TeleportRelay`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: All 6 lateral faces are `Door`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`

### 7. `RoomRole::Keystone`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: All 6 lateral faces are `Door`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`

### 8. `RoomRole::DualStation`
- **Footprint cells**: `[(0, 0, 0), (1, 0, 0)]` (2-hex strip)
- **Cell connections**: Internal face between them is `Sealed`. All other lateral faces are `Door`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`
  - `"port_b"` at `(1, 0, 0)` face `East`

### 9. `RoomRole::GuardianControl`
- **Footprint cells**: `[(0, 0, 0), (0, 0, 1)]` (2-level vertical atrium)
- **Cell connections**:
  - `(0, 0, 0)`'s `Up` face: `PortClass::ShaftOpen` (internal transit connection)
  - `(0, 0, 1)`'s `Down` face: `PortClass::ShaftOpen` (internal transit connection)
  - Lateral faces of both cells are `Door`.
- **Named ports**:
  - `"lower_port"` at `(0, 0, 0)` face `West`
  - `"upper_port"` at `(0, 0, 1)` face `East`

### 10. `RoomRole::Monitor`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: All 6 lateral faces are `Door`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`

### 11. `RoomRole::Recovery`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: All 6 lateral faces are `Door`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`
