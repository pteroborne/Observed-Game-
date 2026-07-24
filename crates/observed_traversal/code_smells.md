# Code Smells Analysis: `observed_traversal`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_traversal` implements the deterministic movement engine. The primary smell stems from maintaining legacy AABB collision code alongside the promoted Rapier kinematic controller.

---

## Identified Code Smells

### 1. Legacy Seam / Dual Physics Backends (`PhysicsBackend::LegacyAabb`)
- **Category**: Dispensables (Legacy Code / Alternative Interfaces)
- **Severity**: Medium
- **Location**: [`src/lib.rs:PhysicsBackend`](file:///o:/Observed%202/crates/observed_traversal/src/lib.rs#L34-L38)
- **Description**: `PhysicsBackend::LegacyAabb` remains in `PhysicsBackend` and `lib.rs` for replay tape backwards compatibility, despite Rapier being promoted as the exclusive runtime controller.
- **Impact**: Code duplication between legacy AABB axis collision and raw Rapier kinematic capsule stepping.
- **Remediation**:
  - Apply **Inline / Remove Legacy Seam**: Once legacy replay tapes are migrated or archived, remove `LegacyAabb` and `lib.rs` collision helpers to leave `rapier_controller.rs` as the sole controller.

### 2. Large File (`lib.rs` - 811 lines)
- **Category**: Bloaters
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/crates/observed_traversal/src/lib.rs)
- **Description**: Combines `FpsBody`, `FpsConfig`, `FpsArena`, AABB step integration, and horizontal contact classification.
- **Impact**: Harder to isolate physics configuration structs from integration step logic.
- **Remediation**:
  - Apply **Extract Module**: Move AABB math to `aabb.rs` and step integration to `step.rs`.

---

## Clean Aspects & Good Practices
- **Deterministic Replay Guarantee**: Fixed timestep (`FIXED_DT = 1/60s`) and substep integration ensure bit-identical movement replays.
- **Input Decoupling**: Uses abstract `PlayerIntent` exclusively, allowing bots, controllers, and network replay tapes to drive movement transparently.
