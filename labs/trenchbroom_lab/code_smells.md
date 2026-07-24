# Code Smells Analysis: `trenchbroom_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`trenchbroom_lab` evaluates Quake `.map` level parsing using `bevy_trenchbroom` for 3D interior geometry.

---

## Identified Code Smells

### 1. Prototype Seam (Map Importer Data Boundary)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/project.rs`](file:///o:/Observed%202/labs/trenchbroom_lab/src/project.rs)
- **Description**: Parses `QuakeMapEntities` into domain `RoomId`, `PortId`, and AABB colliders.
- **Impact**: Keeps TrenchBroom editor entity types out of the gameplay ECS loop.
- **Remediation**:
  - Maintain data projection boundary as implemented.

---

## Clean Aspects & Good Practices
- **Data-Only Importer**: Re-projects `.map` brush geometry through `observed_style` treatments, preventing map-authored materials from breaking the Legibility Contract.
