# Code Smells Analysis: `wellshaft_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`wellshaft_lab` evaluates vertical silo hubs and stair tread connections (`shaft.rs`).

---

## Identified Code Smells

### 1. Hardcoded Level Height Steps (`shaft.rs`)
- **Category**: Primitive Obsession / Hardcoded Configuration
- **Severity**: Low
- **Location**: [`src/shaft.rs`](file:///o:/Observed%202/labs/wellshaft_lab/src/shaft.rs)
- **Description**: Vertical level heights (`LEVELS = 5`, `LEVEL_HEIGHT = 4.8`) are hardcoded constants.
- **Impact**: Altering wellshaft level count requires editing constants across `shaft.rs`.
- **Remediation**:
  - Apply **Extract Parameter Object**: Encapsulate silo level dimensions into a `ShaftSpec` struct.

---

## Clean Aspects & Good Practices
- **Stair Tread Autostep Alignment**: Stair tread risers are explicitly constrained below `STEP_HEIGHT` (0.45 m), ensuring smooth physical traversal without jumping.
