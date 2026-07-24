# Code Smells Analysis: `fps_elevation_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`fps_elevation_lab` tested step-up stair autostep logic, which was subsequently promoted into `observed_traversal`.

---

## Identified Code Smells

### 1. Hardcoded Grounded Epsilon Flicker
- **Category**: Primitive Obsession / Precision Risks
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/fps_elevation_lab/src/main.rs)
- **Description**: Documented limitation where `grounded` flag flickered on/off by one tick at rest due to micro-fall re-grounding.
- **Impact**: Caused test assertions to check position heights instead of `grounded` booleans.
- **Remediation**:
  - Apply **Introduce Epsilon Tolerance**: Promoted `UNDERFOOT_EPS` in `observed_traversal` resolved this flicker.

---

## Clean Aspects & Good Practices
- **Step Classification**: Categorizes horizontal contacts cleanly into Underfoot, StepUp, and Wall.
