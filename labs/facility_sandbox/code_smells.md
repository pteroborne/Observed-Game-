# Code Smells Analysis: `facility_sandbox`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`facility_sandbox` is an early integration prototype combining ladder climbing, equipment carrying, and 5-room facility layouts.

---

## Identified Code Smells

### 1. Monolithic Facility Script (`src/facility.rs`)
- **Category**: Bloaters (Large File)
- **Severity**: Low
- **Location**: [`src/facility.rs`](file:///o:/Observed%202/labs/facility_sandbox/src/facility.rs)
- **Description**: `facility.rs` handles room setup, power cell socketing, pit bridging, and win condition checks in one file.
- **Impact**: Increased complexity when adding new puzzle objectives.
- **Remediation**:
  - Apply **Extract Class / Module**: Move pit bridging to `mechanics/bridge.rs` and powered doors to `mechanics/door.rs`.

---

## Clean Aspects & Good Practices
- **Despawn On Exit**: Uses `DespawnOnExit` components to cleanly tear down entities on state transitions back to the main menu.
