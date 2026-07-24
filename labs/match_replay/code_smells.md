# Code Smells Analysis: `match_replay`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`match_replay` tests recording and replaying full match input tapes for spectator scrubbers and network playback. Logic was promoted into `observed_match::replay`.

---

## Identified Code Smells

### 1. Prototype Seam (Promoted Re-export)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/match_replay/src/lib.rs)
- **Description**: Re-exports types promoted to `observed_match::replay`.
- **Impact**: Lab acts as a spectator projection over match tapes.
- **Remediation**:
  - Retain current clean re-export structure.

---

## Clean Aspects & Good Practices
- **Deterministic Replay Guarantee**: Proves that seeking to round N reproduces match states bit-for-bit, verifying that `seek(N) == step(N)`.
