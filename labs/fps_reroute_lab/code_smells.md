# Code Smells Analysis: `fps_reroute_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`fps_reroute_lab` tests off-camera atomic corridor swaps over decohering graphs.

---

## Identified Code Smells

### 1. Dual Layout State Management (`rendered` vs `target`)
- **Category**: Change Preventers / Complex State Synchronization
- **Severity**: Low
- **Location**: [`src/reroute.rs`](file:///o:/Observed%202/labs/fps_reroute_lab/src/reroute.rs)
- **Description**: Maintains two separate full maze layouts (`rendered` and `target`) until atomic swap conditions pass.
- **Impact**: Increased memory footprint and double iteration over tile maps.
- **Remediation**:
  - Keep current atomic swap gate model as it is required to prevent player clipping bugs.

---

## Clean Aspects & Good Practices
- **Atomic Deferral Safety**: Reroutes defer gracefully when target tiles are within camera view or under player feet, avoiding visual popping.
