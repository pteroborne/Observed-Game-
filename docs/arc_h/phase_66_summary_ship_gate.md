# Phase 66 — Post-Match Summary & the Ship Gate

**Objective:** close Arc E Phase 50's remaining half — a results screen that reads
the run's *story*, not just its outcome — then run the arc's ship gate: hygiene
items, a scripted full-loop evidence set, and a human playtest checklist. When
this phase lands, the MVP is declared shipped. Read [README.md](README.md) first.

## Read first

- `docs/arc_e/phase_50_hud_results_clarity.md` — the original intent and the
  2026-07-09 immersion redirection; the tac-map/HUD half landed, the results
  half did not.
- `game/src/screens/menu.rs` `setup_results` + `GameState::Results` — the current
  minimal results screen this phase grows.
- `game/src/sim/director.rs` — `MatchDirector`/`EliminationSeries` outcome API:
  placement authority is `outcome()`; the run's events (escapes, eliminations,
  collapse claims) are the story's raw material. **Read-only** — if a statistic
  isn't exposed, add presentation-side accumulation from existing events, not
  new sim state.
- `docs/bug_backlog.md` "Minor / hygiene" — the two items this phase closes.

## Design rulings (already decided)

- **The summary tells the run's story, legibly:** final placement and series
  standing; who escaped and in what order; what the collapse took (rooms lost);
  the player's own path highlights (rooms discovered, keystones, anchor uses) —
  laid out in `observed_style` typography, readable in one glance per the
  Legibility Contract. No stats dump; five to eight lines of story.
- **Every outcome shape renders:** win / placed / absorbed / solo (Phase 54's
  true-singleplayer match must produce a sane solo summary — no rival lines).
- **A clean path back into the loop:** one keypress to rematch (same config,
  new seed) or return to menu; the summary screen respects UI sounds and
  despawns cleanly (screens are state-scoped; the existing no-leak discipline).
- **Hygiene (from the backlog):** `BotPopulations::from_str` logs a warning and
  falls back to default on an unknown token instead of panicking; add the
  all-on-default digest characterization test (`MatchDirector::new` with default
  config produces a pinned digest across the seed corpus sample).

## The ship gate (parent session + user, after the code lands)

1. **Scripted evidence set:** refresh the bot-POV GIF, the full visual audit,
   and one capture per: results screen (each outcome shape), observation room,
   rebind overlay, a dressed room, a WFC hallway with clean thresholds. All
   viewed.
2. **Human playtest checklist** (the user runs it; the phase doc's as-landed
   note records the results): first-run onboarding through a full match; a solo
   match; rebind a key; audit each bug-backlog fix by hand; listen for the audio
   palette (beds localize, cues distinct, klaxon loop right).
3. Anything found goes to `docs/bug_backlog.md`; the MVP ships when the
   checklist passes, not when the tests do.

## Files you may edit

`game/src/screens/menu.rs` (results), `game/src/flow.rs` (rematch path),
`game/src/sim/director.rs` (**only** `BotPopulations::from_str` panic→warning),
`game/src/view/theme.rs` (summary typography helpers), `game/src/tests.rs`,
`game/src/evidence/` (results-screen capture support). Do NOT add sim state or
change match/series rules.

## Success criteria

- Tests render every outcome shape's summary data (pure summary-building
  function unit-tested per shape, including solo); rematch/menu paths covered by
  the screen-lifecycle tests; `OBSERVED2_BOTS=garbage` boots with a logged
  warning and default bots; the digest characterization test pins the all-on
  default.
- A capture of each outcome-shape summary, viewed; the report describes each.
- The ship-gate checklist runs end-to-end and its results are recorded in the
  as-landed note.
- Full verification recipe green.
