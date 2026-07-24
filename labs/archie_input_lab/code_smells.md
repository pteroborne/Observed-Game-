# Code Smells Analysis: `archie_input_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`archie_input_lab` evaluates the `bevy_archie` candidate crate for multi-controller input binding. It cleanly isolates archie data types inside an adapter (`src/adapter.rs`).

---

## Identified Code Smells

### 1. Global vs Per-Player State Mismatch (Inappropriate Intimacy)
- **Category**: Couplers / Object-Orientation Abusers
- **Severity**: Medium
- **Location**: [`src/adapter.rs`](file:///o:/Observed%202/labs/archie_input_lab/src/adapter.rs)
- **Description**: `bevy_archie` provides a global `ActionState` resource combining all inputs. The lab wraps this in custom per-player sampling to avoid merging P1–P4 controller intents.
- **Impact**: Requires manual iteration over device IDs in lab adapter code.
- **Remediation**:
  - Apply **Hide Delegate / Extract Adapter**: Keep device sampling strictly contained inside `adapter.rs` so callers only see isolated `PlayerId` intents.

---

## Clean Aspects & Good Practices
- **Strict Intent Boundary**: Translates gamepad and rebind actions into `player_input::PlayerIntent` without polluting gameplay logic.
- **Resettable Lab State**: Cleanly tears down UI probes on `R` reset.
