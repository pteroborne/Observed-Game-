# Code Smells Analysis: `lab_observability_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`lab_observability_lab` evaluates `bevy_mod_config` for typed config persistence and event tracing while rejecting egui-coupled dependencies (`bevy_log_events`).

---

## Identified Code Smells

### 1. Dual Seed State (Candidate vs Active Manifest)
- **Category**: Change Preventers / Complex State Synchronization
- **Severity**: Low
- **Location**: [`src/lab.rs`](file:///o:/Observed%202/labs/lab_observability_lab/src/lab.rs)
- **Description**: Maintains `candidate_seed` in config and `active_seed` in `ActiveManifest` to isolate non-deterministic live tweaks from deterministic simulation restarts.
- **Impact**: Requires explicit `Enter` key presses to commit candidate seeds into active manifests.
- **Remediation**:
  - Keep this dual-state separation to protect simulation determinism.

---

## Clean Aspects & Good Practices
- **Egui Rejection Discipline**: Rejects `bevy_log_events` because it forcibly pulls an egui UI dependency stack, keeping the workspace build lean with zero unwanted dependencies.
