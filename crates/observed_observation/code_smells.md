# Code Smells Analysis: `observed_observation`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_observation` is the core observation graph engine. It is highly cohesive, pure, and deterministic.

---

## Identified Code Smells

### 1. Hardcoded 2D Grid Constants (`ROWS`, `COLS`, `ROOM_SPACING`)
- **Category**: Hardcoded Configuration / Magic Numbers
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/crates/observed_observation/src/lib.rs#L24-L30)
- **Description**: `ROWS = 3`, `COLS = 3`, `ROOM_HALF = 120.0`, `ROOM_SPACING = 320.0` are top-level module constants.
- **Impact**: Ties graph positioning helper (`room_center`) to 2D square lattices.
- **Remediation**:
  - Apply **Extract Parameter Object**: Move spatial lattice dimensions into a configurable `GridTopologySpec` parameter.

### 2. Side Alias (`pub use Direction as Side`)
- **Category**: Dispensables (Type Alias Overlap)
- **Severity**: Very Low
- **Location**: [`src/lib.rs:Side`](file:///o:/Observed%202/crates/observed_observation/src/lib.rs#L32)
- **Description**: `Side` is a re-exported alias for `observed_core::Direction`.
- **Impact**: Slight naming confusion across crate boundaries.
- **Remediation**:
  - Standardize on `Direction` directly across new modules.

---

## Clean Aspects & Good Practices
- **Completed PRNG Consolidation**: Uses `observed_core::SplitMix` exclusively for Fisher-Yates shuffles.
- **Pure Graph Involution**: `decohere()` preserves strict symmetric involution (`partner(partner(door)) == door`) across all random rewires.
