# Code Smells Analysis: `observed_match`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_match` is the core competitive match brain. It has undergone previous successful refactoring (e.g. `hybrid.rs` split into `hybrid/`), but maintains some large modules.

---

## Identified Code Smells

### 1. Large Class / Long File (`maze.rs`)
- **Category**: Bloaters
- **Severity**: Medium
- **Location**: [`src/maze.rs`](file:///o:/Observed%202/crates/observed_match/src/maze.rs)
- **Description**: `maze.rs` handles randomized DFS generation, corridor wall segmentation, and spatial grid embedding in a single large file.
- **Impact**: Harder to isolate spanning-tree graph logic from geometry layout generation.
- **Remediation**:
  - Apply **Extract Class / Module**: Split `maze.rs` into `maze/dfs.rs`, `maze/grid.rs`, and `maze/geometry.rs`.

### 2. High Responsibility Count (`hex_wfc.rs`)
- **Category**: Bloaters / Divergent Change
- **Severity**: Low
- **Location**: [`src/hex_wfc.rs`](file:///o:/Observed%202/crates/observed_match/src/hex_wfc.rs)
- **Description**: `hex_wfc.rs` manages cell projection, Rapier delta updates, caged lantern anchors, Guardian AI pathing, and bot-POV evidence captures.
- **Impact**: Changes to Guardian AI or lantern anchors require editing the same large match adapter file.
- **Remediation**:
  - Apply **Extract Submodules**: Split into `hex_wfc/lantern.rs`, `hex_wfc/guardian.rs`, and `hex_wfc/tacmap.rs`.

---

## Clean Aspects & Good Practices
- **Successful Modularization (`hybrid/`)**: Refactored the previous 1,156-line `hybrid.rs` into a clean folder module (`match_state.rs`, `round_step.rs`, `replay.rs`, `test.rs`), enforcing single responsibility principles.
- **Deterministic Replay**: Match state stepping is driven by PRNG seeds and versioned commands, guaranteeing 100% lockstep replay fidelity.
