# Code Smells Analysis: `content_manifest_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`content_manifest_lab` validates typed JSON content schemas (`content_manifest.json`) against `observed_content`.

---

## Identified Code Smells

### 1. Hardcoded Path Relative to Cwd
- **Category**: Primitive Obsession / Path Coupling
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/content_manifest_lab/src/main.rs)
- **Description**: Expects `assets/content_manifest.json` relative to execution working directory.
- **Impact**: Execution fails if launched from outside the workspace root.
- **Remediation**:
  - Apply **Extract Method**: Use `observed_assets::assets_root()` to build canonical absolute path.

---

## Clean Aspects & Good Practices
- **Strict Provenance Verification**: Validates SHA-256 fingerprints and CC0 license strings before instantiating meshes.
- **Presentation-Only Assets**: Imported GLTF materials are overridden by `observed_style` treatments to enforce the Legibility Contract.
