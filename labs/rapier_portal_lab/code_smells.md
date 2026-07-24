# Code Smells Analysis: `rapier_portal_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`rapier_portal_lab` tests threshold portal windows, render-to-texture previews, and anchor locks over Gantry and Colonnade modules.

---

## Identified Code Smells

### 1. Hardcoded Offscreen Render Layer Assignment
- **Category**: Primitive Obsession / Hardcoded Configuration
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/rapier_portal_lab/src/main.rs)
- **Description**: Render-to-texture portal previews use hardcoded Bevy `RenderLayers` index (Layer 1).
- **Impact**: Offscreen camera setups require matching manual layer constants.
- **Remediation**:
  - Apply **Extract Constant**: Move layer definitions to a `PortalRenderLayers` constant.

---

## Clean Aspects & Good Practices
- **Strict Spatial Window Representation**: Portal camera follows player eyes relative to threshold bounds, creating seamless non-Euclidean portal views without static thumbnails.
