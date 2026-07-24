# Code Smells Analysis: `observed_authoring`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_authoring` is a well-structured parser and asset compilation pipeline. It isolates TrenchBroom file reading and geometry conversion into pure, deterministic data transformations.

---

## Identified Code Smells

### 1. Legacy Compatibility Seam (`manifest.rs`)
- **Category**: Object-Orientation Abusers / Change Preventers
- **Severity**: Low
- **Location**: [`src/manifest.rs`](file:///o:/Observed%202/crates/observed_authoring/src/manifest.rs)
- **Description**: `manifest.rs` supports legacy v1 map manifest entries alongside the version-2 `source.rs` metadata schema.
- **Impact**: Requires maintaining two separate deserialization paths for tile definitions.
- **Remediation**:
  - Apply **Inline / Deprecate Legacy Seam**: Once all v1 map assets are migrated to version-2 `.map` metadata, remove `manifest.rs` to simplify catalog compilation.

### 2. Retained Generator Code (`tile_source.rs`)
- **Category**: Dispensables (Speculative Generality / Dead Code risk)
- **Severity**: Low
- **Location**: [`src/tile_source.rs`](file:///o:/Observed%202/crates/observed_authoring/src/tile_source.rs)
- **Description**: `tile_source.rs` generates programmatic tile fixtures for Arc-L tests.
- **Impact**: Developers might accidentally call `tile_source` in runtime spawning loops, overwriting hand-authored level designs.
- **Remediation**:
  - Apply **Move to Test Module (`#[cfg(test)]`)**: Restrict `tile_source` module visibility strictly to test targets so it cannot be imported by production crates or game modules.

### 3. Positional Conversion Primitives (`f64` Scaling)
- **Category**: Primitive Obsession
- **Severity**: Very Low
- **Location**: [`src/lib.rs:UNITS_PER_METER`](file:///o:/Observed%202/crates/observed_authoring/src/lib.rs#L25)
- **Description**: Conversions between TrenchBroom integer map units and world meters rely on raw `f64` division (`world_pos / 16.0`).
- **Impact**: Risk of rounding discrepancies when converting floating-point values back to integer editor coordinates.
- **Remediation**:
  - Apply **Replace Primitive with Value Object**: Encapsulate coordinates in `MapUnits(i32)` and `WorldMeters(f32)` newtypes with explicit conversion methods.

---

## Clean Aspects & Good Practices
- **Deterministic Hull Extraction**: Uses exact convex plane intersection logic, avoiding non-deterministic mesh simplification libraries.
- **Strict Schema Enforcement**: Validates tile footprint boundaries and quantized port alignments at compile/intake time before loading into Bevy.
