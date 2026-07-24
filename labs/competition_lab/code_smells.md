# Code Smells Analysis: `competition_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`competition_lab` evaluates multi-team racing and capacity-limited exit gates. Logic is now promoted into `observed_match::competition`.

---

## Identified Code Smells

### 1. Prototype Seam (Promoted Code Re-export)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/competition_lab/src/lib.rs)
- **Description**: `competition_lab` now re-exports types promoted to `observed_match::competition`.
- **Impact**: Lab acts as a wrapper projection rather than owning original models.
- **Remediation**:
  - Keep re-export structure as required by the lab sandbox architecture.

---

## Clean Aspects & Good Practices
- **No Direct Damage Rule**: Enforces non-violent competition (progress is never lowered; interference is purely indirect via shared control holds).
