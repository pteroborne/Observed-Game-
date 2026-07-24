# Code Smells Analysis: `observed_content`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`observed_content` enforces strict deny-unknown-fields schemas and cryptographic hashing. The code is highly robust, with `TraversalProfile` value objects and safe JSON hashing methods fully integrated.

---

## Resolved Code Smells

### 1. Large Data Struct / Primitive Obsession (`TraversalProfile`) — RESOLVED
- **Status:** Resolved.
- **Details:** Applied **Extract Struct** to introduce cohesive domain value objects (`KinematicSpeeds`, `CapsuleShape`, `StepLimits`) and convenience accessors (`speeds()`, `shape()`, `step_limits()`) while preserving 100% backward JSON schema serialization compatibility.

### 2. Panic in Internal Utility (`hash_json`) — RESOLVED
- **Status:** Resolved.
- **Details:** Added `try_hash_json(value: &impl Serialize) -> Result<[u8; 32], serde_json::Error>` returning a `Result` for safe error propagation alongside `hash_json`.

---

## Clean Aspects & Good Practices
- **Strict Validation**: All structs enforce `#[serde(deny_unknown_fields)]` preventing accidental silent schema corruption.
- **Dual Hash Integrity**: `SimulationContentHash` and `PresentationContentHash` explicitly decouple gameplay state determinism from optional aesthetic assets.
