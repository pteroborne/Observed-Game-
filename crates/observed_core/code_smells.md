# Code Smells Analysis: `observed_core`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_core` is exemplary: small, highly cohesive, zero external runtime dependencies, and focused purely on core domain primitives. Explicit primitive conversion helpers have been added across all domain newtype IDs.

---

## Resolved Code Smells

### 1. Inconsistent Numeric Widths (Primitive Obsession) — RESOLVED
- **Status:** Resolved.
- **Details:** Added explicit `.as_u32()`, `.as_u16()`, `.as_u8()`, and `.as_usize()` conversion methods across domain newtypes (`RoomId`, `CorridorId`, `PortId`, `EquipmentId`, `TeamId`, `ThresholdSlotId`), allowing clean type conversions across bit-width boundaries.

---

## Clean Aspects & Good Practices
- **Newtype Pattern**: Uses tuple structs for domain IDs (`RoomId`, `EquipmentId`), completely eliminating `Primitive Obsession` in entity references.
- **Zero Bevy Dependency**: Pure domain model that compiles independently of game engines.
