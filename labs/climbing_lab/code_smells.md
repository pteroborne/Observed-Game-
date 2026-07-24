# Code Smells Analysis: `climbing_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`climbing_lab` evaluates discrete vertical traversal modes (ladder, ledge hang, grapple sockets). It maintains clean separation between pure state transitions (`simulation.rs`) and Bevy presentation (`app.rs`).

---

## Identified Code Smells

### 1. Large Enum State Machine (`ClimbMode`)
- **Category**: Object-Orientation Abusers / Switch Statements
- **Severity**: Low
- **Location**: [`src/simulation.rs`](file:///o:/Observed%202/labs/climbing_lab/src/simulation.rs)
- **Description**: `ClimbMode` handles `Free`, `Ladder`, `LedgeHang`, and `Grapple` transitions via a single match statement in the tick handler.
- **Impact**: Adding new traversal types (e.g. wall slides or ropes) increases match statement size.
- **Remediation**:
  - Apply **Replace Conditional with State Pattern**: Move mode-specific update logic into dedicated `ModeHandler` traits if more climb modes are added.

---

## Clean Aspects & Good Practices
- **Readable Constraints Rule**: Uses explicit authored sockets and ladders rather than complex continuous mesh collision detection.
- **Pure Simulation Layer**: `simulation.rs` is pure Rust and fully unit-testable without launching Bevy.
