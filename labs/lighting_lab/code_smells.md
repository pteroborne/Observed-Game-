# Code Smells Analysis: `lighting_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`lighting_lab` is the primary visual architecture diorama lab, testing lighting legibility contracts against the 9 architecture registers.

---

## Identified Code Smells

### 1. Magic Numbers in Luminance Corridor Math
- **Category**: Primitive Obsession / Magic Numbers
- **Severity**: Low
- **Location**: [`src/luminance.rs`](file:///o:/Observed%202/labs/lighting_lab/src/luminance.rs)
- **Description**: Rec. 709 luminance weights (`0.2126`, `0.7152`, `0.0722`) and target percentile corridors are hardcoded in grading loops.
- **Impact**: Altering grading thresholds requires updating raw float constants in `luminance.rs`.
- **Remediation**:
  - Encapsulate constants in a `LuminanceCorridorSpec` struct.

---

## Clean Aspects & Good Practices
- **Strict Legibility Contract Enforcement**: Guarantees gameplay signals (anchors, exit gates, rivals) punch through neon-noir fog at verified brightness thresholds.
