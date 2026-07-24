# Code Smells Analysis: `observed_style`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_style` acts as the single source of truth for the project's neon-noir visual language and Legibility Contract. While conceptually pure, its implementation is housed in an oversized monolithic file.

---

## Identified Code Smells

### 1. Monolithic Single File (`src/lib.rs` - 1,770 lines)
- **Category**: Bloaters (Large File / Monolithic Module)
- **Severity**: High
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/crates/observed_style/src/lib.rs)
- **Description**: `lib.rs` is ~1,770 lines long and contains all color vision matrices, surface role mappings, marker treatments, door identity reads, district atmosphere palettes, and legibility assertion tests.
- **Impact**: Makes editing specific aesthetic tokens difficult and bloats diffs.
- **Remediation**:
  - Apply **Extract Module**: Reorganize into a folder module:
    - `src/surfaces.rs`: `SurfaceRole` and `Treatment` math.
    - `src/markers.rs`: `MarkerRole`, `OutlineRole`, and team palettes.
    - `src/doors.rs`: `DoorIdentityRole` and `ThresholdFrameState`.
    - `src/districts.rs`: District atmosphere and lighting scale definitions.
    - `src/accessibility.rs`: `ColorVisionMode` matrix simulation and contrast assertion tests.

---

## Clean Aspects & Good Practices
- **Strict Legibility Contract**: Formally asserts minimum emissive luminance (`SIGNAL_MIN_LUMINANCE = 2.0`) for all gameplay-critical cues.
- **No Ad-Hoc Presentation Colors**: Completely prevents visual inconsistencies by routing all material colors through centralized semantic functions.
