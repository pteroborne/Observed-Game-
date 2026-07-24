# Code Smells Analysis: `ldtk_schematic_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`ldtk_schematic_lab` evaluates 2D level design and tactical map schematics using `bevy_ecs_ldtk`.

---

## Identified Code Smells

### 1. Embedded LDtk JSON Project String Literals
- **Category**: Primitive Obsession / Hardcoded Assets
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/ldtk_schematic_lab/src/main.rs)
- **Description**: LDtk level JSON strings are hardcoded into source files for headless testing.
- **Impact**: Level updates require editing embedded JSON strings.
- **Remediation**:
  - Apply **Extract Resource / File**: Move project JSON to `assets/ldtk/schematic.ldtk`.

---

## Clean Aspects & Good Practices
- **Strict Domain Boundary**: LDtk raw entities are converted to `RoomId` and `PortId` domain types and never leak into Bevy gameplay ECS loops.
