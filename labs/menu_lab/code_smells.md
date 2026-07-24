# Code Smells Analysis: `menu_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`menu_lab` tests state-scoped entity cleanup and UI screen transitions (`Splash -> Menu -> Loading -> Game -> Pause -> Results`).

---

## Identified Code Smells

### 1. Repeated Despawn Component Boilerplate
- **Category**: Dispensables / Duplicate Code
- **Severity**: Low
- **Location**: [`src/main.rs`](file:///o:/Observed%202/labs/menu_lab/src/main.rs)
- **Description**: Spawns `DespawnOnExit` on screen entities repeatedly.
- **Impact**: Minor verbosity in UI screen setup functions.
- **Remediation**:
  - Apply **Extract Method**: Use helper command extensions (`commands.spawn_screen_root()`).

---

## Clean Aspects & Good Practices
- **Strict Entity Teardown**: Guarantees zero entity leaks across 10 repeated menu-to-game transition cycles.
