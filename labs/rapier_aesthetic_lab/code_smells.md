# Code Smells Analysis: `rapier_aesthetic_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`rapier_aesthetic_lab` proves neon-noir district lighting and style treatments applied to TrenchBroom/Rapier interior geometry.

---

## Identified Code Smells

### 1. Hardcoded Visual Style Overrides
- **Category**: Primitive Obsession / Hardcoded Configuration
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/rapier_aesthetic_lab/src/main.rs)
- **Description**: Stages visual palettes (`Archive/BLAME` vs `Reactor/Silo`) directly inside lab scene setups.
- **Impact**: Altering register lighting parameters requires updating lab-local material setups.
- **Remediation**:
  - Delegate all lighting palette assignments directly to `observed_style::architecture`.

---

## Clean Aspects & Good Practices
- **Legibility Contract Preservation**: Guarantees signal markers (anchors, portals, controls) maintain required luminance above dark neon-noir environments.
