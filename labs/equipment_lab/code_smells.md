# Code Smells Analysis: `equipment_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`equipment_lab` tests persistent equipment states (carried, deployed, socketed, ground). Logic is promoted to `observed_interaction::equipment`.

---

## Identified Code Smells

### 1. Prototype Seam (Promoted Re-export)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/equipment_lab/src/lib.rs)
- **Description**: Re-exports types promoted to `observed_interaction::equipment`.
- **Impact**: Lab acts as a visual projection for the equipment model.
- **Remediation**:
  - Keep current re-export structure intact.

---

## Clean Aspects & Good Practices
- **Persistent Domain IDs**: Uses `EquipmentId` exclusively, guaranteeing item state survives entity despawning or room replacement.
