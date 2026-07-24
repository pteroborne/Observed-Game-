# Code Smells Analysis: `topology_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`topology_lab` was the foundational 2D graph prototype. Its core graph matching models were later promoted to `observed_observation` and `observed_doors`.

---

## Identified Code Smells

### 1. Custom Linear Congruential PRNG (`SimpleRng`)
- **Category**: Dispensables / Reinventing the Wheel
- **Severity**: Low
- **Location**: [`src/model.rs`](file:///o:/Observed%202/labs/topology_lab/src/model.rs)
- **Description**: Uses a simple custom LCG PRNG for seed shuffling.
- **Impact**: Production graph decoherence uses `SplitMix` in `observed_observation`.
- **Remediation**:
  - Keep lab `SimpleRng` as an isolated prototype fixture.

---

## Clean Aspects & Good Practices
- **Pure Domain Separation**: Graph model (`model.rs`) and graph parsing logic (`logic.rs`) are pure Rust modules without Bevy ECS dependencies.
