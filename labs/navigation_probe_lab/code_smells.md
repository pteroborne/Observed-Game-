# Code Smells Analysis: `navigation_probe_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`navigation_probe_lab` evaluates 2D polyanya navmesh routing (`vleue_navigator`) over a 4-room graph cycle.

---

## Identified Code Smells

### 1. Inappropriate Intimacy (Manual Navmesh Rebuilds)
- **Category**: Couplers / Inappropriate Intimacy
- **Severity**: Low
- **Location**: [`src/nav.rs`](file:///o:/Observed%202/labs/navigation_probe_lab/src/nav.rs)
- **Description**: Navmesh is manually torn down and rebuilt whenever a door toggles open or closed.
- **Impact**: Navmesh reconstruction overhead scales with graph size.
- **Remediation**:
  - Apply **Hide Delegate / Dynamic Obstacles**: Use dynamic obstacle layers if navmesh routing is promoted to production 3D facilities.

---

## Clean Aspects & Good Practices
- **Strict Agreement Rule**: Guarantees navmesh routes always agree bit-for-bit with authoritative graph reachability checks (`nav == graph`).
