# Code Smells Analysis: `semantic_vfx_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`semantic_vfx_lab` evaluates `bevy_hanabi` for event-driven GPU particle effects (door slams, reroute flashes, pressure gates).

---

## Identified Code Smells

### 1. Prototype Seam (Candidate Crate Integration)
- **Category**: Dispensables / Speculative Generality
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/semantic_vfx_lab/src/main.rs)
- **Description**: `bevy_hanabi` particle effects are kept isolated inside the lab crate.
- **Impact**: Candidate particle system is not re-exported for workspace crates yet.
- **Remediation**:
  - Keep candidate crate lab-local.

---

## Clean Aspects & Good Practices
- **Zero Simulation Effect**: Disabling particle projection (`V` key) leaves fixed-step simulation timelines and signal anchors completely unaltered.
