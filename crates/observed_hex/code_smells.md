# Code Smells Analysis: `observed_hex`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_hex` is a textbook example of a focused, zero-dependency domain math crate. Code quality is exceptional.

---

## Identified Code Smells

### 1. Floating-Point Metric Epsilon (`ACROSS_CORNERS`)
- **Category**: Primitive Obsession / Precision Risks
- **Severity**: Very Low
- **Location**: [`src/metrics.rs`](file:///o:/Observed%202/crates/observed_hex/src/metrics.rs)
- **Description**: `ACROSS_CORNERS` uses an irrational floating point constant (`8.082903768654761`).
- **Impact**: Floating-point rounding error when checking corner point equality without epsilon tolerances.
- **Remediation**:
  - Apply **Introduce Epsilon Equality Helper**: Ensure geometry tests use `approx_eq` instead of exact `f32` equality checks when comparing corner coordinates.

---

## Clean Aspects & Good Practices
- **Single Source of Truth**: Eliminates duplicate hex metrics across `observed_authoring`, `observed_facility`, and `game`.
- **Pure Math Module**: Free of engine dependencies; fast compile times and 100% unit-testable.
