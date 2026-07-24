# Code Smells Analysis: `hex_room_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`hex_room_lab` showcases multi-cell hex room composition and district key/fill lighting across all 9 architecture registers.

---

## Identified Code Smells

### 1. Hardcoded Room Anchor Offset
- **Category**: Primitive Obsession / Hardcoded Configuration
- **Severity**: Low
- **Location**: [`src/lib.rs:ANCHOR`](file:///o:/Observed%202/labs/hex_room_lab/src/lib.rs#L56-L60)
- **Description**: Anchor hex coordinate `HexCoord { q: 24, r: 24, level: 0 }` is hardcoded to offset grid cells.
- **Impact**: Multi-cell room centering is bound to hardcoded coordinate space offsets.
- **Remediation**:
  - Apply **Extract Method**: Compute centroid offsets dynamically based on blueprint extent bounds.

---

## Clean Aspects & Good Practices
- **Dollhouse Cutaway Filter**: Drops ceiling convex hulls dynamically to allow clear top-down visual inspection of room interiors and lighting setups.
