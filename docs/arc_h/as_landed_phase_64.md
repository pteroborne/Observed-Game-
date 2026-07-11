# Phase 64 As-Landed Notes: Threshold Geometry Integrity

## Phase Objective
Playtesting found that hallway thresholds overlapped with walls or smashed into random corners. The goal was to find where the projection and the interior walls disagreed, fix it at the generator/projection source, and add geometry integrity checks to the map audit.

## Diagnosis and Fix
The bug was located in [wfc.rs](file:///O:/Observed%202.phase-64/crates/observed_facility/src/wfc.rs) in the vertex coordinate calculation for hallway interior wall segment generation. 
Specifically, the projection of interior grid corners (`px` and `py`) was shifted by half a cell:
```rust
let px = -footprint_x + (vx as f32 - 0.5) * cell;
let py = -footprint_y + (vy as f32 - 0.5) * cell;
```
This offset shifted the generated walls/pillars, causing door thresholds (which align with grid boundaries) to overlap or smash into the shifted walls.

**The Fix:**
The projection formula was corrected to align vertices directly on grid vertex boundaries:
```rust
let px = -footprint_x + vx as f32 * cell;
let py = -footprint_y + vy as f32 * cell;
```
This aligns the interior walls/pillars with the outer perimeter grid, ensuring they do not collide with door thresholds or approach paths.

## Validation Gates Added
Permanently integrated into [map_validation.rs](file:///O:/Observed%202.phase-64/game/src/map_validation.rs):
- `hallway_threshold_integrity_holds_for_template_seed_corpus` test: Verifies that hallway thresholds do not intersect walls, do not overlap each other, and have clear approach boxes on both sides.
- `capture_threshold_integrity_wfc_hallways_when_requested` test: If `OBSERVED2_CAPTURE_THRESHOLD_INTEGRITY` is set, dumps BMP files showing hallway interiors with corridor paths, walls, and thresholds.
- `corner_pillars_are_emitted_on_grid_vertices` test: Assures corner pillars sit exactly on grid vertex boundaries.

## Verification Results
1. **Pre-fix fail / Post-fix pass**:
   - Reverting the vertex projection fix caused **128 hallway places to fail** the new integrity audit (variation 8, version 1 and variation 12, version 1 across seeds 0–63) with hundreds of blocked approach warnings.
   - Restoring the fix results in **100% passing tests** (all 29 tests in `observed_facility` and all 226 tests in `observed_game`).
2. **Falsifiable Evidence**:
   - Hallway captures generated successfully as BMP files:
     - [phase64_threshold_integrity_v8_seed0.bmp](file:///C:/Users/comma/.gemini/antigravity/brain/0ab37ce1-f3da-4dfd-b6f3-e79449c7dd2b/phase64_threshold_integrity_v8_seed0.bmp)
     - [phase64_threshold_integrity_v12_seed0.bmp](file:///C:/Users/comma/.gemini/antigravity/brain/0ab37ce1-f3da-4dfd-b6f3-e79449c7dd2b/phase64_threshold_integrity_v12_seed0.bmp)
3. **Clippy and Format**:
   - `cargo clippy --workspace --all-targets` is clean.
   - `cargo fmt --all --check` is clean.
