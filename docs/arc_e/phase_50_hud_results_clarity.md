# Phase 50 — HUD & Results Clarity

**Objective:** a first-timer parses "where's the exit, what do I still need, who's ahead,
how close is the collapse" at a glance, and the results screen tells the run's story
legibly. Read [README.md](README.md) first.

> **2026-07-09 immersion redirection (supersedes rulings 1–2 below).** The first HUD
> pass shipped an always-on multi-line status panel; playtest direction reversed it:
>
> - **Normal play is HUD-free.** The status readouts and the legend spawn only under
>   the app-level `DebugHud` flag (`OBSERVED2_DEBUG_HUD`, implied by
>   `OBSERVED2_VIS_AUDIT`/`OBSERVED2_FREECAM`). The world communicates diegetically;
>   the pause overlay and Tab tac-map remain player-facing.
> - **The tac-map is a survivor's sketch, not a blueprint.** A `MapKnowledge` ledger
>   (sibling of `RivalSightings`, written in `sim/knowledge.rs` from the player's
>   teleport place) records visited rooms, doorway-glimpsed rooms, and witnessed
>   connections; `tacmap::build_map` filters ALL structure through it. Unknown rooms
>   are absent, glimpsed rooms are hollow outlines, the exit marker appears only once
>   the exit room is witnessed, keystone/collapse/rival marks only on known rooms, and
>   a witnessed edge a reroute removed silently drops off. Bounds/sizing come from the
>   full facility so discovered rooms never shift. Rival team labels are
>   spectator-mode-only; the series status line is debug-HUD-only.

## Read first

- `game/src/screens/hud.rs` — `spawn_match_hud` (status panel, pause, tac-map panel,
  legend), `match_draw`, `draw_tac_map`, `update_teleport_animation`. The tac-map already
  demotes rival data to the Phase-42 `RivalSightings` ledger (fog of war — keep it).
- `game/src/tacmap.rs` — `build_map` (routes, rooms, exit, keystones, rival pips from
  sightings, player, series status) and its element model; the readability assertions
  live against this.
- `game/src/keystones.rs` (`exit_room`, keystone set/progress),
  `crates/observed_match/src/facility.rs` (`collapse_rooms`/`collapse_frontier`,
  `room_collapse`, `objective_sequence`, `team_progress`, `standings`),
  `crates/observed_match/src/elimination.rs` (series round / adversary / countdown).
- `game/src/screens/menu.rs` `setup_results` + `game/src/flow.rs` `MatchResult`
  (`placement`, `escaped`, `absorbed`, `winner`, `local_won`) and `resolve_series`.

## Design rulings (already decided)

- Readability over density: the HUD answers the four questions above without clutter;
  signals stay legend-backed (`observed_style`) and Legibility-Contract-safe.
- Fog of war stays: the tac-map shows the player's *sightings*, not live rival truth
  (Phase 42) — clarify presentation, don't leak omniscience.
- Results reads the **series** outcome (the placement authority), consistent with the
  match-end invariant (README #6).
- No gameplay changes — this is a presentation/readability pass.

## Files you may edit

`game/src/screens/hud.rs`, `game/src/tacmap.rs`, `game/src/screens/menu.rs`
(`setup_results`), `game/src/view/{theme.rs, components.rs}` (HUD chrome only),
`game/src/tests.rs`. Do NOT change the brain, the sighting ledger's rules, or match-end.

## Implementation

1. **Objective legibility (in-match HUD).** A compact, always-legible objective readout:
   the exit's location/direction, keystone progress (have / needed, from `KeystoneState`),
   and the collapse frontier (`collapse_frontier`/`room_collapse`) — so the player always
   knows the goal, the gate, and the threat. Reuse `observed_style` marker/surface colours;
   add legend lines for anything new.
2. **Standing at a glance.** Surface the series standing / who's ahead (`standings`,
   `team_progress`, escaped/absorbed) in a glanceable form; keep the tac-map the detailed
   view, the HUD the summary.
3. **Tac-map polish.** Tighten `build_map` presentation (route/room/exit/keystone/rival-
   pip/collapse legibility) without changing what it's allowed to show; keep the pip
   staleness/fade and Heard-vs-Seen distinction.
4. **Results & summary.** Rework `setup_results` into a post-match summary that reads the
   `MatchResult` + series: placement, escapes, what the collapse took, and the series
   standing — every outcome shape (won / placed / absorbed / last) rendered clearly, with
   a clean path back into the loop (Menu / play again).
5. **First-time hints** hook: coordinate with Phase 48's onboarding flag if present
   (context hints for the objective/tac-map on first exposure).

## Tests

- `hud_shows_objective_keystone_and_collapse_readouts_on_the_generated_map` — the
  in-match HUD spawns the objective/keystone/collapse elements against
  `map_catalog::default_map_spec` (not authored-map literals).
- Tac-map element counts still match the sighting-driven model (extend the existing
  tac-map test rather than loosening it).
- `results_screen_renders_every_outcome_shape` — win / placed / absorbed / last each
  render without panic and show the right summary.
- Results reads the series result (placement authority), consistent with the match-end
  invariant.

## Verification

Per the README recipe; refresh a HUD/results evidence screenshot. Report: the HUD
readouts added, the results summary shape, the tac-map polish (what changed vs. what it's
still forbidden to show), test list, verification results.
