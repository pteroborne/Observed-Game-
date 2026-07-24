# Code Smells Analysis: `discovery_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`discovery_lab` tests typed rooms (Keystone Vault, Reactor, Sensor, Decoy, etc.), pre-commit doorframe reads, and harvest solvability constraints.

---

## Identified Code Smells

### 1. Large Match Statement on Room Types
- **Category**: Object-Orientation Abusers / Switch Statements
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/discovery_lab/src/main.rs)
- **Description**: Harvest outcomes and doorframe glyph mappings use a large `match room_type` block covering all 8 types.
- **Impact**: Adding new room types requires updating multiple match blocks.
- **Remediation**:
  - Apply **Replace Conditional with Trait / Method**: Encapsulate room type properties (`glyph()`, `harvest_yield()`, `door_role()`) directly on the `RoomType` enum.

---

## Clean Aspects & Good Practices
- **Solvability Conservation Rule**: Shifting only affects unharvested rooms, preserving the total multiset of keystones and preventing uncompletable runs.
