# Code Smells Analysis: `fps_hybrid_match_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`fps_hybrid_match_lab` was the capstone lab for 3D first-person hybrid matches in rerouting mazes. Its core model was promoted into `observed_match::hybrid`.

---

## Identified Code Smells

### 1. High Complexity Integration Orchestrator
- **Category**: Bloaters (Large Class / Divergent Change)
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/fps_hybrid_match_lab/src/main.rs)
- **Description**: Orchestrates player controllers, WFC reroute deferral checks, replay tapes, Tac-Map rendering, and match director ticks.
- **Impact**: Lab setup requires wiring multiple heavy resources.
- **Remediation**:
  - The underlying model was already refactored into `observed_match::hybrid` submodules (`match_state.rs`, `round_step.rs`, `replay.rs`).

---

## Clean Aspects & Good Practices
- **Atomic Off-Camera Rerouting**: Deferred corridor commits prevent visual geometry popping or physics clipping while a player is inside or looking at a hallway.
