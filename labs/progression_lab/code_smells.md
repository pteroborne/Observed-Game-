# Code Smells Analysis: `progression_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`progression_lab` tests career XP, level thresholds, and cosmetic unlocking/persistence. Logic is promoted to `observed_progression`.

---

## Identified Code Smells

### 1. Prototype Seam (Promoted Progression Crate)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/progression_lab/src/lib.rs)
- **Description**: Re-exports types promoted to `observed_progression`.
- **Impact**: Lab acts as a visual projection for career XP and unlock persistence.
- **Remediation**:
  - Retain current clean re-export structure.

---

## Clean Aspects & Good Practices
- **Complete Orthogonality Guarantee**: Proves that equipping cosmetics or accumulating XP never alters deterministic match outcomes or replay hashes.
