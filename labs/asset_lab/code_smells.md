# Code Smells Analysis: `asset_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`asset_lab` provides a visual showcase for the `observed_assets` drop-in manifest. It is a simple, focused UI lab with minimal code smells.

---

## Identified Code Smells

### 1. Manual Path String Formatting
- **Category**: Primitive Obsession
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/asset_lab/src/main.rs)
- **Description**: Displays paths by building string paths manually in UI formatters.
- **Impact**: Minor duplication of path formatting logic across overlay systems.
- **Remediation**:
  - Apply **Extract Method**: Use `observed_assets::slot_full_path` exclusively for UI path strings.

---

## Clean Aspects & Good Practices
- **Procedural Fallback**: Scene rendering falls back to procedural magenta meshes when files are missing, ensuring zero crash risk during asset evaluation.
