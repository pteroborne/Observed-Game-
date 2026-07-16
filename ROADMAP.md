# Observed Game Roadmap

This document outlines the current active development goals, completed milestones, and upcoming phases for the game.

## Current Goals (North Star)
1. **Make a fun game:** Establish tension↔release rhythms between decision-making Rooms (cooperative, puzzle-solving) and hazard-filled Corridors (traversal, risk).
2. **Develop effectively with agents:** Focus on reusable modules, code-as-art neon-noir procedural aesthetics, and clear evidence-gathering pipelines.

---

## Active & Upcoming Phases

**Arc L — Hex Tile Facility** `[active]` (plan: [docs/hex_tile_arc_plan.md](docs/hex_tile_arc_plan.md); hand-offs: [docs/arc_l/](docs/arc_l/README.md)). The map-system pivot (user spec, 2026-07-16): replace the square full-WFC lattice with a TrenchBroom-authored hex-prism tile system — taller cells (8 m levels), a larger grid in all three axes (28×20×10), multi-tile room blueprints, halls as long as they need to be, walkable ramp prefabs and true shaft stacks producing tall formations (silo wellshafts, ramp chains). Landed lab-first per the mandated sequence: 2D hex WFC lab with an animated step mode → TrenchBroom tile lab (`observed_authoring` promotion — the A9 adopt-when trigger has fired) → 3D hex facility lab → full game integration. Locked rulings: hex replaces square at integration (square demoted to regression-only); full relayout/decoherence/determinism parity in-arc before the swap; ramps and shafts both first-class.

### Phase 86 — Arc K Closeout & Clean Baseline `[x]`
Commit the outstanding Arc K working tree (bot navigation/fall-recovery follow-ups, the untracked `map_observer_lab`, evidence refresh), split `full_wfc/collapse.rs` under the 600-line ratchet (validation extracted to `validate.rs`), and open the arc docs. The Arc K Phase 85 hands-on playtest gate is **formally carried into Phase 95**, where it is discharged on the hex game.

### Phase 87 — observed_hex Foundation `[ ]`
The shared hex-grid crate: axial `HexCoord`, 8-face `HexFace`, `PortClass`/`PortSignature` port vocabulary, the quantized-hexagon metrics (integer corners, exact TrenchBroom snapping), world mapping, property tests. ([docs/arc_l/phase_87_observed_hex_foundation.md](docs/arc_l/phase_87_observed_hex_foundation.md))

### Phase 88 — 2D Hex WFC Solver + Animated Step Lab `[ ]`
`observed_facility::hex_wfc` at one level (AC-3, min-entropy, SplitMix determinism, forced route, validation) plus `hex_wfc_lab`'s animated step mode showcasing propagation fronts, collapse order, and halls snaking between rooms. ([docs/arc_l/phase_88_hex_wfc_2d_lab.md](docs/arc_l/phase_88_hex_wfc_2d_lab.md))

### Phase 89 — TrenchBroom Tile Pipeline + Tile Lab `[ ]`
`crates/observed_authoring` (promoted from `trenchbroom_lab`/`rapier_authoring_lab`), the tile schema + manifest, the locked authoring template, seed tiles including a full two-level ramp prefab, and `hex_tile_lab` proving first-person ramp ascent. ([docs/arc_l/phase_89_tile_pipeline_lab.md](docs/arc_l/phase_89_tile_pipeline_lab.md))

### Phase 90 — Solver Verticality & Room Blueprints `[ ]`
Levels > 1: shaft stacks, paired ramp variants, multi-hex room blueprint stamping with stable `generation_key` identity, tall-formation validation, per-level slice view in the lab. ([docs/arc_l/phase_90_verticality_blueprints.md](docs/arc_l/phase_90_verticality_blueprints.md))

### Phase 91 — Tile Library Authoring `[ ]`
The real tile set against the frozen schema — hall families per archetype, ramps in all six directions, shaft segments/caps/landings, room blueprint footprints per role — with the manifest coverage validator green. ([docs/arc_l/phase_91_tile_library.md](docs/arc_l/phase_91_tile_library.md))

### Phase 92 — 3D Hex Facility Lab `[ ]`
`observed_match::hex_wfc::geometry`: solved world + manifest → tile prefab placement → `ArenaSpec`; the lab grows to 3D free-fly + FPS walkthrough with the 2D step mode retained; collider budget measured. ([docs/arc_l/phase_92_hex_facility_3d.md](docs/arc_l/phase_92_hex_facility_3d.md))

### Phase 93 — Relayout / Decoherence Parity `[ ]`
Multi-tick relayout on hex with blueprint-granularity pins, stable IDs across relayouts, commit-time route re-validation, replay determinism tests, and an animated relayout demo in the lab. ([docs/arc_l/phase_93_relayout_parity.md](docs/arc_l/phase_93_relayout_parity.md))

### Phase 94 — Match-Layer Parity: Bots, Routing, Thresholds `[ ]`
Ramp/shaft-aware routing costs, bots physically walking ramps and climbing shafts, threshold/door semantics on blueprint ports, and the headless gate: a bot completes spawn→exit across ramp levels and a shaft, deterministically. ([docs/arc_l/phase_94_match_parity.md](docs/arc_l/phase_94_match_parity.md))

### Phase 95 — Game Integration & Square Demotion `[ ]`
`game/src/hex_wfc/` becomes the Play flow with the full visual/audio language; the square lattice is demoted to regression-only; docs and evidence refresh; the arc closes on the hands-on user playtest (absorbing the Arc K Phase 85 gate). ([docs/arc_l/phase_95_game_integration.md](docs/arc_l/phase_95_game_integration.md))

---

**Arc K — Full-WFC Gameplay Lock** `[implementation complete; playtest carried into Arc L Phase 95]` (plan and gate record: [docs/full_wfc_gameplay_arc_plan.md](docs/full_wfc_gameplay_arc_plan.md)). The continuous full-WFC lattice is now the canonical game and owns the full room/hall catalog, four-team co-op race, physical objectives, anchors, team pads, Guardian, survivor tac-map, audiovisual feedback, bots, replay, and results behind a deterministic multiplayer-ready input/snapshot boundary. Its visual-parity pass now carries the proven pre-full-WFC language into the continuous shell: canonical apertures with Kenney frames/cables, full textured walls/floors and treated ceilings, all nine register dressings, fog/bloom/high SSAO/FXAA, bounded volumetrics, shadows, and a deterministic local-light budget. The legacy teleport-place match is regression-only. Phases 78–84 are complete; Phase 85's implementation landed and its hands-on playtest gate is formally deferred to Arc L Phase 95, where it is discharged on the hex facility (ruling 2026-07-16).

---

**Arc E — Ready to Race** (plan and design rules: [docs/ready_to_race_arc_plan.md](docs/ready_to_race_arc_plan.md)). The polish-and-presence arc: make the proven race a finished game a real person wants to play — onboarding, settings, audio, game-feel, and HUD clarity. **Scope ruling 2026-07-09:** the arc ends when Phase 50 lands; the LAN multiplayer phases (51–53) are deferred to the Post-MVP Backlog below (absorbed into "true LAN multiplayer with dedicated servers" — their design docs in `docs/arc_e/` are retained as design input).

### Phase 48 — Onboarding & Settings `[x]`
Close the biggest QoL gap for a real player: first-run teaching of the core loop (observe-to-freeze, anchors, the tac-map, the gantry, the collapse) surfaced through the existing legend/discovery/hint systems; a real settings screen (volume, mouse sensitivity, key rebinding via the proven `control_lab` overlays and the `player_input` abstraction, plus `observed_style` accessibility toggles), persisted with the career profile; and context control hints.

### Phase 49 — Audio & Game Feel `[x]`
Deepen the audio (spatial/attenuated cues, per-district ambient beds, collapse/klaxon/escape stings, UI sounds — all drop-in) and add movement/camera juice (teleport transitions, crossing feedback, gantry jump/land feel, restrained collapse camera). Every effect verified against the Legibility Contract — atmosphere never hides a signal — with refreshed evidence GIFs.

### Phase 50 — HUD & Results Clarity `[x]`
Immersion-first presentation (redirected 2026-07-09, see [docs/arc_e/phase_50_hud_results_clarity.md](docs/arc_e/phase_50_hud_results_clarity.md)): normal play is HUD-free — the status readouts and legend are debug-only (`OBSERVED2_DEBUG_HUD`) — and the Tab tac-map is a fog-of-war survivor's sketch that draws only rooms/connections the player has personally witnessed (`MapKnowledge` ledger): glimpsed rooms as hollow outlines, the exit unmarked until found, witnessed edges silently dropping off when a reroute removes them. The Phase-66 implementation supplied the post-match story and direct rematch path; closed 2026-07-11 with Phase 66's ship gate.

### Phases 51–53 — LAN Multiplayer `[deferred to Post-MVP]`
Shared Actions (`LocalAction::PlaceAnchor`), Real Transport (loopback → LAN), and the LAN Lobby were planned as Arc E's back half and are deferred whole to the Post-MVP Backlog (2026-07-09 ruling). The designs stay valid — first-person actions entering the lockstep stream, a socket adapter behind `observed_net`, lobby/discovery over `observed_progression` — and live in [docs/arc_e/phase_51_shared_actions.md](docs/arc_e/phase_51_shared_actions.md), [phase_52_real_transport.md](docs/arc_e/phase_52_real_transport.md), and [phase_53_lan_lobby.md](docs/arc_e/phase_53_lan_lobby.md).

---

**Arc F — Sight & Sound** `[all phases complete 2026-07-10 — landing debt absorbed into Arc H]` (plan: [docs/sight_and_sound_arc_plan.md](docs/sight_and_sound_arc_plan.md); hand-offs: [docs/arc_f/](docs/arc_f/README.md)). All seven phases landed: True-Singleplayer toggles, the AudioDirector + fully procedural audio palette (`tools/generate_audio.py` — composed liminal ambience beds, distinct per-signal cue families), and the sprite layer (pipeline+lab → objects → directional actors → dressing). The 2026-07-10 review confirmed the code claims (189 green suites, clippy clean) but found the imported textures visually overpowering the neon-noir style layer, two evidence captures missing/empty, five phase docs without as-landed notes, and the entire arc uncommitted — all scheduled as Arc H's opening work.

### Phase 54 — True Singleplayer (bot & guardian toggles) `[x]`
Menu/settings toggles that disable rival teams, own AI teammates, and the guardian separately — for a "True Singleplayer" facility used both for gameplay and clean testing. Implemented as deterministic match configuration (disabled populations never spawn in the sim; headless == interactive), following the `OBSERVED2_MAP` precedent with an `OBSERVED2_BOTS` override, persisted with the career profile; all-on default pinned byte-identical, solo matches end and report sanely. ([docs/arc_f/phase_54_true_singleplayer.md](docs/arc_f/phase_54_true_singleplayer.md))

### Phase 55 — Audio Architecture (the mixer) `[x]`
One `AudioDirector` owns every cue decision through a single data-driven cue table: bus model (master/music/sfx/ui over the Phase-48 settings channels), sting-ducks-ambience easing (volume-only, never stream restarts), and structural per-class cooldowns/instance caps so the next event-spam bug is inaudible by construction. Every spawn site migrates to director requests; no audible identity changes. ([docs/arc_f/phase_55_audio_architecture.md](docs/arc_f/phase_55_audio_architecture.md))

### Phase 56 — Audio Content & Spatial Depth `[x]`
An audited cue set on top of the director: a test-enforced coverage table mapping every gameplay-critical signal to a cue (or an explicit silence ruling), distance-rolloff and through-wall occlusion classes, richer district beds, and the CC-BY reference libraries replaced with CC0 and removed — closing the license caveat for good. ([docs/arc_f/phase_56_audio_content_spatial.md](docs/arc_f/phase_56_audio_content_spatial.md))

### Phase 57 — Sprite Metadata Pipeline & the OGA Lab `[x]`
The raw OpenGameArt intake becomes a metadata-driven pipeline: checked-in frame metadata (rects, pivots, ppm, directional counts, semantic clips), a deterministic derive script into `assets/oga_25d/derived/`, `TextureAtlasLayout` support, and an `oga_25d_lab` (grown from `sprite3d_placeholder_lab`) proving actors, objects, decorations, and direction/clip debugging — game untouched. ([docs/arc_f/phase_57_sprite_pipeline_lab.md](docs/arc_f/phase_57_sprite_pipeline_lab.md))

### Phase 58 — Gameplay Object Sprites `[x]`
OGA objects enter the played game first: eight semantic object slots (keystone card/core, exit access card, anchor torch, route cell, relay device, battery, repair token) driven by existing match state, with `observed_style` halos layered over the art, clean despawn on reset/exit, and procedural fallbacks intact. ([docs/arc_f/phase_58_gameplay_object_sprites.md](docs/arc_f/phase_58_gameplay_object_sprites.md))

### Phase 59 — Directional Actors `[x]`
Rivals and the guardian move to directional sheets: direction from camera-relative angle, clip from existing presentation state (combat frames remapped to operate/alert/disrupted/exit or dropped), frame selection a tested pure function, guardian light/halo signals independent of the art, and all three fallback rungs (sheet → single-frame → capsule) working. ([docs/arc_f/phase_59_directional_actors.md](docs/arc_f/phase_59_directional_actors.md))

### Phase 60 — Room Dressing, Textures & Interaction Read `[x]`
The atmosphere payoff: seeded, role-driven props under three hard rules (never cover a threshold, never steal a signal color, always dimmer than interactables), optional LAB albedo variants under style tint, and a documented ruling on the interaction read (diegetic cue preferred over any crosshair — normal play stays HUD-free). Full visual-audit and evidence refresh closes the arc. ([docs/arc_f/phase_60_dressing_textures_reticle.md](docs/arc_f/phase_60_dressing_textures_reticle.md))

---

**Arc H — Ground Truth** (plan: [docs/ground_truth_arc_plan.md](docs/ground_truth_arc_plan.md); per-phase sub-agent hand-offs: [docs/arc_h/](docs/arc_h/README.md)). The harden-and-ship arc: make the game match its claims — visually, mechanically, and in the ledger — then declare the MVP shipped. No new features; the bug backlog and Arc F's landing debt are the scope. New standing rule born from the Arc F review: **every phase ends with evidence a human can falsify** — agents view their own captures, the parent session rejects phases whose evidence doesn't visibly show the claim. Waves: [61] → [62 ∥ 63 ∥ 64] → [65 ∥ 66].

### Phase 61 — Land Arc F (commits, ledger, honest evidence) `[x]`
Stage the entire uncommitted Arc F working tree into reviewed commits; tick the ledger (ROADMAP milestones, the five missing as-landed notes incl. the Phase-60 reticle ruling, memory, Catalogue); re-capture the evidence that is missing or shows nothing (rivals with rivals actually in frame, dressing before/after). Bookkeeping only — no behavior changes. ([docs/arc_h/phase_61_land_arc_f.md](docs/arc_h/phase_61_land_arc_f.md))

### Phase 62 — Style Reconciliation `[x]`
Textures back under the Contract: district palette tint modulates every albedo (two districts unmistakable in a capture; drained/klaxon still read), world-unit UVs end the smearing, the triangulated ceiling-tile geometry is removed, and the visual audit gains a style-presence check proven to fail against the old broken state. Closes bug backlog #2. ([docs/arc_h/phase_62_style_reconciliation.md](docs/arc_h/phase_62_style_reconciliation.md))

### Phase 63 — Control Rebind, For Real `[x]`
Replace the custom rebind capture with the proven `control_lab` overlay machinery (user ruling); the capture arms on the activation key's release so binding-the-activation-key is structurally impossible; conflicts surface visibly; round-trip and gamepad-regression tests. Closes bug backlog #1. ([docs/arc_h/phase_63_control_rebind.md](docs/arc_h/phase_63_control_rebind.md))

### Phase 64 — Threshold Geometry Integrity `[x]`
Write the audit check first (no threshold may intersect an interior wall or another threshold, corpus-wide, DFS and WFC, all decoherence versions), use its failures as the reproduction set, fix the generator/projection disagreement at the source, and keep the check as a permanent map-validation gate. Closes bug backlog #3. ([docs/arc_h/phase_64_threshold_integrity.md](docs/arc_h/phase_64_threshold_integrity.md))

### Phase 65 — Observation Rooms Made Real `[x]`
The 3×3 observation panels become legible schematic room feeds rendered from simulation data (footprint, doorways, occupant dots; anchors cyan, guardian red — the existing state signals layered on top), diegetic-only (panels never write into the player's fog-of-war `MapKnowledge`), with the jutting geometry fixed. Closes bug backlog #4. ([docs/arc_h/phase_65_observation_rooms.md](docs/arc_h/phase_65_observation_rooms.md))

### Phase 66 — Post-Match Summary & the Ship Gate `[x]`
The results screen tells the run's story (placement, escapes in order, what the collapse took, the player's own path) for every outcome shape including solo, with a one-keypress path back into the loop — closing Arc E Phase 50 — plus the backlog hygiene items (`OBSERVED2_BOTS` panic→warning, the all-on digest characterization test). **Complete 2026-07-11: implementation, four-outcome results evidence, the scripted cross-phase evidence refresh (all captures viewed), and the user playtest all passed — the MVP is shipped.** Playtest findings recorded as backlog #5–#7 (bot-POV stall, dark world captures, audio mix balance). ([docs/arc_h/phase_66_summary_ship_gate.md](docs/arc_h/phase_66_summary_ship_gate.md))

---

**Arc I — Light & Line** (plan: [docs/light_and_line_arc_plan.md](docs/light_and_line_arc_plan.md); hand-offs: [docs/arc_i/](docs/arc_i/README.md)). The atmosphere-and-legibility arc, planned 2026-07-11 with the user, same day the MVP shipped. Thesis: **atmosphere and legibility are the same problem** — the game currently renders zero shadows, and one shadow-casting light per place buys both mood and readability at once (cheap under the teleport model: light budget is per-place). Nine user-chosen media references (Japanese architecture, Brutalism, Backrooms, Severance, Halo CE, BLAME!, Silo, Library of Babel, Rudon's Plane) each isolate one liminal element and become one lab diorama; districts then take identities as coordinates on the resulting axes (brightness, scale, repetition, directionality). All Arc H standing rules carry over. Waves: [67 ∥ 68] → [69] → [70 ∥ 71] → [72].

### Phase 67 — Hygiene Opener: Audio Mix & Bot-POV Stall `[ ]`
Rebalance the gain staging (effects down, ambience beds up — backlog #7, user listen-through finding) with a pinned mix-relationship test, and fix the bot-POV walkthrough freezing in Room 11 (backlog #5) so the evidence GIF reaches the exit. ([docs/arc_i/phase_67_audio_mix_bot_stall.md](docs/arc_i/phase_67_audio_mix_bot_stall.md))

### Phase 68 — lighting_lab: Nine Liminal Dioramas `[ ]`
A new lab staging the nine reference registers as static dioramas with freestanding geometry (user ruling), keys 1–9, one viewed capture per scene, the signal kit staged in every scene, the volumetrics × bloom matrix, slat shadow tuning, per-scene frame times, and the luminance-corridor check prototyped (fails on the archived near-black Phase-62 capture and on an all-white fixture). ([docs/arc_i/phase_68_lighting_lab.md](docs/arc_i/phase_68_lighting_lab.md))

### Phase 69 — Light Staging in the Game + the Luminance Gate `[ ]`
One shadow-casting key light per place, per-district key/fill temperature pairs as style data, pools-rhythm fixture spacing, and the luminance corridor joining the visual audit as a permanent gate; the Phase-62 evidence set re-captured legibly. Closes backlog #6. ([docs/arc_i/phase_69_light_staging_gate.md](docs/arc_i/phase_69_light_staging_gate.md))

### Phase 70 — District Light Identities `[ ]`
Each district takes a register coordinate — exactly one district is the **overlit** register (user-endorsed) — two districts unmistakable in captures by light alone, drained/klaxon reading under every identity, all identity parameters as `observed_style` data. ([docs/arc_i/phase_70_district_light_identities.md](docs/arc_i/phase_70_district_light_identities.md))

### Phase 71 — WFC Register Composition `[ ]`
**Fast-track ruling 2026-07-11 (user):** all nine registers enter the game with WFC as the composition mechanism — geometry grammar and light staging merge into one module layer (slat screens, seam strips, panel grids, practicals, shelf runs, bare decay, void edges) the hallway WFC solves with adjacency rules, weighted by district identity; includes the lab reference-scene rework (monolith shaft, forerunner volumetrics, shoji simplification). Within the prop rules and the Phase-64 threshold gate, deterministic from seed, sim/nav untouched. ([docs/arc_i/phase_71_district_geometry_language.md](docs/arc_i/phase_71_district_geometry_language.md))

### Phase 72 — Arc Gate: Evidence Refresh & Playtest `[ ]`
The Arc H ship-gate pattern rerun: full evidence refresh from one build (all viewed, luminance corridor green), then the user playtest — liminal feel per district, overlit-as-intent, signal legibility in darkest/brightest, the mix verdict, comfort check. The arc closes when the checklist passes. ([docs/arc_i/phase_72_arc_gate.md](docs/arc_i/phase_72_arc_gate.md))

---

**Arc J — Solid Connections (Rapier3D Integration).** The traversal-ground-truth arc: finish moving the played game's room and corridor collision onto deterministic raw `rapier3d`, then make thresholds explicit enough that rendered apertures, physical apertures, and graph connections cannot disagree. A corridor is a stable place with authored threshold sockets, not an implicit `(from, to)` room pair; every live connection is one reciprocal room-socket↔corridor-socket attachment. This is the rule that prevents a threshold from looking walled-off while remaining traversable, and it unlocks real multi-exit Gantry and Wellshaft routes without overlapping room/corridor geometry. Presentation continues to read simulation-owned topology, persistent identity uses domain IDs rather than Bevy `Entity`, and rendered structure and Rapier structure are derived from the same place geometry. Waves: [73] → [74] → [75] → [76].

### Phase 73 — Deterministic Rapier Foundation `[x]`
Prove `rapier3d 0.34` fixed-step lockstep with `enhanced-determinism`, promote the raw-Rapier kinematic capsule into `observed_traversal`, and integrate the controller with the played game's generated room and hallway structural geometry. Keep the resettable determinism/controller labs as focused baselines; expose the exact structural collider list for debugging; establish stable `CorridorId`, `PlaceId`, and `ThresholdId`, reciprocal `JunctionTopology`, and data-driven `MapCorridor` groundwork. **Complete 2026-07-12:** determinism and replay proofs, room/hall traversal, crossing integration, threshold-preview correction, and the stable junction model landed in `8cd5f63`.

### Phase 74 — First-Class Corridor & Threshold Runtime `[x]`
Replace the live pair-shaped `Place::Hallway { from, to, ... }` contract with stable corridor identity plus threshold-socket crossing. Resolve every transition through `JunctionTopology::partner`; validate that each socket has at most one reciprocal attachment; support corridors with two or more exits; and make reroutes update attachments atomically, never one side of a connection. Door/threshold render gaps, occlusion, crossing volumes, and Rapier apertures must consume that same active socket set, with regression fixtures for the formerly visible-but-traversable “wall in front of a threshold” failure. **Complete 2026-07-13:** the two-vocabulary fusion (canonical `CorridorId`/`ThresholdSlotId` unified with the teleport model), `place_junction` + partner-resolved `apply_crossing`, socket-sourced apertures, and the four regression fixtures landed in `013af3a` ([hand-off](docs/arc_j/phase_74_first_class_corridor_runtime.md)). Deferred to Phase 75: the ~12 logic consumers, the full corpus sweep, and unifying the runtime corridor-id space with `MapSpec::corridors`.

### Phase 75 — Room/Hall Variation Parity & Consumer Migration `[x]`
Run the production Rapier path across the complete procedural corpus: every room role and seeded footprint, plus Chicane, Colonnade, Maze (DFS and WFC interiors), Gantry, and Wellshaft corridors at every supported elevation and entrance. Migrate all remaining pair-assuming consumers—bots, nav, guardian targeting, items/anchors, ambience, observation knowledge, tac-map, previews, diagnostics/evidence, replay, and map validation—to stable place/threshold identities. Add corpus tests proving rendered and collidable structural segments agree, every active aperture can be crossed in both directions, sealed sockets cannot be crossed, reroutes remain solvable, and reset/exit leaves no Rapier bodies or colliders behind. **Complete 2026-07-13:** 75a connectivity spine, 75b tether/consumer migration, and the full corpus verification suite (structural agreement, bidirectional crossing, sealed uncrossable, solvability, and lifecycle tests) landed. Default-map determinism digest unchanged.

### Phase 76 — Multi-Exit Junction Gate `[x]`
**Complete 2026-07-13:**
- Shipped `multi_exit_fixture` map spec and catalog registration containing a 3-exit Gantry elevated pathway and a 3-exit Wellshaft vertical stairs with dynamically generated middle level landing bridges.
- Integrated first-class spec corridor geometry and navigation, letting multi-exit endpoints map directly to their respective corridor sockets.
- Added comprehensive integration test suite `phase76_multi_exit_crossing_integration_test` verifying bidirectional crossing, side-exit routing, and topological validity.
- 0 clippy warnings and all workspace tests passing.

### Phase 77 — Threshold & Architecture Refresh `[x]`
**Complete 2026-07-13:**
- Added one pure aperture planner for off-centre, multiple, and elevated thresholds; visible walls, Rapier perimeter collision, trim, previews, and map validation now consume it, while sealed/locked/collapsed states are explicit closure geometry.
- Corrected the Gantry understory side threshold to live on its actual boundary and made frame indicators report durable anchors only, never transient observation.
- Added a weighted semantic room-template catalog to `MapSpec`; WFC and authored maps store their deterministic architectural kit and production room footprints consume the selected profile.
- Promoted the manifest-governed Kenney CC0 gate and cable bundle into threshold presentation with procedural semantic accents, style-normalized imported materials, and procedural fallbacks.
- Made Rapier the assembled game's sole controller path and composed authored Gantry/Colonnade interiors with generated aperture-safe boundaries; removed the runtime physics and geometry selectors.

---

## Bug Backlog

Playtest defects tracked in [docs/bug_backlog.md](docs/bug_backlog.md). The four
original defects (#1 rebind, #2 textures/ceiling, #3 thresholds, #4 observation
rooms) are fixed and were hand-audited in the 2026-07-11 ship-gate playtest.
Open post-ship findings are all scheduled into Arc I: #5 bot-POV stall and
#7 audio mix → Phase 67, #6 near-black world captures → Phase 69. New findings
land in the backlog first, then get scheduled.

---

## Post-MVP Backlog (listed, not scheduled)

Recorded so the horizon is explicit; none of this is being built yet.

1. **True LAN multiplayer with dedicated servers.** The deferred Arc E designs (Phases 51–53: shared lockstep actions, socket transport behind `observed_net`, LAN lobby/discovery) plus a dedicated-server deployment model — a headless deterministic host that peers connect to, replacing pure peer-to-peer session ownership. Full online play (NAT traversal / relay / matchmaking) remains beyond even that.
2. **World interaction.** Players act on the facility graph itself: explorers "hacking" a room console to connect it to a specified room ID (player-driven rerouting, subject to the solvability invariant and decoherence rules), and fallen/absorbed teams connecting nodes from a top-down view — extending "eliminated teams join the adversary" into an active graph-editing role that keeps every player playing to the end.
3. **Carried follow-ups from Arcs C/D:** ~~a third hall endpoint so the gantry's understory exit reaches a genuinely different neighbour~~ **scheduled into Arc J Phases 74–76**; the decoherence counter-tool (Phase 38's criterion (d) never triggered it) remains unscheduled.
4. ~~District architectural language (user idea, 2026-07-11).~~ **Scheduled into Arc I Phase 71** (geometry grammar per district).
5. ~~Directional light-and-shadow staging (user idea, 2026-07-11).~~ **Scheduled into Arc I Phases 68–70** (lighting_lab → game staging → district identities).

---

## Recent Milestones (Completed)

### Phase 66 — Post-Match Summary & the Ship Gate `[x]` — **MVP SHIPPED**
Completed 2026-07-11:
- Results screen builds a pure seven-line `ResultsStory` (standing, escape order, collapse losses, own path, keystones, anchors, replay identity) for all four outcome shapes including true solo; Enter/A rematches directly with a new seed.
- Hygiene: `OBSERVED2_BOTS` unknown tokens warn and fall back to all-on; director corpus digest pinned.
- Scripted ship-gate evidence regenerated from one build and viewed: full visual audit (zero findings), bot-POV GIF, first-ever Phase-63 rebind-overlay capture, Phase-64 threshold renders, Phase-62 set. Fixed the capture drivers' splash-timer race and documented the capture env requirements (`OBSERVED2_BOTS=all`; Vulkan for world scenes, dx12 for Results).
- User playtest passed all four checklist items (onboarding match, solo match, rebind + hand-audit of backlog #1–#4, audio listen-through).
- Findings recorded as backlog #5 (bot-POV stall), #6 (near-black world captures), #7 (audio mix balance).

### Phase 65 — Observation Rooms Made Real `[x]`
Completed 2026-07-11:
- Replaced the protruding monitor dioramas with a literal 3×3 bank of flush schematic feeds built from live simulation data: room footprints, doorway stubs, rival occupants, cyan anchor halos, and the guardian's red room marker.
- Kept facility-camera knowledge diegetic: panel rendering is read-only and regression-tested not to change `MapKnowledge`.
- Added pure content-model tests, live guardian relocation coverage, multi-digit room labels, panel entity-budget/geometry checks, and style-owned observation signal treatments.
- Viewed the tether and guardian observation-room captures in `docs/evidence/phase_65_observation/`; both show the claimed panel states and the visual audit reports zero findings.
- Verification: `cargo fmt --all -- --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets` pass.

### Arc F — Sight & Sound (Phases 54–60) `[x]`
Landed 2026-07-09/10 across seven sub-agent phases; committed and ledgered 2026-07-10 (Arc H Phase 61):
- **True Singleplayer (54):** `BotPopulations` toggles (rival teams / AI teammates / guardian) as deterministic match config through `MatchDirector::new`; `OBSERVED2_BOTS` override; lobby toggles persisted with the career profile; disabled populations never spawn in the sim.
- **Audio architecture (55):** a Match-scoped `AudioDirector` with a data-driven cue table — buses over the settings channels, sting-ducks-ambience easing, structural per-class cooldowns/instance caps; every spawn site migrated.
- **Audio content (56):** test-enforced signal→cue coverage table (`docs/arc_f/audio_coverage.md`), spatial rolloff/occlusion classes, four new cues, CC-BY libraries deleted. Post-landing review fixed the ambience-bed double-write and wired corridor/gantry hallway beds; playtest iteration replaced the whole palette with the deterministic in-repo synthesizer `tools/generate_audio.py` (composed 32 s liminal beds — chord progressions, sparse echoing melodies, tape-wobble pads — and one timbre family per cue family; klaxon now a 1.4 s loop).
- **Sprite layer (57–60):** metadata-driven pipeline over the OpenGameArt CC0 intake (`assets/oga_25d/derived/` + `oga_25d_lab`), eight gameplay-object sprite slots with style halos, directional rival/guardian sheets with a pure tested `actor_frame`, seeded role-driven room dressing under three hard rules, `WALL_ALBEDO_LAB`, and a minimal near-interactable reticle dot.
- **Verification:** 189 workspace test suites green, clippy clean. **Review caveats absorbed into Arc H:** imported albedos visually overpowered the district palettes (Phase 62), the rivals evidence capture shipped empty (re-captured, and the origin of Arc H's falsifiable-evidence rule), threshold/rebind/observation-room playtest bugs scheduled as Phases 63–65.

### Phase 57 — Sprite Metadata Pipeline & the OGA Lab `[x]`
Turned the raw OpenGameArt intake into a metadata-driven sprite pipeline:
- **Derived Assets & Metadata**: Created `assets/oga_25d/derived/` with cropped transparent PNGs and JSON metadata skeletons (rects, pivots, ppm, directional counts, semantic clips) for actors (guard), objects (keystones, cards, batteries, tools), decorations (columns, torches), and textures (tiles).
- **Derive Script**: Implemented a deterministic, rerunnable Rust utility `derive_sprites` that handles extraction and image processing to ensure byte-identical rebuilds.
- **OGA Lab**: Created `oga_25d_lab` demonstrating pacing guard actors with angle-driven 8-way directional clips, objects and decorations at game-plausible scales, texture tile sampling, a debug metadata overlay, and interactive billboard-vs-directional/auto-orbit controls.
- **Verification**: Added metadata loader unit tests verifying coordinate bounds, normalized pivots, clips consistency, and multiple-of-direction constraints. All workspace clippy and test suites run warning-free and pass.


### Phase 56 - Audio Content & Spatial Depth `[x]`
Closed Arc F's audio content pass on top of the mixer:
- **Coverage audit:** added `docs/arc_f/audio_coverage.md`, with tests enforcing that every `MatchAudioCue` variant is represented and no unresolved placeholder remains.
- **Spatial classes:** the director now applies cue-table rolloff and occlusion classes; rival bleed uses threshold/wall attenuation instead of local volume math, and guardian proximity has a low dread cue.
- **Critical cue gaps:** tool interactions, keystone pickup, exit unlock, and guardian dread are semantic optional slots with short in-repo synthesized OGG files.
- **License cleanup:** removed the attribution-required raw OGA sound archives and scrubbed their manifest/source references; game-ready sound slots are covered by the source ledger.
- **Verification:** `cargo fmt --all`, `cargo test -p observed_assets`, and `cargo test -p observed_game` pass.

### Sprite3D Dev Placeholder Pass `[x]`
Added a Bevy `0.18`-compatible 2.5D sprite placeholder path for dev-visible actors
and devices. `sprite3d_placeholder_lab` proves `bevy_sprite3d` with loaded-image
gating, fallback meshes, yaw-facing billboards, and reset safety; the assembled game
now uses checked-in CC0 Kenney sprite slots for rival avatars, the guardian, and the
anchor/control device when loaded, while preserving procedural fallbacks.

### Phase 49 - Audio & Game Feel `[x]`
Finished the Arc E audio/game-feel pass:
- **Manifest-owned audio slots:** per-district ambience now lives in `observed_assets::DISTRICT_AMBIENCE`, and UI sound slots point at the checked-in `ui_click`/`ui_hover` drop-ins, so the game layer no longer hard-codes district sound paths or string slot names.
- **Stutter diagnosis and fix:** the WIP pause/resume ambience path and hot diagnostics logging were removed; ambience beds are stable loop entities that cross-fade volume only, avoiding stream restart churn. The persistent "Geiger counter" artifact was confirmed as repeated landing cue spawns from exact floor/deck contact toggling airborne/grounded in `observed_traversal`; resting support now stays grounded and reports `landed` only on an actual fall-to-support transition. Muted SFX channels suppress one-shot cue entities instead of spawning silent sounds.
- **Stings and movement feel:** rival bleed stays attenuated, collapse and klaxon stings are settings-gated and one-per-event/loop, jump/land cues feed smooth camera easing, and teleport/collapse feedback uses small height/level offsets instead of shake or full-screen violence.
- **Evidence and verification:** added Phase 49 regression tests for manifest alignment, UI slot loading, muted cue gating, one-shot/loop sting behavior, idle-match audio cue stability, and traversal resting support. `OBSERVED2_VIS_AUDIT=docs/evidence/visual_audit` reports zero findings, and `docs/evidence/bot_pov/bot_pov.gif` was refreshed from 120 captured frames.
- **Verification:** `cargo fmt --all`, `cargo test -p observed_traversal` (17 tests), `cargo test -p observed_game` (195 tests), `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass.

### Phase 47 — WFC Corridor Interiors `[x]`
Closed Arc D by reviving the archived hallway-interior WFC generator into the game:
- **Generator home:** the hallway-interior WFC logic archived in Arc G1 now lives in `observed_facility::wfc` behind the `wfc` feature (`generate_interior_walls`/`InteriorSeg`), so `ghx_proc_gen` stays out of the game crate; `game::wfc_interior` is the pure `InteriorSeg → WallSeg` adapter that picks the same door columns the DFS maze would.
- **Role-driven, deterministic selection:** a grid hallway on a `CorridorRole::Mystery` edge (resolved via `MapSpec::corridor_role_between`, frozen into `FrozenDest.corridor_role` so preview and crossing agree) renders a WFC labyrinth; every other role and the specless dev map keep the DFS+braid maze; a WFC non-convergence falls back to DFS as a pure function of the seed.
- **Proven real, not silent fallback:** a pinned-seed test shows WFC converges with zero retries on every real hallway grid size (4×4/5×6/6×7/7×5/4×8); selection, fallback determinism, and bot routing through a WFC interior are all covered. The lab archive shrank to a re-export of the live code.
- **Verification:** 800 workspace tests, 35 `observed_facility --features wfc` tests, clippy clean with the feature on and off.

### Phase 46 — The Liminal Flip `[x]`
Completed Arc D's default-map flip and liminal comfort pass:
- **Generated maps by default:** `game::map_catalog` now defaults to `liminal_wfc_v1`; `OBSERVED2_MAP=dev` / `sector_relay_v1` keeps the old authored fixture available for regression testing.
- **Fast generated-map iteration:** map specs are memoized per `(map, seed)`, and the game-side corpus proves generated maps produce complete placed matches across seeded `MatchDirector` runs.
- **Liminal scale dials:** `game::layout` owns room and hall scale multipliers; room footprints scale by `RoomRole`, while authored non-grid/non-Gantry hallway templates stretch/widen without changing edge variation determinism.
- **Validation and evidence:** semantic map geometry audits now use a liminal renderer-frame bound, and the generated-map audit evidence in `docs/evidence/map_audit/` was refreshed after the scale pass.
- **Verification:** `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p observed_game`, `cargo test --workspace`, and `OBSERVED2_CAPTURE_MAP_AUDIT=docs/evidence/map_audit cargo run -p observed_game` pass.

### Phase 44 — Map-Agnostic Plumbing `[x]`
Added the map selection layer needed before generated maps enter the game:
- **Validated catalog:** `game::map_catalog` owns active map selection, normalizes `OBSERVED2_MAP`, defaults to `sector_relay_v1`, and validates every registered `MapSpec` builder before returning it.
- **Startup plumbing:** interactive match setup, headless `play_match()`, keystone defaults, debug room coercion, and map-audit capture now read the active map spec rather than directly constructing Sector Relay.
- **Role-aware geometry:** the teleport `Nav` snapshot now carries the active map room role so monitor-room footprint shaping comes from the selected `MapSpec`, not a hard-coded sector lookup.
- **Verification:** added catalog tests for default selection, aliases, and unknown-name rejection; `cargo test -p observed_game` passes.

### Phase 41 — Pressure Made Flesh `[x]`
Made the director's pressure readable in the played game:
- **Collapse in geometry:** collapse-sealed room slots now stay in the room nav as non-traversable `GapKind::Collapsed` thresholds, so claimed doorways render as rubble instead of silently disappearing.
- **Style-owned rubble:** the match asset registry builds a `SurfaceRole::Rubble` material, collapsed thresholds spawn `Collapsed rubble` leaves, and the visual audit status distinguishes `collapsed_rubble`.
- **Drained and klaxon lighting:** place palettes now derive from live collapse state; dying/collapsed places use `observed_style::drained`, and the elimination first-escape countdown drives a facility-wide `observed_style::klaxon` light state.
- **Presentation refresh:** the place render signature tracks sealed slots, collapse state, and countdown state, so pressure changes rebuild the current place without waiting for another teleport.
- **Verification:** added Phase 41 game regressions for drained/klaxon palette state and rubble threshold rendering. `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass.

### Phase 40b — The Gantry Made Real `[x]`
Replaced the flat in-game placeholder with the genuine two-level jump-map hall:
- **Height-aware thresholds:** `DoorGap` carries `floor_y` and crossings are feet-height gated, so the deck-level exit is reachable only from the deck a ground body walks beneath.
- **Walkable decks:** `PlaceGeom` carries `DeckSeg` decks that `place_arena` extrudes as standable solids; the gantry projects the pure course's six platforms plus a five-step mount stair under the controller's 0.45 step limit.
- **Four real thresholds:** ground entry, deck-level upper exit to the destination, ground safe-bypass to the destination, and the understory side exit back the way you came — falling now genuinely reroutes the crossing.
- **Readable commitment:** decks render with the `GantryDeck` surface, emissive `GantryEdge` rim strips mark every jump line, and an `Understory` landing marker shows where a fall puts you before you jump; legend-backed per the Legibility Contract.
- **Verification:** 116 game tests and workspace clippy clean; the capture helper now honors a gap's true target so evidence drivers cannot misroute the side exit.

### Phase 40 — The Gantry (Jump-Map Halls) `[x]`
Finished the Phase 40 vertical corridor proof:
- **Pure traversal model:** `observed_traversal::gantry` owns the authored two-level course, upper platform thresholds, lower understory exits, deterministic route runner, and timing windows.
- **Readable style roles:** `observed_style` now includes semantic treatments for gantry decks, lit jump edges, and visible understory landings, keeping platform commitment cues legend-backed.
- **Playable lab:** new `gantry_lab` renders the jump-map hall and lets agents review clean-jump, fall-recover, and safe-bypass runs (`1`/`2`/`3`, `R` reset).
- **Game template hook:** `HallwayFlavor::Gantry` is in the assembled game's hallway library with a walkable lower-bypass projection that respects the current two-endpoint teleport contract.
- **Verification:** focused tests are green for `observed_traversal`, `observed_style`, `gantry_lab`, `style_lab`, and the game hallway/bot gantry paths.

### Phase 39 — Doors With Identity `[x]`
Finished the Phase 39 `discovery_lab` read-layer proof that typed rooms can be read before commitment:
- **Style-owned door reads:** `observed_style::DoorIdentityRole` now owns legend-backed glyphs, colors, emissions, and ambience labels for typed-room doorframes, and `style_lab` renders the new semantic row.
- **Threshold reads vs truth:** `discovery_lab` separates team-map knowledge (tile fill) from current doorframe reads (frame/glyph/bleed). Decoys advertise an exit-like `E` signal until direct visitation resolves the truth as `!`.
- **Type-true payoffs:** Sensor visits tag adjacent rooms as Sensor-fed team-local map knowledge; Decoys never count as keystones and track resolved false-exit lies.
- **Reader bot proof:** the seeded corpus asserts a scripted reader bot escapes every run and beats the random-door bot by at least `READER_BOT_TARGET_VISIT_MARGIN` mean visits over `READER_BOT_SEEDS`.
- **Verification:** focused tests are green for `observed_style`, `style_lab`, and `discovery_lab`.

### Phase 38 — Contested Observation `[x]`
Finished the Phase 38 `contention_lab` proof that observation is objective and shared while knowledge remains team-local:
- **Shared pins and private knowledge:** `ContentionWorld` keeps team-attributed presence and anchors as one shared freeze predicate, while each team records its own stale/fresh doorway ledger for fog-of-war over truth.
- **Route-denial evidence:** the headless seed-corpus race now has a deterministic Denier policy that evaluates route candidates with a one-decoherence probe, then spends its bounded anchor budget on the room that best improves the acting team's predicted path position.
- **Room rhythm in the race model:** each tick separates the room decision beat (observe/place anchors), decoherence, and traversal, so anchors affect the next graph truth before teams commit to a corridor step.
- **Verification:** Phase criteria pass over the 200-seed corpus: denial changes 159/200 outcomes (79.5%), improves team 0 mean placement from 3.270 to 2.995, produces byte-identical repeated digests, preserves solvability after every decoherence, and has no all-Denier stalemate (all 200 seeds finish, worst pinned fraction 0.778).

### Phase 37 — Game-Layer Architecture Cleanup (Refactor Arc G) `[x]`
Dissolved the assembled game's accumulated architecture debt in six verified stages (plan and as-landed record: `docs/refactor_game_arc_plan.md`):
- **Dead code & dedup:** archived the unused hallway-WFC generator into `wfc_proc_gen_lab` (dropping `ghx_proc_gen` from the game), converted the game's copy-pasted splitmix64 PRNGs to `observed_core::SplitMix`, and moved the game's spatial constants into a game-owned `layout` module.
- **Screens hub dissolved:** `screens.rs` shrank from 710 to 177 lines; shared state moved to `sim/` (simulation resources), `view/` (theme, drop-in asset registry, presentation components), with explicit imports everywhere and `arch_check` ratchet tests banning glob re-exports, non-test `use super::*`, and sim→presentation imports.
- **One match brain:** the new `MatchDirector` owns the live networked match and the elimination series behind a single tick/outcome API; headless career matches run the same director as the on-screen match, pinned by a characterization test.
- **Renderer flattened & session lifecycle:** the place renderer's strategy/policy indirection became plain functions and data; `place/mod.rs` went from 1,288 to 42 lines; every Match resource is enumerated once and a no-leak test guards `OnExit(Match)` (fixing four resources that previously leaked across matches).
- **Evidence consolidated:** both screenshot pipelines (captures and the visual audit) now live under `game/src/evidence/` behind one `configure()`, staging the brain through shared director helpers; the `OBSERVED2_*` env-var surface is unchanged.
- **Verification:** every stage landed green — 672 workspace tests, workspace clippy clean.

### Phase 36 - Map Iteration & Render-Bounds Validation `[x]`
Added a validation loop for the semantic Sector Relay map before expanding map content:
- **Pure semantic map audit:** `game::map_validation` builds teleport-place nav snapshots from `MapSpec`, then checks room bounds, doorway gaps, polygon wall splits, interior wall bounds, and representative semantic room coverage.
- **Many-iteration traversal check:** Sector Relay rooms and hallways are audited across multiple match seeds and decoherence versions; failures report map name, seed, version, place, room role, connections, bounds, gap count, and spawn.
- **Bot routing hardening:** local bot routing keeps the existing navmesh fast path and now falls back to a conservative grid route when generated labyrinth obstacles defeat the navmesh.
- **Visual evidence hook:** `OBSERVED2_CAPTURE_MAP_AUDIT=<dir>` captures representative semantic rooms for quick review without changing normal play.

### Phase 35 — Bot-Spectated Procedural Co-op Race `[x]`
Integrated the original co-op/team race goals into the AI spectator path:
- **Procedural co-op seed plan:** seeded role assignment now chooses keystone rooms, a two-operator gate, an anchor beat, a team-keyed teleport relay, a guardian pressure room, and a control room over the protected nine-room route.
- **Pure two-member teamplay brain:** `observed_match::teamplay` simulates two members per team, co-op station completion, shared keystones, anchor torch use, team-keyed teleport pads, guardian setbacks, decoherence events, and deterministic round outcomes.
- **Elimination-series bridge:** bot-spectated co-op outcomes now feed the existing series rules, so escapes, eliminations, adversary growth, and final standings come from the teamplay run instead of an unrelated autoplay timer.
- **Spectator integration:** `Spectate AI` owns the procedural co-op state, advances it while the first-person bot drives the visible body, and reports seed/plan/team metrics in the HUD and TAC-MAP.
- **Focused verification:** added pure tests for seed solvability, two-operator gating, tool use, guardian setbacks, determinism, and game integration assertions for the spectator menu path.

### Phase 34 — Spectate AI Menu Mode `[x]`
Integrated the autoplay slice with the assembled first-person match:
- **Main-menu spectator option:** added `Spectate AI`, launching directly into the 3D Match on the fixed demo seed.
- **Bot-controlled first-person body:** reused the capture bot's navmesh/threshold routing so the spectator follows a real body through the same collision, doorway, teleport, keystone, and guardian systems.
- **Manual input handoff boundary:** normal Play still uses player input; Spectate AI suppresses manual movement and keeps the cursor unlocked while the bot drives.
- **HUD feedback:** the Match HUD clearly reports when the body is AI-controlled.

### Phase 33 — AI Elimination-Series Slice `[x]`
Added the next match layer implied by the finished roadmap reflection:
- **Elimination series:** active teams race repeated keystone-route rounds until one team remains.
- **First escape countdown:** the first team through the exit starts a deterministic countdown; teams that fail to escape before survivor slots fill or the countdown expires are eliminated.
- **Adversary escalation:** eliminated teams join the facility adversary for later rounds, raising pressure in the autoplay model.
- **Fully AI playtest path:** every team can run automatically for capture/evidence, while the assembled game keeps the first-person match as a manual takeover surface.
- **Team-keyed tools:** pads and anchor state now carry team identity so future team inventories can share the same rule boundary.

### Phase 32 — ASCII Map Editor & Topology Validation `[x]`
Add structured editing and geometry validation capabilities to prepare the workspace for custom facility topologies:
- **ASCII Map Editor:** Design a simple, human-readable text representation of rooms, hallways, and portals, along with a parser that constructs the in-memory graph.
- **Topology Validators:** Add automated validation rules to ensure generated or loaded levels have no overlaps, no wall segments shorter than `MIN_WALL_LENGTH`, and all room ports align correctly.

### Phase 31 — Specialized Room Types `[x]`
Implemented specialized room types to diversify gameplay:
- **Master Room (Room 4):** 8-sided regular polygon geometry with direct one-way exits to all other rooms.
- **Tether Camera Room (Room 5):** 3x3 holographic display panels glowing cyan when the corresponding room has a player anchor torch active.
- **Guardian Observation Room (Room 6):** 3x3 warning panels flashing red when the guardian enters the corresponding room.
- **Guardian Control Room (Room 3):** Central interactive console that allows players to reassign the guardian to hunt rival teams.
- **Interior Collisions:** refactored analytical containment to check `geom.interior` so players physically collide with the Room 3 console and other interior obstacles.

### Phase 30 — Fix Bot Pathfinding (Lab & First-Person) `[x]`
Replaced ad-hoc grid pathfinding with a stable, dynamic navmesh pathfinder in both the simulation lab and the main game. Prevents the spectating bot from getting stuck on pillars.
