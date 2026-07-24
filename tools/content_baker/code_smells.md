# Code Smells Analysis: `tools/content_baker`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`content_baker` is an offline CLI tool baking TrenchBroom `.map` brush entities into canonical JSON convex module files (`assets/content/modules/`).

---

## Identified Code Smells

### 1. Primitive Bit-Winding Comparison
- **Category**: Primitive Obsession
- **Severity**: Low
- **Location**: [`src/main.rs:brush_points`](file:///o:/Observed%202/tools/content_baker/src/main.rs#L91-L104)
- **Description**: Uses raw `to_bits()` equality checks on float arrays to deduplicate 3D hull vertices.
- **Impact**: Floating-point corner precision variations could produce duplicate vertices if precision differs slightly.
- **Remediation**:
  - Apply **Introduce Epsilon Tolerance**: Use an epsilon-distance check (`(a - b).length_squared() < 1e-6`) for vertex deduplication.

---

## Clean Aspects & Good Practices
- **Deterministic Offline Baking**: SHA-256 fingerprints ensure baked module JSON files remain bit-identical across runs, eliminating runtime map parsing overhead.
