# Code Smells Analysis: `mutable_facility`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`mutable_facility` was the first integration lab combining graph observation, protected route spines, and objective power cells. Logic is promoted to `observed_facility::mutable`.

---

## Identified Code Smells

### 1. Prototype Seam (Promoted Module Re-export)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/mutable_facility/src/lib.rs)
- **Description**: Re-exports types promoted to `observed_facility::mutable`.
- **Impact**: Lab acts as a visual projection for objective-driven graph rewires.
- **Remediation**:
  - Retain current clean re-export structure.

---

## Clean Aspects & Good Practices
- **Deterministic Objective Loop**: Guarantees that moving squad members along protected spine routes maintains continuous exit reachability.
