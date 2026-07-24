# Code Smells Analysis: `door_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`door_lab` is the 2D visual projection of the `observed_doors` crate, demonstrating doors as observation gates.

---

## Identified Code Smells

### 1. Hardcoded Keyboard Map Shortcuts (`1`, `2`, `3`, `4`)
- **Category**: Primitive Obsession / Hardcoded Controls
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/door_lab/src/main.rs)
- **Description**: Number keys `1`–`4` are hardcoded to toggle North, East, South, and West doors.
- **Impact**: Bypasses the unified `PlayerIntent` action mapping.
- **Remediation**:
  - Apply **Introduce Parameter Object / Intent Adapter**: Map door toggles to `PlayerIntent` directional interactions.

---

## Clean Aspects & Good Practices
- **Strict Pinning Invariant**: Open doors freeze connections; closed doors allow rewires behind unseen thresholds.
