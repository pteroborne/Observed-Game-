# Code Smells Analysis: `tools/trenchbroom`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`tools/trenchbroom` provides editor setup scripts and `.fgd` game configuration files for TrenchBroom.

---

## Identified Code Smells

### 1. Hardcoded APPDATA Target Path
- **Category**: Primitive Obsession / Hardcoded Configuration
- **Severity**: Low
- **Location**: [`install_config.ps1`](file:///o:/Observed%202/tools/trenchbroom/install_config.ps1)
- **Description**: Default destination joins `$env:APPDATA\TrenchBroom\games\Observed 2`.
- **Impact**: Non-Windows or custom TrenchBroom installations require passing the `-Destination` parameter manually.
- **Remediation**:
  - Keep default parameter for standard Windows setup.

---

## Clean Aspects & Good Practices
- **Data-Driven FGD Definition**: TrenchBroom `.fgd` file maps `observed_module` and `observed_door` entity definitions directly without hardcoded brush indices.
