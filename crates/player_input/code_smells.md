# Code Smells Analysis: `player_input`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`player_input` is an exemplary input boundary crate. It isolates hardware input sampling from game simulation.

---

## Identified Code Smells

### 1. Hardcoded Action Booleans (`PlayerIntent`)
- **Category**: Long Parameter List / Data Clumps
- **Severity**: Very Low
- **Location**: [`src/lib.rs:PlayerIntent`](file:///o:/Observed%202/crates/player_input/src/lib.rs#L38-L47)
- **Description**: `PlayerIntent` contains individual boolean fields (`jump_pressed`, `sprint_held`, `interact_pressed`, `interact_held`, `climb_pressed`).
- **Impact**: Adding new action buttons requires adding struct fields and updating binary network packet layouts.
- **Remediation**:
  - Apply **Replace Data Value with Bitflags**: Encapsulate secondary action triggers in a bitflag `PlayerActions(u16)` enum.

---

## Clean Aspects & Good Practices
- **Pure Input Boundary**: Fully decoupled from Bevy rendering or hardware drivers when compiled with `--no-default-features`.
- **Vector Sanitization**: `sanitized()` prevents diagonal movement speed exploits by clamping vector magnitudes.
