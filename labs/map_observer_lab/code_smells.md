# Code Smells Analysis: `map_observer_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`map_observer_lab` provides free-fly and top-down inspection controls over `observed_game` state.

---

## Identified Code Smells

### 1. Large System Function (`observer_fly_control`)
- **Category**: Bloaters (Long Method)
- **Severity**: Low
- **Location**: [`src/lib.rs:observer_fly_control`](file:///o:/Observed%202/labs/map_observer_lab/src/lib.rs#L155-L413)
- **Description**: Handles initialization, teleport shortcuts (`1`–`6`), mode switches, mouse flight, and screenshot saving in a single system.
- **Impact**: Increased function size (~260 lines).
- **Remediation**:
  - Apply **Extract Method**: Break down into `handle_teleports`, `handle_mode_switch`, and `update_fly_pose`.

---

## Clean Aspects & Good Practices
- **Diagnostic Edge Glow**: Gizmo wireframes highlight wall and floor boundaries dynamically without altering underlying rendering components.
