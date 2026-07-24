# Code Smells Analysis: `competitive_facility`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`competitive_facility` is an integration lab combining graph observation, protected route spines, multi-team races, and AI director collapse. Logic is promoted to `observed_match::facility`.

---

## Identified Code Smells

### 1. High Integration Coupling (Composition Smells)
- **Category**: Couplers
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/competitive_facility/src/lib.rs)
- **Description**: Integrates 4 distinct lab models (`observation`, `facility`, `competition`, `director`).
- **Impact**: Requires keeping multiple domain simulation data structures in sync during lab ticks.
- **Remediation**:
  - Keep models unified via `observed_match` production crate entry points.

---

## Clean Aspects & Good Practices
- **Deterministic Match Progress**: Progress is tied to discrete graph steps along the route spine, ensuring robust replayability.
