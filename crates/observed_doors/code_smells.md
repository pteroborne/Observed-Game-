# Code Smells Analysis: `observed_doors`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_doors` cleanly implements the door-as-observation-gate mechanic. The model is pure, stateful, and deterministic. Hardcoded spine configuration assumptions have been resolved.

---

## Resolved Code Smells

### 1. Hardcoded Lattice Assumptions (`SPINE_ROOMS`) — RESOLVED
- **Status:** Resolved.
- **Details:** Applied **Introduce Parameter Object** to encapsulate spine room configurations into `SpineConfig` with `DoorWorld::with_config(config)`. `DoorWorld::authored()` delegates to `SpineConfig::default()` for full backwards compatibility.

---

## Clean Aspects & Good Practices
- **Deterministic Re-matching**: Uses `SplitMix` with explicit seed mixing (`base_seed ^ decoherence_count`), making rewiring fully reproducible in replay tests.
- **State Audit Tracking**: Exposes exact metrics (`decoherence_count`, `rewires_last`, `slams`, `reopened_changed`) for automated test verification.
