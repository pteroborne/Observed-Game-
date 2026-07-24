# Code Smells Analysis: `full_wfc_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`full_wfc_lab` is the 3-level 2D/3D projection lab for the demoted square lattice WFC solver (`observed_facility::full_wfc`).

---

## Identified Code Smells

### 1. Prototype Seam (Demoted Solver Projection)
- **Category**: Dispensables / Legacy Adapters
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/full_wfc_lab/src/main.rs)
- **Description**: Projects `observed_facility::full_wfc` (square 8×5×3 lattice), which was demoted in favor of `hex_wfc_lab` (canonical hex lattice).
- **Impact**: Serves as a regression fixture rather than active game presentation.
- **Remediation**:
  - Maintain as a regression lab.

---

## Clean Aspects & Good Practices
- **Deterministic Solvability Enforcement**: Constrained WFC pulses gate every atomic relayout with A* route checks to ensure exit reachability.
