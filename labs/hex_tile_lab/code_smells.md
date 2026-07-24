# Code Smells Analysis: `hex_tile_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`hex_tile_lab` is a 1,678-line showcase lab combining single tile previews, multi-tile compositions, TUI menu overlays, and camera view modes.

---

## Identified Code Smells

### 1. Large Monolithic Module (`src/lib.rs` - 1,678 lines)
- **Category**: Bloaters (Large File / God Object)
- **Severity**: Medium
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/hex_tile_lab/src/lib.rs)
- **Description**: `lib.rs` manages composition building, camera flight modes, mesh generation, Rapier scene construction, keyboard sampling, and status UI overlays in a single file.
- **Impact**: Increased difficulty when navigating or extending camera or mesh builders.
- **Remediation**:
  - Apply **Extract Module**: Split into `compositions.rs`, `camera.rs`, `visuals.rs`, and `input.rs`.

---

## Clean Aspects & Good Practices
- **Multi-Camera Flexibility**: Smoothly transitions between First-Person kinematic walking, Turntable Orbit, and Free Look fly cameras without breaking collision arenas.
