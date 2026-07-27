# Bug Backlog

Known defects from playtesting, recorded 2026-07-09 (the original four were
fixed in Arc H and hand-audited in the 2026-07-11 ship-gate playtest). Open
entries are post-ship findings, unscheduled. New findings land here first, then
get scheduled. Keep entries self-contained enough to hand to an agent cold.

Entries 10–12 came out of the 2026-07-26 per-district FPS investigation; its
method, measurements, and the hypotheses it rejected are in
[evidence/arc_m/district_perf/FINDINGS.md](evidence/arc_m/district_perf/FINDINGS.md).
Read that before picking any of them up — several plausible-sounding causes were
measured and ruled out, and the harness has traps that invalidate naive runs.

## Open

### 5. Bot-POV walkthrough stalls in the observation room
**Scheduled: Arc I Phase 67** ([arc_i/phase_67_audio_mix_bot_stall.md](arc_i/phase_67_audio_mix_bot_stall.md)).
**Found 2026-07-11 during the Phase 66 ship-gate evidence refresh.** The
`OBSERVED2_CAPTURE_BOT` walkthrough (all bots on, seed 1) reaches Room 11 — the
tether observation room — around shot 55 of 120 and never moves again; the last
half of the GIF is a frozen frame. The committed 2026-07-10 GIF shows the same
tail-freeze trait (frames 40–48 identical), so this is pre-existing, not an Arc H
regression. Look at: `game/src/evidence/capture/bot_pov.rs` (route driving) and
whatever nav target the bot chases after keystones are force-collected — it may
be waiting on a door/decoherence that never happens while the capture holds
`runtime.done = false`.

### 6. World-space evidence captures render nearly black
**Scheduled: Arc I Phase 69** ([arc_i/phase_69_light_staging_gate.md](arc_i/phase_69_light_staging_gate.md)).
**Root cause found 2026-07-11 (Phase 69 completion pass): two stacked causes.**
(a) The bird's-eye diagnostics (`OBSERVED2_CAPTURE_ROOM`, camera y=42) sit far
beyond every district's `fog_end` (22–28 m) — those captures photographed pure
distance fog in every version ever committed. Fixed: the driver now relaxes fog
like the audit's footprint atlas. (b) Eye-level frames had zero directional
light (no shadow-casting sources existed, and the first key-light attempt spawned
above the shadow-casting ceiling). Fixed: polygon-aware key placement below the
ceiling. Remaining: re-capture the Phase-62 set and verify the corridor gate
live, then close.
**Found 2026-07-11 during the Phase 66 ship-gate evidence refresh.** The
`phase_62_*` capture set (match, long hallway, hallway doorway, drained room) and
the visual audit's geometry/lighting scenarios render as near-black voids — the
committed Phase 62 evidence has looked like this since it landed, so Phase 62's
own success criteria ("two districts unmistakable in a capture", "no smeared
textures on long walls") have never actually been demonstrated by its captures.
Rooms lit by emissives (observation monitors, torch glow) read fine, and the
style-presence material check passes, so this is either (a) capture vantages
pointing at unlit space, (b) the screenshot path losing HDR/bloom that the live
render has, or (c) the world genuinely being too dark to read — the human
playtest adjudicates which. Look at: `game/src/evidence/capture/scenarios.rs`
(camera staging), `game/src/screens/match_runtime/ambience.rs` (palette
luminance), Bevy screenshot tonemapping.

### 7. Audio mix balance: effects too loud, ambience too quiet
**Scheduled: Arc I Phase 67** ([arc_i/phase_67_audio_mix_bot_stall.md](arc_i/phase_67_audio_mix_bot_stall.md)).
**Found 2026-07-11 during the Phase 66 user playtest listen-through.** The
relative mix is off: one-shot effects (cues, UI, stings) overpower the ambient
beds, and the district/location beds sit too quiet to register as atmosphere.
Rebalance the default gain staging between effect cues and beds — this is a mix
change, not a regeneration; the palette itself is fine. Look at:
`game/src/screens/audio.rs` / the `AudioDirector` gain constants
(bed sink levels vs. cue playback volumes), and confirm the settings sliders
(master/SFX/music) still scale sensibly around the new defaults.

### 8. Hex lighting is washed out before its semantic rig takes effect
**Unscheduled; candidate opener for the post-Arc-L polish/performance arc.**
**Found 2026-07-18 during the Phase 95 closure playtest.** The first visible
hex-game frames look washed out before the hex-specific lighting rules pop in;
once the semantic rig is active, the intended presentation reads much better.
Treat this as initialization/staging correctness, not a request for ad-hoc
colors: inspect `game/src/hex_wfc/view/lighting.rs`, state-enter ordering, camera
HDR/tonemapping, fog, and material readiness. The acceptance gate is that the
first player-visible frame already uses the correct `observed_style`-owned hex
lighting treatment, with no default-rig flash or exposure transition.

### 9. Hex mutation commit stalls the whole game
**Unscheduled; partially addressed 2026-07-26 — see below before starting.**
**Found 2026-07-18 during the Phase 95 closure playtest.** When a relayout
commits, the whole game visibly freezes while mutation geometry/state is rebuilt
and synchronized. Phase 93 already advances search attempts deterministically
over multiple ticks; the remaining hitch therefore needs profiling across the
commit, geometry snapshot, collider/render streaming, and entity synchronization
paths. Explore bounded continuous work — dirty-cell deltas, queued geometry and
collider projection, per-frame budgets, or equivalent — while retaining an
atomic authoritative topology commit. Visible/occupied/observed/pinned geometry
must remain stable, headless and interactive digests must agree, and replay
results must remain deterministic. The acceptance gate should include measured
frame-time bounds on a production `28 x 20 x 10` relayout, not merely a smaller
fixture.

**Progress 2026-07-26 (perf investigation, see #10).** One concrete cause found
and fixed: `view::spawn_cells` called `derive_trim` over the *whole* facility —
summarizing all ~109 k projected pieces and sweeping every cell — then discarded
everything but the handful of changed cells. `derive_trim_for` now scopes that to
the changed cells plus the lateral neighbours a seam classification needs.
Measured rebuild median roughly halved (16 651 → 8 455 µs). **Still stalls**: at
~8.5 ms a commit still overruns the 16.67 ms budget once the rest of the frame is
added, so mutation frames land at ~31–33 ms. The remaining cost is the respawn
itself (entity despawn/spawn for the changed cells), which is the amortisation
work this entry originally asked for.

### 10. Visibility propagation over 109 k entities is the largest frame cost
**Unscheduled; the top remaining hex performance item.**
**Found 2026-07-26 during the per-district FPS investigation** (full evidence and
method: [evidence/arc_m/district_perf/FINDINGS.md](evidence/arc_m/district_perf/FINDINGS.md)).
After the simulation fixes below, the single biggest cost in the frame is Bevy's
`PostUpdate` visibility work — `post_update:visibility` measures **3.9 ms**
(`VisibilityPropagate` + `CheckVisibility` + `MarkNewlyHiddenEntitiesInvisible`),
against `post_update:transform_propagate` at 0.97 ms and everything else under
0.1 ms.

This is an **entity-count problem, not an algorithmic one**: a production
`28 x 20 x 10` facility spawns **109 155 mesh entities** up front, and
`view::sync_streamed_cells` hides distant cells rather than removing them —
hiding stops a cell *rendering* but not its children being walked every frame. No
further micro-optimisation reaches this; it needs fewer entities. Two candidates,
both real refactors of `game/src/hex_wfc/view/shell.rs`:

- **Merge each cell's hull meshes into one mesh.** ~20× fewer entities (23 hulls
  per cell on average). Constrains per-piece materials to be uniform within a
  cell, so it interacts with the role/register tinting `observed_style` owns —
  check that before committing to it. Probably the bigger win and the lower
  behavioural risk.
- **Despawn out-of-stream cells instead of hiding them.** Entity count becomes
  proportional to the streaming window rather than the facility, which is the
  architecturally cleaner streaming story — but it moves cost to the streaming
  boundary, where it needs amortising to avoid recreating the spike that #9's
  `derive_trim_for` fix just removed.

Measure with the harness described in #11; `post_update:visibility` is reported
per register in `timings.json`.

### 11. Phase 101 performance gate fails on vsync headroom
**Unscheduled; low priority — the gate is stricter than the observed experience.**
**Found 2026-07-26.** `OBSERVED2_CAPTURE_HEX_WFC_PHASE101` fails at p95 ≈ 30 450 µs
against its ≤ 16 700 µs threshold, and **did not move across three rounds of real
fixes** (30 488 → 30 368 → 30 454) — so it was never measuring the simulation or
the rebuild. The gate runs vsync-capped, where the budget is one 16.67 ms refresh
interval and a miss costs a whole extra one. Uncapped, the same build sits at
~10.3 ms median / ~12.0 ms p95 — comfortable, but ~75 % of budget, so occasional
spikes tip over and land at 33 ms. Closing it means finding headroom in what
remains (#10 is where it is), or deciding the threshold should be measured
uncapped.

**Harness note for whoever picks this up.** The evidence harness in
`game/src/hex_wfc/perf.rs` grew several opt-in switches during the investigation,
all no-ops in normal play:
`OBSERVED2_CAPTURE_HEX_WFC_TICKS` (default 600 ticks is ~10 s, too short for the
bot to leave its spawn register), `..._UNCAPPED` (**essential** — vsync otherwise
pins every register to the same ~16 640 µs median and the data is worthless), and
`..._GPU` (per-render-pass GPU timings via `RenderDiagnosticsPlugin`). Reports
carry per-register frame/sim/visible-piece/fixed-step counts and per-`Update`-system
spans. Two traps: never pool medians across seeds (the same register runs 17 ms in
one facility and 112 ms in another), and treat thin sample buckets as unmeasured.

### 12. Guardian still routes per player and per blueprint each tick
**Unscheduled; minor — no longer dominant.**
**Found 2026-07-26.** `leading_player` runs a full `route_between_cells` per
active player every tick inside `guardian.step`, and `recovery_destination` runs
one per blueprint when a catch resolves. Same redundant-pathfinding pattern as the
fixes below, and both are candidates for `route_within_cost` bounding or for
hoisting the shared spawn→exit term. Left alone because after the fixes below they
no longer show up in the profile — pick this up only if the sim step regresses.

## Minor / hygiene

**Scheduled: Arc H Phase 61 (as-landed notes).**

- Phase 54/55 docs never got "As landed" notes (56 has one; 56 also has a
  "Review fixes" section).

## Fixed

- ~~FPS collapses to 9 in some hex regions (a fixed-timestep catch-up spiral)~~ —
  fixed 2026-07-26. Reported as "FPS drops hard in certain lighting districts";
  the district correlation was incidental. The fixed timestep is 60 Hz (16.67 ms,
  `game/src/screens.rs`), but one simulation step cost 13.1 ms in some regions
  versus 2.0 ms elsewhere. The frame then overran, `RunFixedMainLoop` ran catch-up
  steps (7 observed), each costing another 13.1 ms — stopping only at Bevy's
  `DEFAULT_MAX_DELTA` of 250 ms, which is exactly the `250000` µs maximum frame
  time present in every `timings.json`, including ones predating the
  investigation. **97 % of the expensive step was `guardian.step`**, and inside it
  `player_sees_guardian` ran a full uncapped A* to ask "is the Guardian within two
  cells?" *before* the O(1) distance and facing checks — for every player, every
  tick — which exhausted the whole connected component whenever the Guardian was
  far or unreachable. Fixes, all semantics-preserving: reorder those guards (the
  conjuncts are pure, so only evaluation order changes); add
  `bot_behaviour_and_route` so bots route once per tick instead of twice; add
  `HexWfcWorld::route_within_cost` and bound the three per-frame callers whose
  answers saturate (`pressure_for` at 12 000, `player_sees_guardian` at
  `MAX_CONNECTION_COST`, `lantern_proximity` at its baseline); cache the
  `lantern_proximity` spawn→exit baseline as `spawn_to_exit_cost`, refreshed only
  in `commit_mutation`. Result: the cell that opened the investigation went
  **9 fps → 80 fps**, both seeds now 10.3–10.7 ms median with one fixed step per
  frame, `lantern::sync_dynamic` 11.4 ms → 1.16 ms. Bound exactness and cache
  freshness are both covered by tests
  (`bounded_routing_agrees_with_unbounded_inside_the_bound`,
  `cached_spawn_to_exit_cost_survives_a_committed_relayout`,
  `scoped_trim_matches_the_owned_subset_of_the_full_derivation`). Full method and
  the hypotheses that were measured and *rejected* along the way (key-light
  shadows, light clustering, draw calls, streaming churn):
  [evidence/arc_m/district_perf/FINDINGS.md](evidence/arc_m/district_perf/FINDINGS.md).

- ~~Shadow Screen and Institutional key lights emit nothing and cost a shadow map~~
  — fixed 2026-07-26. Both registers set `key_shadows_enabled` with real intensity
  and range but inherited `key_inner_angle`/`key_outer_angle` of `0.0` from the
  keyless `Hollow` district. Bevy builds the shadow projection as
  `perspective_infinite_reverse_rh(outer_angle * 2.0, ..)`, so a zero outer angle
  puts `1/tan(0)` = inf into the matrix — and a zero-width cone emits no light at
  all. Both were paying for a shadow map that lit nothing. Given real cone angles;
  the palette invariant test now asserts `0 < inner <= outer < PI/2` whenever the
  key casts, which is the check that would have caught it.

- ~~`sync_streamed_cells` marks every cell dirty every frame~~ — fixed 2026-07-26.
  It assigned `*visibility` unconditionally; `Mut` deref marks
  `Changed<Visibility>`, which is exactly the filter Bevy's
  `visibility_propagate_system` runs on, so the whole ~5 600-cell hierarchy
  re-propagated to ~109 k children every frame to reach a state it was already in.
  Now guarded, matching the pattern `sync_practical_shadow_budget` already used.

- ~~Control rebind captures its own activation key~~ — fixed by Arc H Phase 63
  (`control_lab` overlay machinery adopted; capture arms on the activation key's
  release, conflicts surface visibly). Overlay evidence captured and viewed
  2026-07-11 (`docs/evidence/phase_63_rebind_overlay.png`); hand-audited in the
  ship-gate playtest.

- ~~Stretched textures and bad "ceiling tile" geometry~~ — fixed by Arc H
  Phase 62 (palette-over-albedo, world-unit UVs, triangulated ceiling geometry
  removed, style-presence audit check added). Hand-audited in the ship-gate
  playtest 2026-07-11. Note: the phase's own captures render nearly black
  (see open #6), so the in-game look is the verified artifact, not the PNGs.

- ~~Hallway thresholds overlap or land in corners~~ — fixed by Arc H Phase 64
  (audit-first reproduction, generator/projection agreement fixed at the source,
  permanent map-validation gate). Threshold renders captured and viewed
  2026-07-11 (`docs/evidence/phase_64_threshold_integrity/`); hand-audited in
  the ship-gate playtest.

- ~~Unknown `OBSERVED2_BOTS` tokens panic and the all-on digest is unpinned~~ —
  fixed 2026-07-11 in the Phase-66 implementation. Unknown tokens warn and use
  the all-on default; the default director corpus digest is pinned at
  `0x539C35C626B9B30F`.

- ~~Observation rooms don't show camera views~~ — fixed 2026-07-11 by Arc H
  Phase 65. Both rooms now show live 3×3 schematic feeds from simulation data;
  anchor/guardian signals remain cyan/red, panel geometry is flush, and the
  feeds never mutate `MapKnowledge`. See
  [phase_65_observation_rooms.md](arc_h/phase_65_observation_rooms.md).

- ~~District ambience wash / location beds never playing~~ — fixed 2026-07-09
  (see `docs/arc_f/phase_56_audio_content_spatial.md` "Review fixes": bed sinks
  excluded from the director's blanket write; corridor/gantry beds wired).
