# Phase 90 & Phase 91 Alignment: Room Blueprints & Tile Signatures

This document coordinates the room blueprint definitions between the Phase 90 WFC solver implementation and the Phase 91 Tile Library authoring.

## Resolved port model (Phase 118 correction, 2026-07-28)

The catalog below is the frozen contract and matches the implementation in
`crates/observed_facility/src/hex_wfc/blueprint.rs`. Phase 118 superseded the
original perimeter contract after the human playtest showed that it rendered a
footprint as a cluster of cells rather than one room:

- **Interior sibling faces are open.** Adjacent cells in one blueprint share a
  traversable `Door` connection and author no wall geometry at that seam.
- **Exterior openings are exactly `named_ports`.** Every unnamed perimeter
  face is `Sealed`; named faces get the framed threshold geometry and stable
  `ThresholdKey { room, port }` identity. A named port is therefore the complete
  opening contract, not a labeled subset of a larger anonymous perimeter.
- **Identity:** `StampedBlueprint::generation_key()` = `hash(blueprint role,
  anchor cell)`; stable across relayouts, mirrors
  `full_wfc::catalog::corridor_generation_key` discipline.
- **Geometry demand feed:** `observed_facility::hex_wfc::geometry_demands()`
  returns every exact `(tile archetype, PortSignature)` pair the projector can
  emit. It includes authored blueprint-cell semantics, flat halls, RampUp, and
  all 64 Shaft variants; it deliberately excludes Void, the geometry-free
  RampHead, and generic Room variants that cannot leave a stamped blueprint
  domain. The Phase 91 validator requires every demand in every architecture
  register. `demandable_signatures()` remains the broader WFC propagation
  alphabet and is not a tile-coverage oracle.
- **Boundary blueprint rule:** room prefab selection uses the blueprint's
  authored `cell_signature(offset)`. Boundary-adjusted solver placements may
  seal out-of-grid faces (notably Start/Exit); the Phase 92 rhombus boundary
  shell closes those authored exterior apertures physically.

## Blueprint Footprints & Named Ports

The WFC solver expects the following relative coordinates, named ports, and port signatures for the room blueprints stamped into the grid:

### 1. `RoomRole::Start`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: West and East are `Door`; all other faces are `Sealed`.
- **Named ports**:
  - `"entrance"` at `(0, 0, 0)` face `West`
  - `"exit"` at `(0, 0, 0)` face `East`

### 2. `RoomRole::Exit`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: West is `Door`; all other faces are `Sealed`.
- **Named ports**:
  - `"entrance"` at `(0, 0, 0)` face `West`

### 3. `RoomRole::Decision`
- **Footprint cells**: `[(0, 0, 0), (1, 0, 0), (0, 1, 0)]` (3-hex triangle)
- **Cell connections**:
  - `(0, 0, 0)`: East and SouthEast are open to sibling cells; West is the named threshold.
  - `(1, 0, 0)`: West and SouthWest are open to sibling cells; East is the named threshold.
  - `(0, 1, 0)`: NorthWest and NorthEast are open to sibling cells; SouthEast is the named threshold.
  - Every other lateral face is `Sealed`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`
  - `"port_b"` at `(1, 0, 0)` face `East`
  - `"port_c"` at `(0, 1, 0)` face `SouthEast`

### 4. `RoomRole::DecoherenceFork`
- **Footprint cells**: `[(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 0)]` (4-hex diamond)
- **Cell connections**: Every internal adjacent face is open. Only the four named West/East perimeter faces are `Door`; every other perimeter face is `Sealed`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`
  - `"port_b"` at `(1, 0, 0)` face `East`
  - `"port_c"` at `(0, 1, 0)` face `West`
  - `"port_d"` at `(1, 1, 0)` face `East`

### 5. `RoomRole::AnchorCheckpoint`
- **Footprint cells**: `[(0, 0, 0), (0, 1, 0)]` (2-hex strip)
- **Cell connections**: The shared SouthEast/NorthWest faces are open. West on the first cell and East on the second are the only exterior doors.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`
  - `"port_b"` at `(0, 1, 0)` face `East`

### 6. `RoomRole::TeleportRelay`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: West is `Door`; all other faces are `Sealed`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`

### 7. `RoomRole::Keystone`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: West is `Door`; all other faces are `Sealed`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`

### 8. `RoomRole::DualStation`
- **Footprint cells**: `[(0, 0, 0), (1, 0, 0)]` (2-hex strip)
- **Cell connections**: The shared East/West faces are open. West on the first cell and East on the second are the only exterior doors.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`
  - `"port_b"` at `(1, 0, 0)` face `East`

### 9. `RoomRole::GuardianControl`
- **Footprint cells**: `[(0, 0, 0), (0, 0, 1)]` (2-level vertical atrium)
- **Cell connections**:
  - `(0, 0, 0)`'s `Up` face: `PortClass::ShaftOpen` (internal transit connection)
  - `(0, 0, 1)`'s `Down` face: `PortClass::ShaftOpen` (internal transit connection)
  - Lower West and upper East are the only lateral doors; every other lateral face is `Sealed`.
- **Named ports**:
  - `"lower_port"` at `(0, 0, 0)` face `West`
  - `"upper_port"` at `(0, 0, 1)` face `East`

### 10. `RoomRole::Monitor`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: West is `Door`; all other faces are `Sealed`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`

### 11. `RoomRole::Recovery`
- **Footprint cells**: `[(0, 0, 0)]`
- **Exterior ports**: West is `Door`; all other faces are `Sealed`.
- **Named ports**:
  - `"port_a"` at `(0, 0, 0)` face `West`
