# Code Smells Analysis: `contention_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`contention_lab` tests shared team observation, anchors, and private team-local fog-of-war ledgers over `observed_observation::contention`.

---

## Identified Code Smells

### 1. Hardcoded 3x3 Grid Layout Assumptions
- **Category**: Hardcoded Configuration / Primitive Obsession
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/contention_lab/src/main.rs)
- **Description**: Hardcodes room 0, 2, 6, 8 as team spawn corners and room 8 as exit.
- **Impact**: Lab setup assumptions are fixed to a 3x3 square grid.
- **Remediation**:
  - Apply **Extract Parameter Object**: Move spawn assignments into a `MatchLayoutSpec` config.

---

## Clean Aspects & Good Practices
- **Shared vs Private Knowledge Isolation**: Cleanly separates shared graph reality from private team fog-of-war ledgers without desynchronizing network state.
