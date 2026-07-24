# Code Smells Analysis: `rapier_authoring_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`rapier_authoring_lab` tests TrenchBroom `.map` brush point cloud imports converted directly into Rapier 3D convex hull colliders (`observed_authoring`).

---

## Identified Code Smells

### 1. Prototype Seam (Authoring Crate Re-export)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/rapier_authoring_lab/src/lib.rs)
- **Description**: Re-exports types promoted to `observed_authoring`.
- **Impact**: Lab acts as a visual projection for map brush parsing.
- **Remediation**:
  - Retain current clean re-export structure.

---

## Clean Aspects & Good Practices
- **Pure Data Importer**: TrenchBroom `.map` brush parsing runs as pure Rust data transformation without needing active Bevy rendering or scene entities.
