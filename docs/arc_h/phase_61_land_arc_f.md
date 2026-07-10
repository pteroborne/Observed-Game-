# Phase 61 — Land Arc F (commits, ledger, honest evidence)

**Objective:** the entire Arc F output — seven phases, the regenerated audio
palette, the ambience-bed review fixes — sits uncommitted on top of `7720b1c`.
Land it in reviewed commits, square the ledger, and replace the evidence that is
missing or shows nothing. Nothing else in Arc H starts until this is committed.
Read [README.md](README.md) first. This phase suits the parent session or one
careful agent; it changes no behavior.

## Deliverables

1. **Commits.** Stage the working tree into logical commits (house message style,
   Co-Authored-By trailer). A sane split: Arc F planning docs; Phase 54; Phases
   55–56 + palette + review fixes; Phase 57 (lab + derived assets); Phases 58–59;
   Phase 60; Arc H planning docs. One combined "Arc F" commit is acceptable if
   splitting proves impractical — landed beats beautiful.
2. **ROADMAP.** Tick Phases 54, 55, 58, 60 `[x]` (56/57/59 already are); add one
   Recent-Milestones entry per Arc F phase (or one consolidated Arc F entry)
   following the existing format: what landed, key decisions, verification line.
3. **As-landed notes.** Append `## As landed` to phase docs 54, 55, 58, 59, 60
   (57 has one), each recording what shipped, deviations from the doc, and — for
   60 — the reticle ruling actually taken (minimal center dot, visible only near
   interactables).
4. **Honest evidence.**
   - `docs/evidence/oga_25d_rivals.png` currently shows an empty floor. Re-capture
     a played match with at least one rival (and ideally the guardian) visibly in
     frame; the capture driver may need to route the body to a rival-occupied room
     — see `game/src/evidence/` capture helpers.
   - Capture the missing Phase 60 dressing evidence (a dressed monitor/reactor
     room vs a sparse archive room).
   - Per the falsifiable-evidence rule: view every image; the report must say what
     each shows.
5. **Memory + Catalogue.** Update the sight-and-sound memory (arc complete
   2026-07-10, review caveats), ensure `Catalogue.md` lists `oga_25d_lab`.

## Do NOT

- Change any behavior, fix any bug, or "quickly improve" anything found along the
  way — file it in `docs/bug_backlog.md` instead. This phase is bookkeeping only,
  so its commits are trustworthy snapshots of what Arc F actually built.

## Success criteria

- `git status` is clean afterward; `git log` tells the arc's story; every commit
  builds and passes the full verification recipe on its own (verify at least the
  final one; note if intermediate commits were not individually tested).
- ROADMAP, phase docs, memory, and Catalogue agree with reality.
- The rivals capture visibly contains a rival; the dressing captures visibly show
  role-driven props; both are committed.
