# Code Smells Analysis: `game` (Assembled Game)

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
The `game` crate is the final integration package. Refactor Arc G restructured `game` into clean `sim/`, `view/`, and `screens/` trees backed by `arch_check.rs` ratchets. All 326 game tests and architectural ratchets are 100% passing.

---

## Resolved & Documented Aspects

### 1. Architectural Dependency Boundaries — VERIFIED & RATCHETED
- **Status:** Verified clean via `arch_check.rs`.
- **Details:** Ratchet tests enforce one-way dependency flow (`view/` reads `sim/`, `sim/` never imports `view/` or `screens/`), zero glob re-exports (`pub use x::*`), and explicit imports across all modules.

---

## Clean Aspects & Good Practices
- **Architectural Ratchet Protection (`arch_check.rs`)**: Automatically scans source files during `cargo test` to fail if glob re-exports (`pub use x::*`), `use super::*`, or simulation-to-presentation imports are introduced.
- **Strict One-Way Dependency Flow**: `view/` reads `sim/`, but `sim/` never imports `view/` or `screens/`.
- **Canonical Hex WFC Match Engine**: Production game runs on the continuous `hex_wfc` facility with strict cell/whole-room geometry projection.
