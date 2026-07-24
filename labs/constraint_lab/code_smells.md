# Code Smells Analysis: `constraint_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`constraint_lab` validates route-spine graph constraints over Decoherence cycles. Logic is promoted to `observed_facility::constraints`.

---

## Identified Code Smells

### 1. Prototype Seam (Promoted Module Re-export)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/constraint_lab/src/lib.rs)
- **Description**: Re-exports types promoted to `observed_facility::constraints`.
- **Impact**: Lab acts as a visual debug projection for the production crate.
- **Remediation**:
  - Retain current clean re-exports for lab isolation.

---

## Clean Aspects & Good Practices
- **Deterministic Spanning Tree Constraint**: Guarantees level traversability (all 9 rooms reachable) across random graph rewires.
