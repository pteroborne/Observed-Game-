# Code Smells Analysis: `inspector_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`inspector_lab` is a concise, highly isolated adapter lab proving `bevy-inspector-egui`.

---

## Identified Code Smells

### 1. Conditional Feature Seam (`dev_tools`)
- **Category**: Dispensables / Speculative Generality
- **Severity**: Very Low
- **Location**: [`src/lib.rs:DevToolsPlugin`](file:///o:/Observed%202/labs/inspector_lab/src/lib.rs#L32-L40)
- **Description**: Uses `#[cfg(feature = "dev_tools")]` conditional compilation inside system setups.
- **Impact**: Code behavior varies based on feature flags during compilation.
- **Remediation**:
  - Maintain feature seam as required by workspace dependency rules (keeping heavy UI crates out of standard builds).

---

## Clean Aspects & Good Practices
- **Strict Dependency Isolation**: Feature-gating `bevy-inspector-egui` prevents dev tools from bloating normal game binary build times or release targets.
