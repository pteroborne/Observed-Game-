# Phase 67 — Hygiene Opener: Audio Mix & Bot-POV Stall

**Objective:** close the two small post-ship defects from the 2026-07-11 ship-gate
playtest so the arc's real work starts on a clean slate: rebalance the audio mix
(backlog #7) and fix the bot-POV walkthrough stall (backlog #5). Read
[README.md](README.md) first.

## Read first

- `docs/bug_backlog.md` #5 and #7 — the findings, self-contained.
- `game/src/screens/audio.rs` — the `AudioDirector`: bus model, cue table,
  sting-ducks-ambience easing, per-class cooldowns. The mix problem lives in the
  gain constants here, not in the generated assets.
- `game/src/evidence/capture/bot_pov.rs` — the walkthrough driver: how it picks
  the bot's next target after force-collecting keystones, and what happens when
  the route stalls in Room 11 (seed 1, shot ~55 of 120).
- `docs/arc_f/phase_56_audio_content_spatial.md` — the coverage-table discipline
  any cue-level change must keep satisfied.

## Design rulings (already decided)

- **Mix, not regeneration.** The user's verdict: effects too loud, ambience beds
  too quiet. Adjust gain staging (bed sink levels up, one-shot cue levels down)
  — do not regenerate the palette (`tools/generate_audio.py` untouched).
- **Settings still scale sensibly.** Master/SFX/music sliders must behave around
  the new defaults; no channel may clip at 100%.
- **The bot walkthrough must reach the exit.** Diagnose why the bot stops in
  Room 11 — likely the post-keystone nav target or a wait on a door/decoherence
  event that never fires while the capture drives the match. The fix stays in
  the evidence layer if possible; if the stall reveals a real navigation defect,
  report it to the parent before touching `sim/`.

## Files you may edit

`game/src/screens/audio.rs` (gain constants / cue table),
`game/src/evidence/capture/bot_pov.rs`, `game/src/tests.rs`. Do NOT touch
`tools/generate_audio.py`, the generated assets, or `sim/`.

## Success criteria

- A short written mix table in the as-landed note: each bus/class, old level,
  new level, rationale.
- The audio coverage-table test still passes; a new test pins the bed-vs-cue
  gain relationship so the mix cannot silently regress.
- A fresh 120-frame bot-POV run reaches the exit (or the match end) with no
  repeated-position tail; the log's `BOT_CAPTURE_SHOT` positions are strictly
  progressing past shot 55. GIF rebuilt and **viewed**; the as-landed note
  describes what the last third shows.
- Full verification recipe green.
