# Code Smells Analysis: `observed_facility`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_facility` houses the core procedural generation and facility layout algorithms. While the canonical `hex_wfc` system is well-tested, legacy adapters create structural duplication.

---

## Identified Code Smells

### 1. Parallel / Deprecated WFC Solvers (`full_wfc.rs` vs `hex_wfc.rs`)
- **Category**: Change Preventers / Duplicate Code
- **Severity**: Medium
- **Location**: [`src/full_wfc.rs`](file:///o:/Observed%202/crates/observed_facility/src/full_wfc.rs) & [`src/hex_wfc.rs`](file:///o:/Observed%202/crates/observed_facility/src/hex_wfc.rs)
- **Description**: `full_wfc.rs` contains a 8×5×3 square lattice solver retained solely as a demoted Arc-K regression adapter, while `hex_wfc.rs` implements the canonical 28×20×10 hexagonal facility.
- **Impact**: Maintenance overhead when updating shared WFC concepts (like A* path search or candle telemetry).
- **Remediation**:
  - Apply **Collapse Hierarchy / Extract Common Trait**: Move shared A* pathing and observation-pinning logic into a common trait, allowing `full_wfc` to share core code with `hex_wfc`.

### 2. Feature-Flag Inconsistency
- **Category**: Dispensables / Configuration Smells
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/crates/observed_facility/src/lib.rs#L21-L32)
- **Description**: `hex_wfc.rs` is compiled unconditionally, whereas `full_wfc.rs` and `wfc.rs` require `#[cfg(feature = "wfc")]`.
- **Impact**: Confusing build target configurations for developers trying to run WFC tests.
- **Remediation**:
  - Apply **Consolidate Feature Flags**: Standardize WFC solver features under a unified feature gate or make all pure solvers unconditional.

### 3. Oversized Module (`room_world.rs`)
- **Category**: Bloaters (Large File)
- **Severity**: Low
- **Location**: [`src/room_world.rs`](file:///o:/Observed%202/crates/observed_facility/src/room_world.rs)
- **Description**: Mixes ASCII topology parsing, transform alignment, and collision AABB validation in one file.
- **Impact**: Harder to isolate math utilities from level file loading.
- **Remediation**:
  - Apply **Extract Module**: Move ASCII parsing to `room_world/parser.rs` and spatial math to `room_world/transform.rs`.

---

## Clean Aspects & Good Practices
- **Deterministic Randomness**: All WFC collapse choices and tie-breaks use `observed_core::SplitMix`, ensuring bit-identical reproducibility across platforms.
- **Strict Bounds Verification**: Bounded retry budgets (e.g. max 64 collapse attempts) prevent infinite loops during procedural generation.
