# Code Smells Analysis: `hex_wfc_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`hex_wfc_lab` provides the interactive 2D plan and 3D visual projection for `observed_facility::hex_wfc`.

---

## Identified Code Smells

### 1. Hardcoded Preset Seed Array
- **Category**: Primitive Obsession / Hardcoded Configuration
- **Severity**: Low
- **Location**: [`src/lib.rs:PRESET_SEEDS`](file:///o:/Observed%202/labs/hex_wfc_lab/src/lib.rs#L21-L32)
- **Description**: 9 fixed hexadecimal seed constants (`PRESET_SEEDS`) drive preset showcase states.
- **Impact**: Preset seeds are hardcoded in source code arrays.
- **Remediation**:
  - Keep hardcoded seeds as lab showcase fixtures.

---

## Clean Aspects & Good Practices
- **Strict Module Size Ratchet**: Includes a built-in unit test (`lab_rust_sources_stay_below_the_architecture_size_limit`) asserting that every `.rs` file in `src/` stays below 600 lines!
