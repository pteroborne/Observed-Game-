# Architecture Refactor Plan

This plan is based on `Catalogue.md` in the current worktree. The user request
named `Catalog.md`; this repository currently contains `Catalogue.md`, so this
document treats that file as the intended project catalogue.

## Current Shape

The project has a strong prototype history: `Catalogue.md` lists 329 project
files, with a root Cargo workspace, two shared crates, a large set of runnable
labs, an assembled `game`, drop-in assets, and evidence screenshots. The good
pattern is visible everywhere: most labs have a thin `main.rs`, a `lib.rs`
plugin/run helper, a pure `model.rs` or equivalent, and a Bevy `lab.rs`
presentation layer.

The architectural problem is that several labs have stopped being only labs.
They are now production libraries. The assembled game depends directly on lab
crates such as `fps_hybrid_match_lab`, `fps_maze_lab`, `fps_controller_lab`,
`competitive_facility`, `mutable_facility`, `net_match_lab`, `network_lab`,
`progression_lab`, `session_lab`, and `style_lab`. That makes the dependency
graph read as a chain of experiments instead of a stable game architecture.

The catalogue also identifies one clear hotspot: `game/src/screens.rs` is the
central game file and is rated 2/5 for SOLID. It owns menu screens, match
presentation, asset loading, 3D setpieces, audio, HUD, tactical map, input
pumping, and cleanup. It works, but it is too broad to remain the game shell.

## Refactor Goal

Keep the lab discipline, but promote proven lab logic into reusable crates. Labs
should demonstrate and debug systems. The game should compose stable systems.
No production crate should need to depend on `labs/*`.

The target dependency direction is:

```text
game
  -> Bevy app shell and presentation adapters
  -> reusable Bevy integration crates
  -> pure simulation/domain crates
  -> observed_core / player_input-style foundations

labs
  -> reusable crates
  -> lab-only presentation/debug harnesses

assets
  -> consumed through a shared manifest/loader convention
```

Hard rules for the refactor:

1. `crates/*` must never depend on `labs/*` or `game`.
2. `game` must depend on `crates/*`, not `labs/*`.
3. Labs may depend on `crates/*`; cross-lab dependencies should be deleted as
   the referenced behavior is promoted.
4. Pure simulation crates should avoid Bevy. Bevy should enter through adapter
   crates or clearly named `bevy` modules.
5. Domain IDs remain stable newtypes. Bevy `Entity` values stay presentation or
   runtime handles only.
6. Assets and visual treatments are addressed by semantic names, not ad-hoc
   file paths and colors spread through presentation code.
7. Every extraction must leave the old lab runnable before moving to the next
   extraction.

## Proposed Workspace

This is the target map, not a command to create every crate immediately. Create a
crate only when moving real code into it.

```text
crates/
  observed_core/          Stable IDs, small math/domain primitives.
  player_input/           Pure PlayerIntent and PlayerId; Bevy input adapters split out.
  observed_style/         Semantic state -> visual treatment; moved from style_lab.
  observed_assets/        Asset slot manifest, paths, presence checks, loading facade.
  observed_app/           Common Bevy app state, scoped cleanup, capture hooks.
  observed_facility/      Rooms, ports, sockets, room definitions, graph topology.
  observed_observation/   Observed/unobserved/frozen/rerouting rules.
  observed_doors/         Door threshold and observe/decohere mechanics.
  observed_traversal/     Movement, FPS body stepping, climbing, traversal sockets.
  observed_interaction/   Interaction, equipment, shared/exclusive operation.
  observed_match/         Team race, hybrid match rules, director pressure, objectives.
  observed_net/           Lockstep protocol, recorded input, replayable session model.
  observed_progression/   Career/profile/session data that is not screen code.
  observed_presentation/  Shared Bevy rendering/HUD/tac-map adapters for the game.

labs/
  *_lab/                  Thin executable harnesses around one crate capability.

game/
  src/
    lib.rs                App construction and top-level state registration only.
    screens/              Splash/menu/loadout/lobby/results modules.
    match_view/           First-person match presentation and HUD.
    assets.rs             Uses observed_assets; no hard-coded slot logic.
    capture.rs            Evidence capture systems.
```

## Boundary Details

### Foundation

`observed_core` should keep stable identifiers and tiny domain primitives. It
should not become a dump crate. `PlayerIntent` currently lives in `player_input`
but uses Bevy `Vec2` and derives `Component`. Split that into:

- Pure data: `PlayerId`, `PlayerIntent`, sanitization, recording-friendly
  structures.
- Bevy adapter: component derives, keyboard/controller sampling, resource wiring.

This keeps bots, replays, network packets, and tests from inheriting Bevy unless
they actually need ECS integration.

### Simulation Crates

Promote the pure models first:

- `observation_lab/src/model.rs` -> `observed_observation`
- `constraint_lab/src/model.rs` plus graph constraints -> `observed_facility`
  or `observed_observation`, depending on ownership
- `door_lab/src/door.rs` -> `observed_doors`
- `equipment_lab/src/model.rs` and `interaction_lab/src/model.rs` ->
  `observed_interaction`
- `fps_controller_lab/src/controller.rs`, movement stepping, and climbing
  kernels -> `observed_traversal`
- `competition_lab`, `competitive_facility`, `fps_hybrid_match_lab`, and
  `director_lab` pure match state -> `observed_match`
- `network_lab/src/model.rs` and `net_match_lab/src/netmatch.rs` protocol
  pieces -> `observed_net`

Each moved module should keep its existing unit tests. The old lab should import
the new crate and continue to provide the visual/debug harness.

### Presentation

Split `game/src/screens.rs` by runtime responsibility, not by arbitrary size:

- `screens/menu.rs`: menu widget creation, cursor navigation, actions
- `screens/loadout.rs`: cosmetic selection and profile display
- `screens/lobby.rs`: session/lobby presentation
- `match_view/runtime.rs`: `MatchRuntime`, fixed-step match pumping
- `match_view/place_renderer.rs`: room/hallway geometry and setpieces
- `match_view/hud.rs`: HUD, pause, tactical map
- `match_view/audio.rs`: cue selection and playback
- `match_view/input.rs`: mouse/keyboard sampling into `PlayerIntent`
- `capture.rs`: all evidence screenshot systems

The game's `ObservedGamePlugin` should read like composition: register states,
add screen plugins, add match-view plugin, add capture plugin if environment
variables request it. It should not know how to spawn a wall mesh or play a
footstep.

### Threshold Continuity Contract

The room/hallway system needs stable language because threshold behavior touches
geometry, lighting, observation, anchors, future map editing, and procedural
generation.

Terms:

- A **threshold slot** is a possible doorway location on a room or hallway
  template. In a future editor this is the authored port point; in procedural
  generation it may be derived from room/hall type plus seed.
- A **threshold assignment** is the current destination bound to a slot. This is
  the part that may decohere while unobserved.
- A **hallway edge** is the realized traversal space for one assignment. Its
  template/variation can be selected by seed and edge identity, but once a
  threshold is observed or anchored, the selected relation must be replayable.
- A **room anchor** is a room-level lock. Dropping an anchor in a room stores the
  room's complete visible threshold assignment table at that moment. While the
  anchor remains, the room must render exactly that threshold count and exactly
  those destinations. No new live graph relation may appear as a new threshold.
- A **hallway anchor** is an edge-level lock. Dropping an anchor in a hallway
  freezes that hallway relation and variation, but it does not by itself lock the
  complete threshold set of either endpoint room.

The invariant is: preview, crossing, arrival geometry, collision, and threshold
lighting must all read the same threshold assignment snapshot. If a player sees a
threshold previewing hallway A, crossing that threshold must enter hallway A. If
a room is anchored with N visible thresholds, it must keep those N thresholds and
only those N thresholds until the anchor is removed.

The future map editor and any WFC/procedural system should therefore separate
slot generation from assignment solving. Room type, hallway type, authored ports,
and seed may decide how many slots exist and where they sit. The mutable graph or
WFC solver decides where each slot leads. Observation and anchors freeze
assignments, not ad-hoc rendered meshes.

### Style

`style_lab` already contains the right abstraction: semantic role to visual
treatment, with tests for legibility. Move that module into `crates/observed_style`
and leave `style_lab` as the visual proof app. All game/lab presentation code
should consume `observed_style`; none should invent gameplay colors locally.

### Assets

`asset_lab/src/manifest.rs` is also a reusable abstraction trapped in a lab. Move
the manifest, slot definitions, presence checks, and path helpers into
`crates/observed_assets`. Then:

- `asset_lab` becomes only the showcase.
- `game` consumes the same manifest.
- Asset paths stop being duplicated between `asset_lab`, `assets/README.md`,
  `assets/ASSET_PLAN.md`, and `game/src/screens.rs`.

Do not replace the code-as-art visual direction with a heavy authored-asset
pipeline. The manifest should describe optional assets and procedural fallbacks,
not require a content build step.

## Refactor Sequence

1. Add dependency guardrails in documentation first: new production code belongs
   in `crates/*`; labs are not production dependencies.
2. Extract `observed_style` from `style_lab`. Update `style_lab` and `game` to
   depend on it.
3. Extract `observed_assets` from `asset_lab`. Update `asset_lab`, `game`, and
   asset docs to read from one slot list.
4. Split `game/src/screens.rs` into modules without changing behavior.
5. Promote one pure model at a time, starting with the least entangled:
   observation, doors, interaction/equipment, traversal, facility topology,
   match state, network state.
6. After each promotion, update the original lab to import the crate and rerun
   its focused tests.
7. Remove `game` dependencies on lab crates one by one. The game is considered
   architecturally clean when its local path dependencies are only `crates/*`.
8. Only after the game is clean, consider third-party Bevy ecosystem assets.

## Success Criteria

The refactor is successful when:

- `cargo metadata` shows no `game -> labs/*` dependencies.
- Each lab still launches independently.
- Pure crates have focused tests that run without rendering plugins.
- Bevy adapter crates have scheduling/lifecycle tests where entity cleanup
  matters.
- `game/src/lib.rs` reads as top-level composition.
- No single presentation file owns menus, match runtime, assets, HUD, audio, and
  capture at once.
- `OBSERVED2_CAPTURE*` screenshot hooks still work.

## Bevy Assets Catalogue

Source visited: https://bevy.org/assets/

The Bevy Assets page describes itself as a community collection of Bevy assets,
plugins, learning resources, and apps. It is mostly useful here as a catalogue of
ecosystem crates, not as a CC0 art replacement source. The current raw art assets
come from Kenney, ambientCG, and Poly Haven and should stay governed by
`assets/SOURCES.md`.

Compatibility note: this workspace is pinned to Bevy `0.18.1`. Entries listed
below with `^0.19` should be treated as future candidates or checked for older
compatible releases before adoption.

| Current project area | Bevy Assets candidates | Fit | Recommendation |
| --- | --- | --- | --- |
| Drop-in asset manifest and loading | `bevy_asset_loader`, `bevy_common_assets`, `bevy_embedded_assets`, `bevy_dlc` | Medium | First extract `observed_assets`. Then consider `bevy_asset_loader` only if state-scoped loading becomes more complex than the current manifest. |
| Raw GLB/texture/audio assets | Built-in Bevy loaders already cover PNG/JPG/HDR/GLB/OGG/WAV with enabled features | High | Do not replace art sourcing through Bevy Assets. Keep CC0 art provenance in `assets/SOURCES.md`; centralize loading and fallbacks instead. |
| Optional scene metadata inside GLB files | `bevy_gltf_components`, `bevy_gltf_blueprints`, `skein` | Low now, possible later | Avoid while code-as-art is primary. Reconsider only if authored room/setpiece metadata becomes a bottleneck. |
| Neon outlines and signal readability | `bevy_mod_outline`, `bevy_vector_shapes`, `bevy_hanabi` | Medium | Keep `observed_style` as the semantic source of truth. Consider outline/particle plugins as render adapters for signals, not as gameplay logic. |
| Tactical map and debug vector drawing | `bevy_vector_shapes`, `bevy_svg`, `bevy_mod_gizmos` | Medium | Useful after `match_view/hud.rs` is split out. Prefer replacing ad-hoc drawing helpers, not the map model. |
| First-person debug camera | `bevy_flycam`, `bevy_customizable_camera_controllers`, `bevy_panorbit_camera` | Medium | Use for debug/spectator tooling only. The player controller should remain project-owned because traversal is core gameplay. |
| Character movement / collision | `bevy_rapier`, `avian`, `bevy-tnua`, `bevy_fpc` | Low to medium | Do not casually replace the custom controller. Run an isolated physics spike only if stairs/slopes/carryables exceed the simple authored-constraint model. |
| Fixed timestep smoothing | `bevy_transform_interpolation` | Medium | Candidate for presentation smoothing around the existing fixed-step controller, after traversal is extracted. |
| Input mapping and rebinding | `leafwing_input_manager`, `bevy_enhanced_input` | Medium | Keep `PlayerIntent` pure. Consider one of these only as the Bevy input adapter if rebinding/controller support expands. |
| Menus, HUD, and settings UI | `bevy_egui`, `bevy_flair`, `Bevy Lunex`, `bevy-ui-navigation`, `bevy_screen_diagnostics` | Medium | Native Bevy UI is fine for shipped UI. `bevy_egui` and diagnostics are strong debug-tool candidates; menu navigation could replace local cursor plumbing if it keeps tests simple. |
| Debug inspection | `bevy-inspector-egui`, `bevy_mod_debugdump`, `bevy-debug-text-overlay`, `bevy_screen_diagnostics` | High for labs | Add behind a `dev_tools` feature after core extraction. This can replace some custom overlays, but not required debug semantics. |
| Audio channel management | `bevy_kira_audio`, `bevy_audio_controller` | Low now | Current audio is simple. Consider only if the game gains layered ambience, ducking, snapshots, or more music state. |
| Persistence and settings | `bevy-persistent`, `bevy-settings`, `bevy_pkv`, `bevy_simple_prefs` | Medium | Good candidate once `observed_progression` owns profile/settings data. Keep the domain structs serializable and storage-agnostic. |
| Network transport | `lightyear`, `bevy_quinnet`, `naia`, `bevy_ggrs`, `bevy_matchbox`, `aeronet` | Future | Keep the deterministic lockstep model pure. When online networking becomes active, choose topology first, then spike one transport behind `observed_net`. |
| Recorded/manual integration testing | `bevy-autoplay` | Medium | Candidate for reproducible manual flows after app states are modular. It complements, not replaces, unit tests and screenshot capture. |
| Procedural generation helpers | `Noiz`, `bevy_ghx_proc_gen`, `bevy_generative` | Low now | The game needs authored readable rooms and constraints. Do not add WFC/proc-gen dependencies until the room vocabulary is stable. |

## Dependency Policy For Bevy Assets

Before adding any third-party Bevy asset:

1. Prove the problem exists in this codebase.
2. Check Bevy `0.18.1` compatibility or plan a deliberate Bevy upgrade.
3. Add the dependency behind the smallest adapter crate.
4. Keep domain state independent of the plugin's component/resource types.
5. Add one lab that proves the dependency and one test that guards the adapter.
6. Document maintenance cost and fallback behavior.

The strongest near-term candidates are `bevy_asset_loader` for state-scoped
loading, `bevy-inspector-egui` or `bevy_screen_diagnostics` for lab/debug
inspection, `bevy_vector_shapes` for tactical-map drawing, and possibly an input
mapping crate after `PlayerIntent` is made pure. Physics, networking, and authored
scene workflows should wait.
