# Phase 72 — Arc Gate: Evidence Refresh & Playtest

**Objective:** the Arc H ship-gate pattern, rerun for Light & Line: one final
build, full evidence refresh, everything viewed, then the user plays and the arc
closes when the checklist passes. Read [README.md](README.md) first.

## Read first

- `docs/arc_h/phase_66_summary_ship_gate.md` — the pattern, including the
  "Ship-gate evidence refresh — 2026-07-11" section's capture environment
  requirements (`OBSERVED2_BOTS=all`, Vulkan for world scenes, dx12 for
  Results, the phase-0 re-assert rule).
- Phase 67–71 as-landed notes — every claim this gate re-verifies.
- `docs/bug_backlog.md` — findings land here first.

## The gate

1. **Scripted evidence set, one build, all viewed:** full visual audit (with the
   luminance corridor), bot-POV GIF (must reach the exit — Phase 67's fix),
   per-district light-identity contact sheet, geometry-grammar captures, the
   lighting_lab nine (re-run confirms the lab still boots), drained/klaxon under
   the new registers, one results-screen capture (dx12) confirming nothing
   regressed.
2. **User playtest checklist** (results recorded in the as-landed note):
   - [ ] Walk a full match: does traversal *feel* liminal — and does the feel
         change district to district?
   - [ ] The overlit district: does it read as intent (pristine wrongness), not
         a rendering bug?
   - [ ] Signal legibility spot-check: find a keystone, an anchor, the exit in
         the darkest and the brightest districts without frustration.
   - [ ] Audio: is the Phase-67 mix right in real play (beds present, cues not
         startling)?
   - [ ] Comfort: no strobe-feel from breathing shadows; decoherence still
         diegetic.
3. Findings go to `docs/bug_backlog.md`; the arc closes when the checklist
   passes, not when the tests do.

## Files you may edit

Docs, evidence, and ledger only (`ROADMAP.md`, `docs/arc_i/`, `docs/evidence/`,
memory). Any code fix discovered here goes through the backlog unless the parent
rules it a same-day trivial fix.

## Success criteria

- Every capture listed above exists, was produced by the final build, and was
  viewed; the as-landed note describes each in a sentence.
- The luminance corridor passes on every world capture.
- The user checklist is complete with results recorded; findings (if any) are in
  the backlog with self-contained entries.
- ROADMAP, Recent Milestones, and memory updated; the arc's phases each landed
  as their own commit.
