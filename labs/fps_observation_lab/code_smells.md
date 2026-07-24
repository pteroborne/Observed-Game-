# Code Smells Analysis: `fps_observation_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`fps_observation_lab` proves 3D line-of-sight observation driving graph freezing over `observed_observation`.

---

## Identified Code Smells

### 1. Hardcoded Raycast Sight Depth
- **Category**: Primitive Obsession / Magic Numbers
- **Severity**: Low
- **Location**: [`src/vision.rs`](file:///o:/Observed%202/labs/fps_observation_lab/src/vision.rs)
- **Description**: Maximum corridor raycast depth is hardcoded in line-of-sight checks.
- **Impact**: Changing camera observation range requires editing source code constants.
- **Remediation**:
  - Apply **Extract Parameter Object**: Move line-of-sight range parameters into a `SightConfig` resource.

---

## Clean Aspects & Good Practices
- **Sight Follows Graph Topology**: Camera rays trace through rewired graph connections rather than physical world space, preserving non-Euclidean spatial mechanics.
