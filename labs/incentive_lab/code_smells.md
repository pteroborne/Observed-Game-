# Code Smells Analysis: `incentive_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`incentive_lab` tests scoring mechanics based on team dispersion (spreading across distinct rooms) and room charge regrowth.

---

## Identified Code Smells

### 1. Prototype Seam (Promoted Module Re-export)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/incentive_lab/src/lib.rs)
- **Description**: Re-exports types promoted to `observed_progression::incentive`.
- **Impact**: Lab acts as a visual projection for scoring rules.
- **Remediation**:
  - Retain current clean re-export structure.

---

## Clean Aspects & Good Practices
- **Anti-Clumping Design**: Dispersion multipliers incentivize splitting squad members across unobserved sectors, complementing observation mechanics.
