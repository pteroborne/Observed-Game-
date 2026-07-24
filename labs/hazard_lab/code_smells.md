# Code Smells Analysis: `hazard_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`hazard_lab` evaluates two-player machinery gates and environmental pressure fronts.

---

## Identified Code Smells

### 1. Hardcoded Route Zones (`INTAKE`, `CORE`, `SPINE`)
- **Category**: Primitive Obsession / Hardcoded Configuration
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/hazard_lab/src/main.rs)
- **Description**: Route zones for pressure hazard steering are hardcoded enums.
- **Impact**: Adding new room zones requires modifying lab enum variants.
- **Remediation**:
  - Apply **Extract Parameter Object**: Move hazard zones to room socket tags.

---

## Clean Aspects & Good Practices
- **No Direct Player Damage**: Hazards stall progress or add delay penalties rather than inflicting health bar damage or killing player bodies.
