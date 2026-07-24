# Code Smells Analysis: `fps_visibility_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`fps_visibility_lab` evaluates 5x5 sub-room cell visibility fields for partial-room freezing.

---

## Identified Code Smells

### 1. Hardcoded Sub-Room Cell Grid (5x5)
- **Category**: Primitive Obsession / Hardcoded Configuration
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/fps_visibility_lab/src/main.rs)
- **Description**: Sub-room sampling resolution is fixed at 5x5 cells per room.
- **Impact**: Changing visibility sampling density requires editing array dimensions.
- **Remediation**:
  - Apply **Extract Parameter Object**: Move cell resolution into a `VisibilityConfig` resource.

---

## Clean Aspects & Good Practices
- **Partial Room Visibility**: Freezes doorway endpoints based on fine-grained raycasts through aperture slots, enabling realistic partial-room observation.
