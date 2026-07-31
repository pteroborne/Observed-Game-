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

Entries 13–17 came out of the 2026-07-26 Arc O planning survey and are structural
rather than observational: each one was found by reading the solver, the catalog,
and the projector rather than by playing. They are all scheduled into Arc O
([arc_o/README.md](arc_o/README.md)) because the arc cannot deliver legible
districts while any of them stands.

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

### 18. A two-level room's internal vertical link is not traversable
**Scheduled: Arc O Phase 111** ([arc_o/README.md](arc_o/README.md)).
**Found 2026-07-27 while reproducing the Phase 107 soak failure.** A room
blueprint that spans two levels (`GuardianControl`) exposes `ShaftOpen` between
its stacked cells, and nothing can climb it. The bot's stair handling is gated on
`HexArchetype::Shaft` (`crates/observed_match/src/hex_wfc/model/bot.rs`), and the
room's geometry comes from `blueprint_cell_archetype`, which returns `sanctuary`
for every role and cell (#15), so there are no treads inside it either.

**Updated 2026-07-27 (Phases 109 and 111).** Both prerequisites are now done and
this is a geometry task, not a wiring one. The bot follows whatever climb a tile
declares — `vertical_command` gates on the *port class*, not the archetype, so an
atrium shipping a `StairSpine` would simply be climbed — and the atrium now has
geometry of its own (`room_atrium_lower` / `room_atrium_upper`) to hang a stair
on rather than two copies of a single-hex room.

What remains is the stair, and it is the size of Phase 109's tower work. The
lower cell declares doors on all six faces and is one level tall, so a flight
cannot run along a wall without blocking a door, and cannot reach the upper
cell's gallery without the tile spanning two levels. Either the blueprint gives
up a door and the tile gains a level, or the room gives up its vertical port. The
Phase 107 guard in `topology::is_connection_open` stays until then: it is
correct, and it costs nothing but a connection that does not exist.
Phase 107 stopped the router promising the climb —
`topology::is_connection_open` no longer treats a room-to-room vertical port as a
connection — which is a correct guard but not a fix. When #15 gives rooms real
per-role geometry, a two-level room should get real stairs and this guard should
be lifted, or the blueprint should stop claiming a vertical port it cannot serve.

### 20. The played hex facility is murky, samey, and claustrophobic

**Scheduled: Arc P Phases 114 and 118** ([arc_p/README.md](arc_p/README.md)).
**Found 2026-07-27 during the incomplete Phase 113 run-through.** The semantic
lighting technically arrives, but already-dark structural materials were locally
dimmed, room fog retained corridor-scale bounds, and the fixed first-person FOV
amplified the sense of enclosure. The first correction overcompensated by
removing the shell attenuation while lifting global ambient; the 2026-07-28
rebalance restores black levels and makes release come from longer sightlines and
continuous local practicals. The implementation uses one composition-aware style
mapping and exposes a saved 50°–80° FOV setting. It remains open until the human
Phase 118 gate judges the live result.

### 21. `Expanse` cells do not reliably compose into wide-open places

**Scheduled: Arc P Phases 115 and 118** ([arc_p/README.md](arc_p/README.md)).
**Found 2026-07-27 during the incomplete Phase 113 run-through.** Arc O proved an
archetype share, not an open-space experience: isolated wall-free cells could
satisfy the count while the facility still played as uniformly narrow routing.
Production collapse now reinforces adjacent expanses and rejects any solve where
an active level lacks a connected seven-cell, three-exit open volume. It also
rejects any walkable region more than 24 graph edges from an open/decision beat.
The deterministic gate is landed; the perceptual judgment remains with Phase 118.

### 22. The canonical hex race lacks deliberate intermediate choices

**Scheduled: Arc P Phases 116–118** ([arc_p/README.md](arc_p/README.md)).
**Found 2026-07-27 during the incomplete Phase 113 run-through.** The facility
contained semantic room roles but the played match still reduced to following a
route to the exit. Production now repeats decision roles and exposes typed
authored mechanism sockets. Teams claim two contested keystones, synchronize two
operators at one station, and regroup at the exit; monitors are optional local
surveys and anchors keep their observation-freeze role. Bots, LAN authority,
snapshots, cues, map knowledge, and world labels use the same model. It remains
open until Phase 118 establishes that the loop feels like choosing rather than
running errands.

### 23. Arc P objective routing collapses production FPS

**Fix landed 2026-07-28; awaiting human verification in Phase 118.** The first
Arc P playthrough fell into the fixed-timestep catch-up ceiling: the profiler
measured a 250 ms median frame and 32.8 ms median fixed step. Objective selection
ran exact A* once per candidate, per bot, per tick; after that was removed,
Guardian leader selection still routed once per runner per tick and lantern
feedback routed once per lantern per rendered frame. The corrected production
capture measures 8.55 ms median / 10.40 ms p95 frames uncapped and 0.10 ms median
fixed steps. Bot paths are derived caches invalidated by objective or facility
generation; Guardian target ranking uses travel distance; lantern route signals
cache by player cell, Guardian cell, inventory, and facility generation.

### 24. Arc P room lighting flattens the neon-noir contrast

**Fix landed 2026-07-28; awaiting human verification in Phase 118.** The first
pass simultaneously removed the hex shell's contrast attenuation and raised
dark-register room ambient from 40 to at least 110. That made the wider spaces
read as globally exposed haze rather than light contained inside darkness. The
shell attenuation is restored, room/vertical ambient lifts are bounded to +10
and +5 respectively, and the intended release now comes from longer fog
sightlines and continuous local practical pools. The production evidence was
recaptured after the correction; the human gate decides whether the balance is
now right.

### 25. Multi-hex rooms read as clusters of tiles instead of cohesive rooms

**Fix landed 2026-07-28; awaiting human verification in Phase 118.** The Phase
118 playtest found that room footprints still advertised the cell lattice: the
solver declared sibling faces `Sealed` while declaring every unnamed perimeter
face a `Door`. Whole-room geometry visually omitted the internal walls, but the
topology could not traverse those seams and the outer shell became a near-solid
run of anonymous doorframes. The contract is now the intended inverse: sibling
faces are open, only named exterior ports receive framed thresholds, and every
other perimeter face is a solid wall. Solver compatibility, route validation,
whole-room modules, and the per-cell fallback kit share that contract. Start and
Teleport Relay also have whole-room modules, so every blueprint role can take
the cohesive projection path.

The immediate human recheck still read roughly the same. Phase 118 therefore
keeps #25 open and adds a production-corpus mode to `hex_wfc_lab`: the previous
3D lab silently discarded whole-room prototypes and could not diagnose the
played result faithfully. The new atlas uses production quotas and the runtime
catalogue, shows the full solved room/hall lattice at low cost, streams exact
authored hulls around a free-fly camera, and indexes every active production
room/hall concept for direct comparison.

### 26. Arc Q shipped with a failing suite and a stopped simulation in test

**Fixed 2026-07-30 during the Phase 123 gate.** The Arc P/Q implementation was
never run against its own gates. Five `observed_game` tests failed, and two of
them were hiding real regressions rather than stale expectations: `test_app`
read the developer's own preferences through `load_settings`, so the Arc Q
onboarding gate set `SimulationPolicy::Stop` and every interactive fixture
advanced zero ticks — which is what the interactive determinism gate was
actually reporting when it claimed a digest divergence. Separately the headless
fork of that gate still drove bots with the pre-Arc-P `bot_command` instead of
the objective-aware `bot_player_command` production uses. Fixtures are now
hermetic in settings as well as career, and the fork mirrors `step_runtime`.
Three labs (`lan_lab`, `hex_room_lab`, `asset_lab`) also no longer compiled or
passed against changes made in Arcs O and P.

### 27. Every non-ASCII glyph in the UI renders as a blank box

**Fixed 2026-07-30 during the Phase 123 gate.** The game ships no font asset, so
labels draw with Bevy's embedded default subset, which has no geometric shapes,
dashes, bullets, or degree sign. The Play hub marked the selected preset with
`◆`/`◇`, so at 1280×800 **no preset appeared selected at all** — the one thing a
preset-first hub has to communicate. The Lobby seat legend, the field-of-view
row, and the Results roster subtitle were tofu for the same reason. Rendered
strings are now ASCII, and
`arch_check::rendered_ui_strings_stay_within_the_shipped_font` fails the build on
any non-ASCII character in a non-test string literal. Reopen this as a real
typography task only by shipping a font asset that covers the glyphs wanted.
([arc_q/phase_123_human_ux_gate.md](arc_q/phase_123_human_ux_gate.md))

### 28. The pause overlay renders nowhere in the hex match

**Found in live play 2026-07-31, fixed same day.** Escape paused the match and no
menu appeared. The overlay was spawning correctly; it had no visible camera to
spawn onto. `GameCam` claimed no `IsDefaultUiCamera`, so Bevy assigned every UI
root to the highest-order camera on the primary window — a choice that ignores
`is_active` — and the survivor map's dormant order-1 camera took the pause
overlay, the HUD, and the cue banner with it. The world camera now claims the UI
explicitly and the map legend names the map camera with `UiTargetCamera`.

The whole class was invisible to the suite, which asserts that overlay entities
exist rather than where they render;
`in_match_ui_targets_the_active_world_camera_not_the_dormant_map_camera` now
asserts the target and fails without the fix. Restoring UI visibility exposed a
second defect immediately: the cue banner's opaque plate stayed drawn when its
text was cleared, parking a black bar on the sightline, so it now carries its own
`Visibility`. ([arc_q/phase_123_human_ux_gate.md](arc_q/phase_123_human_ux_gate.md))

## Minor / hygiene

**Scheduled: Arc H Phase 61 (as-landed notes).**

- Phase 54/55 docs never got "As landed" notes (56 has one; 56 also has a
  "Review fixes" section).

## Fixed

- ~~Every room cell of every room role asks for the same archetype~~ — fixed
  2026-07-27 in Arc O Phase 111
  ([arc_o/phase_111_rooms_belong_to_districts.md](arc_o/phase_111_rooms_belong_to_districts.md)).
  `blueprint_cell_archetype` discarded both arguments and answered `"sanctuary"`,
  so a three-hex Decision room was one single-hex shape repeated three times. The
  fix authored nothing: `room_double_*`, `room_tri_*`, `room_fork_*` and the
  atrium pair have been generated in every register since Arc L, their names map
  one-to-one onto the blueprint footprints, and `compatibility_archetype`
  flattened every `room_*` name back to `sanctuary` on the way in. Two stubs
  facing each other across a seam, cancelling a whole family of geometry. Phase
  110's coverage gate checked the fourteen pairings — a wing paired to the wrong
  cell produces a signature nothing satisfies.
- ~~Authored content is unreachable from the solver~~ — fixed 2026-07-27 in Arc O
  Phase 111
  ([arc_o/phase_111_rooms_belong_to_districts.md](arc_o/phase_111_rooms_belong_to_districts.md)).
  `DecoherenceFork` — four hexes, the largest authored module in the corpus — was
  absent from the stamping pool, and adding it was not enough. `span` was
  `max_rooms - min_rooms`, which for the production 9..=10 contract is 1, so
  `rng % span` was always 0: the target was always 9, `max_rooms` had never once
  been reached, and the pool's last slot was unreachable. With the range made
  inclusive the fork appears in 5 of 12 seeds, in exactly its two districts.
  `TeleportRelay` was decided rather than deferred: **not wired**, because the
  hex match has no teleport-pad mechanic — `sync_teleports_to_bodies` reconciles
  spawn, setback and escape moves only — so stamping one would spend a room slot
  on a room that does nothing. Its blueprint stays for the deprecated `full_wfc`
  path, which requires a pair.
- ~~Liminal Grid has no authored `expanse` tiles~~ — fixed 2026-07-27 in Arc O
  Phase 110
  ([arc_o/phase_110_district_tilesets.md](arc_o/phase_110_district_tilesets.md)).
  Liminal Grid had been left out of the generated kit's register table for two
  arcs, on the reasoning that the one district with hand-authored modules needs
  no generated floor under it. That held only while the authored corpus covered
  every demand the solver could make, and `Expanse` ended it. It is now in
  `REGISTERS` with a style of its own, the generated kit sits *under* the
  authored modules rather than replacing them, and a test pins that both
  authored Liminal layouts are still reachable. The `expanse` exemption is gone,
  and so is the `stair_tower` one beside it.
- ~~Every vertical cell in the facility is the same procedural stair~~ — fixed
  2026-07-27 in Arc O Phase 109
  ([arc_o/phase_109_authored_stair_towers.md](arc_o/phase_109_authored_stair_towers.md)).
  Measured at 46.5–47.5 % of every placed cell at the Phase 104 baseline, all of
  it one procedural switchback reached through the `generic` fallback. Now
  **17.7 %**, and the vertical districts build handed towers of their own that
  turn the other way. The blocker was never authoring: the objective bot climbed
  by constants measured off that one tower, so a second shape would have been
  unwalkable and even fixing the first one broke the steering. Tiles now declare
  their own climb and floor path and the bot follows those.
  The `stair_tower` coverage exemption it left behind came down in Phase 110,
  when every register gained an exact kit of its own. Two tower shapes remains a
  handed pair rather than two designs; a genuinely new skeleton is unscheduled
  authoring work.
- ~~Bots can stall exiting a switchback tower laterally~~ — fixed 2026-07-27 in
  Arc O Phase 109
  ([arc_o/phase_109_authored_stair_towers.md](arc_o/phase_109_authored_stair_towers.md)).
  Both directions were broken, not only the exit: a bot *entering* a tower
  laterally was steered straight at the flight and pinned against the guard rail
  around the stairwell, and one leaving was steered across the void. The exit
  waypoints were hardcoded to the generic switchback's geometry, so authored
  towers with different interiors would have been unwalkable by construction.
  Fixed by having each tile declare its own walkable floor (`DeckPath`) beside
  its climb (`StairSpine`), and following those. Sixty lines of per-face local
  coordinates and a rectangle-crossing test are gone from `bot.rs`; nothing in
  the bot now knows what a tower looks like inside.
- ~~Composition tendencies are compiled off after breaking the bot soak~~ —
  fixed 2026-07-27 in Arc O Phase 107
  ([arc_o/phase_107_district_composition.md](arc_o/phase_107_district_composition.md)).
  The flag is on and districts now carry per-archetype weight profiles. The
  failure that disabled it was never the weighting: the tendency shifted exactly
  one of the 28 routable soak layouts into routing through a two-level room's
  internal vertical link, which nothing can climb (now #18). Measured, the old
  weighting produced 0 such routes and the tendency produced 1 — the stalling
  seed. Guarding the router cleared the soak. Facility-wide shaft share fell from
  47 % to 31 % as a side effect, and districts now differ by up to 3.3x on an
  archetype.
- ~~Architecture registers are assigned as per-cell white noise~~ — fixed
  2026-07-27 in Arc O Phase 106 ([arc_o/phase_106_spatial_districts.md](arc_o/phase_106_spatial_districts.md)).
  `register_for` drew one of nine registers per hex from a SplitMix keyed on the
  coordinate, so nine of the ten districts were spatially incoherent; only
  `LiminalGrid` had a zone. It is now a lookup: one seeded anchor per register
  per level, and a cell belongs to the nearest anchor on its own level, which
  makes a district contiguous by construction. Measured across the five pinned
  seeds, mean region size went from **1.4 cells to 29–86**, and every register
  now resolves to exactly one region per level. The old key also folded the
  relayout generation in, so every commit re-rolled the register of every cell in
  the pocket — that churn reached `cell_revisions` and therefore the snapshot
  digest the LAN wire compares, and is now gone.
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
