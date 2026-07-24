# Code Smells Analysis: `capture_pipeline_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`capture_pipeline_lab` evaluates `bevy_image_export` for offscreen screenshot and GIF generation. It isolates GPU frame buffer readbacks from simulation logic.

---

## Identified Code Smells

### 1. Hardcoded Output Directories (Magic Strings)
- **Category**: Primitive Obsession / Hardcoded Configuration
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/capture_pipeline_lab/src/main.rs)
- **Description**: Target export paths (`docs/evidence/capture_pipeline_lab/still/`) are embedded as string literals in system setups.
- **Impact**: Changing output file locations requires editing code across setup systems.
- **Remediation**:
  - Apply **Extract Constant / Resource**: Group export directories inside a dedicated `CaptureConfig` resource.

---

## Clean Aspects & Good Practices
- **Render-Target Isolation**: Exports frames directly from an offscreen render target without hijacking the primary window camera.
- **Zero Simulation Interference**: Capturing evidence frames never pauses or alters fixed-step simulation timelines.
