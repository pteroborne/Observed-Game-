# Code Smells Analysis: `team_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`team_lab` evaluates 2-team 4-player resource contention, narrow passages, and squad cohesion lines over `observed_core::TeamId`.

---

## Identified Code Smells

### 1. Hardcoded Contention Resolution Order
- **Category**: Primitive Obsession / Determinism Assumptions
- **Severity**: Low
- **Location**: [`src/model.rs`](file:///o:/Observed%202/labs/team_lab/src/model.rs)
- **Description**: Simultaneous item grabs or station entries resolve in ascending `PlayerId` order.
- **Impact**: P1 always wins simultaneous tiebreaks against P2/P3/P4.
- **Remediation**:
  - Apply **Introduce Round-Robin / Seeded Tiebreaker**: Use round-robin priority tokens if equal contention priority is needed.

---

## Clean Aspects & Good Practices
- **Multi-Player Assumptions**: Completely avoids single-player global resources, supporting multiple concurrent controllers, AI bots, and network clients.
