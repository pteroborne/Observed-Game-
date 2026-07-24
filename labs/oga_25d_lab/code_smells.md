# Code Smells Analysis: `oga_25d_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`oga_25d_lab` evaluates 2.5D billboard sprite rendering for actor placeholders inside 3D scenes.

---

## Identified Code Smells

### 1. Manual Camera Billboarding System
- **Category**: Dispensables / Feature Envy
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/oga_25d_lab/src/main.rs)
- **Description**: Manually updates sprite entity yaw rotation to face the active camera each frame.
- **Impact**: Adds CPU iteration time per billboard entity.
- **Remediation**:
  - Apply **Extract Component / Helper**: Use a dedicated `Billboard` component with query filtering.

---

## Clean Aspects & Good Practices
- **Procedural Mesh Fallback**: Fallback primitives render if PNG assets are missing, guaranteeing zero app crash risks.
