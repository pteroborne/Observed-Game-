# Code Smells Analysis: `director_lab`

Based on the [Refactoring Skill Taxonomy](file:///o:/Observed%202/.agents/skills/refactor/SKILL.md) and [Refactoring.Guru Code Smells Catalog](file:///o:/Observed%202/.agents/skills/refactor/references/code_smells.md).

## Summary
`director_lab` evaluates the AI Director and collapse line mechanics. Logic is promoted to `observed_match::director`.

---

## Identified Code Smells

### 1. Prototype Seam (Promoted Module Re-export)
- **Category**: Dispensables / Legacy Seams
- **Severity**: Low
- **Location**: [`src/lib.rs`](file:///o:/Observed%202/labs/director_lab/src/lib.rs)
- **Description**: Re-exports types promoted to `observed_match::director`.
- **Impact**: Lab acts as a visual projection for the match crate.
- **Remediation**:
  - Keep current re-export structure intact.

---

## Clean Aspects & Good Practices
- **Escalation Feedback Loop**: Falling behind cleanly transitions teams from runners to director manipulators without modifying physical character velocity formulas.
