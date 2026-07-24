# Code Smells Analysis: `observed_net`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_net` provides lockstep network synchronization. Past refactoring successfully consolidated duplicate error types (`PacketError`).

---

## Identified Code Smells

### 1. Large Class / Long File (`netmatch.rs` & `network.rs`)
- **Category**: Bloaters
- **Severity**: Medium
- **Location**: [`src/netmatch.rs`](file:///o:/Observed%202/crates/observed_net/src/netmatch.rs) & [`src/network.rs`](file:///o:/Observed%202/crates/observed_net/src/network.rs)
- **Description**: Both files exceed 600 lines, mixing packet buffer management, delay simulation, resend requests, and state ticking.
- **Impact**: Makes finding specific networking logic (e.g. resend buffer vs checksum calculation) difficult.
- **Remediation**:
  - Apply **Extract Class / Module**: Split `network.rs` into `network/buffer.rs`, `network/repair.rs`, and `network/syncer.rs`.

---

## Clean Aspects & Good Practices
- **Completed Consolidation (`PacketError`)**: Duplicate error types across `protocol.rs` and `netmatch.rs` were successfully unified into the root `lib.rs`.
- **Content Hash Verification**: Wire protocols verify `SimulationContentHash` before accepting frames, preventing desyncs caused by mismatched client assets.
