# Code Smells Analysis: `control_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`control_lab` tests input sampling, rebinding, intent playback, and gamepad assignment over `player_input`.

---

## Identified Code Smells

### 1. Large Event Handling Function
- **Category**: Bloaters (Long Method)
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/control_lab/src/main.rs)
- **Description**: Hardware sampling and rebind event listeners process multiple keys and gamepads in a single system.
- **Impact**: Increased complexity when adding new input device mappings.
- **Remediation**:
  - Apply **Extract Method**: Separate keyboard sampling, gamepad sampling, and rebind capture into isolated systems.

---

## Clean Aspects & Good Practices
- **Strict Intent Boundary**: Probes and character systems never query hardware keys directly; all input routes through `PlayerIntent`.
