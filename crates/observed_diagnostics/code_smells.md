# Code Smells Analysis: `observed_diagnostics`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_diagnostics` is a clean, engine-independent validation framework. Code smells are minor and mostly relate to large snapshot structs and magic luminance coefficients.

---

## Identified Code Smells

### 1. Magic Numbers (Rec. 709 Luminance Coefficients)
- **Category**: Dispensables (Magic Numbers)
- **Severity**: Low
- **Location**: [`src/lib.rs:luminance`](file:///o:/Observed%202/crates/observed_diagnostics/src/lib.rs#L211-L213)
- **Description**: `luminance(rgb)` computes `0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]`.
- **Impact**: Unnamed magic floating-point constants.
- **Remediation**:
  - Apply **Extract Constant**: Replace raw numbers with named constants (`LUMINANCE_WEIGHT_R`, `LUMINANCE_WEIGHT_G`, `LUMINANCE_WEIGHT_B`).

### 2. Large Struct / Data Clumps (`ThresholdSnapshot`)
- **Category**: Bloaters / Data Clumps
- **Severity**: Low
- **Location**: [`src/lib.rs:ThresholdSnapshot`](file:///o:/Observed%202/crates/observed_diagnostics/src/lib.rs#L112-L128)
- **Description**: `ThresholdSnapshot` holds 13 flat fields (`frame_count`, `leaf_count`, `frame_light_count`, `matching_status_light_count`, etc.).
- **Impact**: Struct initialization is verbose.
- **Remediation**:
  - Apply **Extract Struct**: Group element counts into a `ThresholdCounts` child struct.

---

## Clean Aspects & Good Practices
- **Engine Decoupled**: Free of Bevy or rendering pipeline dependencies, enabling fast headless execution.
- **Clear Rule Functions**: Rule check functions (`check_geometry`, `check_thresholds`, `check_lights`, `check_materials`) are pure and easy to test.
