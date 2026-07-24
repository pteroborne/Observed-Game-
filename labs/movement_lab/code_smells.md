# Code Smells Analysis: `movement_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`movement_lab` evaluated 2D kinematic movement (coyote time, jump buffering, stairs, moving platforms) prior to 3D promotion.

---

## Identified Code Smells

### 1. Prototype Seam (2D Kinematic Physics Engine)
- **Category**: Dispensables / Legacy Code
- **Severity**: Low
- **Location**: [`src/simulation.rs`](file:///o:/Observed%202/labs/movement_lab/src/simulation.rs)
- **Description**: Contains a custom 2D AABB kinematic controller. Production movement uses 3D Rapier controllers in `observed_traversal`.
- **Impact**: Code duplication between 2D prototype and production 3D physics.
- **Remediation**:
  - Keep 2D prototype as an isolated lab test fixture.

---

## Clean Aspects & Good Practices
- **Deterministic Mechanics**: Coyote time, jump buffering, and moving platform displacement are pure functions of `(body, intent, dt)`.
