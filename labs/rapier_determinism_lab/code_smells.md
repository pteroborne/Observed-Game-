# Code Smells Analysis: `rapier_determinism_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`rapier_determinism_lab` isolates pure `rapier3d` lockstep determinism checks (`physics.rs`).

---

## Identified Code Smells

### 1. Hardcoded Hash Bitmask Serializer
- **Category**: Primitive Obsession
- **Severity**: Low
- **Location**: [`src/physics.rs`](file:///o:/Observed%202/labs/rapier_determinism_lab/src/physics.rs)
- **Description**: State hashes are computed by hashing raw float bits into a custom FNV-1a accumulator.
- **Impact**: Altering rigid body snapshot fields requires updating manually packed byte arrays.
- **Remediation**:
  - Encapsulate rigid body state bit packing in a `StateHashBuilder` struct.

---

## Clean Aspects & Good Practices
- **Strict Bitwise Replay Protection**: Offline unit tests assert `world_a.state_hash() == world_b.state_hash()` across 600 ticks under `enhanced-determinism`.
