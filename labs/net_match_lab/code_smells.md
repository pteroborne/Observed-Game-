# Code Smells Analysis: `net_match_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`net_match_lab` tests deterministic lockstep action replication (`LiveNetMatch`) over simulated hostile networks (`observed_net`).

---

## Identified Code Smells

### 1. Prototype Seam (Promoted Netmatch Code)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/net_match_lab/src/lib.rs)
- **Description**: Re-exports types promoted to `observed_net::netmatch`.
- **Impact**: Lab acts as a visual projection for lockstep networking.
- **Remediation**:
  - Retain current clean re-export structure.

---

## Clean Aspects & Good Practices
- **Network Profile Independence**: Proves that clean vs hostile transport profiles (drop, dupe, reorder) land on identical bit-for-bit simulation states.
