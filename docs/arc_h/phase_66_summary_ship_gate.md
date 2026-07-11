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

## Implementation status — 2026-07-11 (ship gate still open)

- The results screen now builds a pure seven-line `ResultsStory` from the series
  result plus the passive replay trace: final standing, escape order, rooms the
  collapse sealed, the local room path, keystone progress, anchor deployments,
  and replay identity. Win, placed, absorbed/last, and true-solo shapes are
  regression-tested; solo emits no rival-team line.
- `ReplayTape` remains presentation-only and now retains those story facts while
  Match-scoped simulation resources are cleaned up. It reads existing state and
  never drives the match.
- The selected results action is a direct rematch: Enter/A launches Match with
  the same bot configuration and a guaranteed different seed. Replay and main
  menu remain adjacent choices, with the existing UI click sound path intact.
- Unknown `OBSERVED2_BOTS` tokens now log a warning and return the all-on default
  instead of panicking. The all-on `MatchDirector::new` corpus digest is pinned at
  `0x539C35C626B9B30F`.
- `OBSERVED2_CAPTURE_RESULTS=<dir>` plus `OBSERVED2_RESULTS_SHAPE` stages one
  deterministic outcome (`victory`, `placed`, `absorbed`, or `solo`). On this
  Windows host, `WGPU_BACKEND=dx12` is required for reliable UI readback; Vulkan
  produced incomplete screenshot damage regions even though the live UI was
  intact.
- Human-viewed evidence: [four-outcome contact
  sheet](../evidence/phase_66_results/contact.png), plus the source [victory](../evidence/phase_66_results/00_victory.png),
  [placed](../evidence/phase_66_results/01_placed.png),
  [absorbed](../evidence/phase_66_results/02_absorbed.png), and
  [solo](../evidence/phase_66_results/03_solo.png) frames. The contact sheet
  visibly shows all four complete story panels and the direct rematch choice.
- Verification passed warning-free: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets`, and `cargo test --workspace`
  (88.3 s); the focused assembled-game suite is 232/232.

### Ship-gate checklist

- [ ] Refresh and view the bot-POV GIF and full visual audit as one final build.
- [x] View one result capture for every outcome shape.
- [x] View the Phase-65 observation-room evidence.
- [ ] Confirm/view the Phase-63 rebind overlay evidence.
- [ ] Confirm/view the Phase-62 dressed-room evidence.
- [ ] Confirm/view the Phase-64 WFC threshold evidence.
- [ ] User: complete first-run onboarding through a full match.
- [ ] User: complete a true-solo match.
- [ ] User: rebind a key and hand-audit backlog fixes #1–#4.
- [ ] User: listen through beds, distinct cues, and the klaxon loop.

The implementation is ready for that gate, but the MVP is not declared shipped
until every unchecked item passes and any findings are recorded in the backlog.
