# Code Smells Analysis: `guardian_ai_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`guardian_ai_lab` tests the weeping-angel style Guardian AI adversary (freeze under sight/anchor light, path toward player, banish on touch).

---

## Identified Code Smells

### 1. Magic Number Timers (`30.0s` Banishment)
- **Category**: Primitive Obsession / Magic Numbers
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/guardian_ai_lab/src/main.rs)
- **Description**: Anchor banishment duration (`30.0` seconds) is hardcoded.
- **Impact**: Changing banishment timer requires editing source code constants.
- **Remediation**:
  - Apply **Extract Constant**: Move timer duration to a `GuardianConfig` struct.

---

## Clean Aspects & Good Practices
- **Observe-to-Freeze AI Rule**: Reuses camera observation semantics so the adversary freezes when looked at, creating high tension without combat systems.
