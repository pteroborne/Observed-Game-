# Code Smells Analysis: `fps_facility_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`fps_facility_lab` turns the 2D room graph into a 3D navigable facility using `observed_facility` and `observed_observation`.

---

## Identified Code Smells

### 1. Prototype Seam (Promoted Module Re-export)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/fps_facility_lab/src/lib.rs)
- **Description**: Re-exports types promoted to `observed_facility`.
- **Impact**: Lab acts as a visual projection for 3D room module generation.
- **Remediation**:
  - Maintain current clean re-export structure.

---

## Clean Aspects & Good Practices
- **Non-Physical Threshold Teleportation**: Graph connections override physical world positions transparently when a player steps through a passage threshold.
