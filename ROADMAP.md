# Observed 2 — Roadmap

A live, status-tracked plan for **Observed 2**, derived from the standing design
doc ([agents.md](agents.md)) and the day-to-day working rules
([CLAUDE.md](CLAUDE.md)). AGENTS.md says *what* the game is and *why*; CLAUDE.md
says *how* to work in the repo; this file says *what is done and what comes next*.

## Direction: the 2D labs were the proof of concept; the target is first-person 3D

The plan all along was to **prove the risky design questions cheaply in 2D, then
take the reusable parts into a full 3D environment**. That target is now explicit:
**Observed 2 is heading toward a 3D first-person game.** The 2D work was not a
different game — it was the de-risking phase, and it paid off: the entire *game
brain* lives in a dimension-agnostic simulation layer with no rendering dependency.

What this buys us (see the first-person pivot section below for the full reuse map):

- **Ports as-is to 3D** — the observe/decohere graph, the constraint spine,
  competition, the facility director, the replay tape, player routes, and incentives
  are all pure `RoomId`/`DoorId`/`TeamId` logic. They do not know or care how they
  are drawn.
- **Becomes in-game tooling** — the 2D schematic/spectator/scrubber (`match_replay`)
  is the first-person game's **tac-map / spectator / killcam**; the director is the
  **AI adversary**; both already exist.
- **Gets rebuilt for 3D** — the player controller (movement/climbing), world
  geometry/collision, and the sprite/gizmo presentation. These are the "projection"
  layer the architecture always treated as swappable.

The first-person observation questions are now answered in two steps: Phase 19
proved that camera line-of-sight can drive the existing graph, and Phase 21 made
that sight continuous, occluded, and partial-room. The FPS arc (below) tackles the
remaining risks one feasibility lab at a time, the same way the 2D systems were
proven.

## Where the project stands (2026-06-19)

**Shared crates**

| Crate | Role |
| --- | --- |
| `crates/player_input` | The input boundary: `PlayerId`, `PlayerIntent`. |
| `crates/observed_core` | Canonical domain IDs: `RoomId`, `PortId`, `EquipmentId`, `TeamId` (re-exports `player_input`). |

**Foundation labs** (the proven 2D technical base)

| Lab | Proven |
| --- | --- |
| `labs/menu_lab` | App states, menus, gameplay lifecycle & cleanup. |
| `labs/control_lab` | Input → abstract `PlayerIntent`; 4 players, devices, recording/playback. |
| `labs/movement_lab` | Deterministic kinematic controller (jump assists, slopes, stairs, platforms). |
| `labs/climbing_lab` | Authored climbing modes: ladders, ledge grab/hang/pull-up/drop/shimmy, grapple. |
| `labs/interaction_lab` | Logical interaction state machine (activate/hold/carry/socket/climb). |
| `labs/room_lab` | Authored modular rooms, typed ports, validated connections, replacement. |
| `labs/equipment_lab` | Persistent equipment; carry/socket/deploy/power; survives carrier-leave & room replacement. |
| `labs/team_lab` | Four players, two teams; deterministic contention; co-op machine; separation/reunion. |

**Higher-level feasibility labs** (each isolates one deferred system — dimension-agnostic logic)

| Lab | Proven |
| --- | --- |
| `labs/observation_lab` | Connections rewire **when unobserved**, freeze **when observed**; deterministic decoherence; traversal follows current links. |
| `labs/constraint_lab` | A persistent route spine keeps the rewiring structure fully traversable; without it, rewiring can isolate a room. |
| `labs/competition_lab` | Multiple teams race to capacity-limited exits; indirect interference via a shared control; deterministic placement. |
| `labs/director_lab` | A collapse absorbs fall-behind teams into the facility director; absorbed teams escalate it; indirect-only threat. |
| `labs/replay_lab` | A match recorded as a deterministic input tape and replayed/scrubbed exactly; spectator reads simulation state. |
| `labs/route_lab` | Player-laid cables persist a route through decoherence; budget-limited and contestable (an opponent can cut them). |
| `labs/incentive_lab` | Scoring rewards spreading across rooms and revisiting regrown rooms; a split team out-scores a clumped one. |
| `labs/discovery_lab` | Typed rooms (core 5) hidden until visited and shifting when unobserved; a gated exit unlocks only on collected keystones/power; a constraint (shift only unharvested rooms) keeps the objective always solvable — without it a keystone can strand. |
| `labs/hazard_lab` | A director-steered pressure front requires two simultaneous relief roles; failed coordination stalls route progress without damage or progress loss. |
| `labs/network_lab` | Two peers run the Phase 20 FPS controller in complete-frame lockstep over checksummed datagrams; resend/ACK survives loss, delay, duplication, and reordering; hashes and replay prove exact convergence. |
| `labs/session_lab` | Compatible queued accounts form a four-seat lobby with stable player IDs and balanced teams; readiness, launch manifest, host migration, reconnect, closure, and rematch are deterministic. |
| `labs/progression_lab` | XP/levels/unlocks + per-slot equipped cosmetics that serialize and round-trip, provably orthogonal to the simulation (the match takes no profile, so cosmetics never change a result or replay). |

**Integration** (the game brain composed; 2D top-down/schematic presentation)

| Lab | Proven |
| --- | --- |
| `labs/facility_sandbox` | Menu + 4 players + run/jump/climb + 5 rooms + powered door + jack + room replacement + map + spectator cam; one completable objective. Combines the foundation systems. |
| `labs/mutable_facility` | First higher-level integration: `observation_lab` + `constraint_lab` folded into one objective — a team carries the cell to the exit while the unobserved structure rewires; observed rooms freeze and the protected spine keeps the exit reachable. |
| `labs/competitive_facility` | Second integration: the mutable facility + `competition_lab` + `director_lab`. Progress is spine position; a full match resolves deterministically — the fastest escape, the rest are absorbed. |
| `labs/match_replay` | Third integration: records the competitive match as a tape and replays/scrubs it exactly. The spectator/director camera + schematic map read replayed simulation state (this is the future in-game map/spectator). |

**3D / first-person** (the new direction)

| Lab | Proven |
| --- | --- |
| `labs/fps_observation_lab` | **First 3D lab.** Observation driven by the first-person camera's line of sight over `observation_lab`'s graph: seen rooms freeze, unseen rooms rewire; sight follows a doorway's current link, deterministically. Only new logic is a line-of-sight function; the proven graph is reused wholesale. |
| `labs/fps_controller_lab` | A **deterministic first-person controller** on the shared `PlayerIntent` — the 3D analogue of `movement_lab`, stepped at a fixed timestep with substep AABB collision. A recorded input sequence replays to an identical path, so replay/lockstep survive the move to 3D. |
| `labs/fps_visibility_lab` | Continuous first-person visibility: stable sub-room cells pass frustum + wall-occlusion tests, rooms can be partially observed, visible doorway endpoints freeze their graph connections, and deterministic decoherence changes only the unseen. |
| `labs/fps_rewire_lab` | Actual 3D modules replace as deterministic atomic batches only while every affected portal aperture is off-camera and doorway-clear; traversal pins the rendered route until arrival, preventing visible pops and mid-door stranding. |
| `labs/fps_facility_lab` | All nine graph rooms instantiated as authored 3D modules with the complete typed-port vocabulary. Every graph door maps uniquely to a Passage port; sealed graph doors generate collision panels; first-person threshold crossing follows the current graph partner. |
| `labs/fps_match_lab` | **FPS-arc capstone.** The full competitive match (`competitive_facility` brain + director AI) played in first person over `fps_facility_lab`'s 3D facility, with the `match_replay` schematic promoted to an in-3D tac-map. A tape of local round actions replays both the match and the first-person pose exactly. |
| `labs/fps_maze_lab` | **Hybrid maze arc start.** The proven room graph embedded in space as a concrete maze: rooms placed deterministically (seeded) and every graph connection routed as a real walkable corridor (gold spine) — no portals. Connected, navigable, seed-varied; a decohered graph still embeds navigably. |
| `labs/fps_reroute_lab` | The maze made **live**: unobserved decoherence re-routes corridors to different rooms via Phase 22's atomic off-camera swap — never in view, never under the player's feet. Observed rooms stay frozen; every reroute leaves a navigable maze; deterministic. |
| `labs/fps_hybrid_match_lab` | **Hybrid-maze capstone.** The complete competitive match played with fixed-step first-person traversal in the concrete rerouting maze. Entering the next spine room is the action boundary; each protected leg offers a pulsing risky shortcut and longer safe bypass; visible/player-overlapping reroutes defer; match, route hazards, rendered/target maze, and pose replay exactly. |

**Tooling** (not a numbered phase)

| Lab | Provides |
| --- | --- |
| `labs/asset_lab` | A drop-in asset convention: a manifest of texture/model/sound slots, each rendered as the loaded file (PNG/JPG, glTF/GLB, OGG/WAV) or a magenta placeholder if absent, with an overlay showing each slot's exact drop path. The shared `assets/` root + [assets/README.md](assets/README.md) list where to drop free/CC0 files. Adds `bevy_gltf`/`bevy_audio` scoped to this lab only. |

Every lab launches independently (`cargo run -p <lab>`), resets without
restarting, ships a debug overlay, and carries a captured screenshot under
[docs/evidence/](docs/evidence). The whole workspace is `cargo fmt` /
`clippy --workspace --all-targets` / `test --workspace` clean. 2D labs build Bevy
with `["2d", "png"]`; 3D labs with `["3d", "png"]`.

## Completed phases

| # | Phase | Result | Evidence |
| --- | --- | --- | --- |
| 0 | Foundation consolidation | `observed_core` shared IDs; labs migrated | — |
| 1 | Climbing Lab | authored vertical traversal modes | [png](docs/evidence/climbing_lab.png) |
| 2 | Equipment Lab | persistent item framework | [png](docs/evidence/equipment_lab.png) |
| 3 | Local Team Simulation | deterministic 4-player / 2-team contention | [png](docs/evidence/team_lab.png) |
| 4 | Facility Sandbox | first integration of the foundation | [png](docs/evidence/facility_sandbox.png) |
| 5 | Observation & Decoherence | the defining observe/rewire mechanic | [png](docs/evidence/observation_lab.png) |
| 6 | Mutable Graph Constraints | persistent spine keeps it traversable | [png](docs/evidence/constraint_lab.png) |
| 7 | Competition | teams + capacity-limited exits | [png](docs/evidence/competition_lab.png) |
| 8 | Facility Director | absorb-and-escalate adversary | [png](docs/evidence/director_lab.png) |
| 9 | Replay & Spectator | record-and-scrub over a real match | [png](docs/evidence/replay_lab.png) |
| 10 | Mutable Facility | observation + constraint spine folded into one objective | [png](docs/evidence/mutable_facility.png) |
| 11 | Competitive Facility | competition + director folded onto the mutable facility | [png](docs/evidence/competitive_facility.png) |
| 12 | Match Replay & Spectator | the competitive match recorded and scrubbed exactly | [png](docs/evidence/match_replay.png) |
| 13 | Persistent player routes | player-laid cables persist a route, contestable | [png](docs/evidence/route_lab.png) |
| 14 | Splitting & backtracking incentives | scoring rewards spread + revisiting | [png](docs/evidence/incentive_lab.png) |
| 15 | Cooperative megastructure hazards | coordinated relief contains a director-steered pressure front; failure delays progress indirectly | [png](docs/evidence/hazard_lab.png) |
| 16 | Deterministic networking | reliable complete-frame lockstep; hostile datagrams converge; hashes and replay match | [png](docs/evidence/network_lab.png) |
| 17 | Matchmaking / session formation | compatible queue → balanced lobby → validated launch → reconnect/rematch lifecycle | [png](docs/evidence/session_lab.png) |
| 19 | FPS Observation | line-of-sight drives the observed set in first-person 3D | [png](docs/evidence/fps_observation_lab.png) |
| 20 | FPS Controller | deterministic first-person controller; recorded input replays exactly | [png](docs/evidence/fps_controller_lab.png) |
| 21 | Continuous line-of-sight | frustum + occlusion, partial rooms, unseen-only decoherence | [png](docs/evidence/fps_visibility_lab.png) |
| 22 | Rewire while unobserved | atomic off-camera module replacement; traversal-safe doorway pinning | [png](docs/evidence/fps_rewire_lab.png) |
| 23 | 3D facility from graph | typed 3D modules; exact graph projection; first-person graph traversal | [png](docs/evidence/fps_facility_lab.png) |
| 24 | First-person competitive match | the full match played first-person and replayed exactly | [png](docs/evidence/fps_match_lab.png) |
| 25 | Spatial maze layout | the graph embedded as a navigable maze of rooms + real corridors | [png](docs/evidence/fps_maze_lab.png) |
| 26 | Rerouting passages | corridors re-route off-camera as the unobserved graph rewires | [png](docs/evidence/fps_reroute_lab.png) |
| 27 | First-person hybrid match | physical maze traversal + full competitive match + safe reroutes + exact replay | [png](docs/evidence/fps_hybrid_match_lab.png) |
| 18 | Progression & cosmetics | XP/levels/unlocks + equip + save round-trip, orthogonal to the sim | [png](docs/evidence/progression_lab.png) |

The 2D foundation, gameplay/networking/session feasibility systems through Phase 17, and the
integration arc (Phases 10–12) are complete. Phases 19–23 establish the first-person path —
camera-driven observation, deterministic movement, continuous partial-room
visibility, pop-free off-camera geometry replacement, and a navigable graph-backed
3D facility — and Phase 24 ties them together: the whole competitive match, played
in first person and replayed exactly. Phases 25–27 replace the portal scaffold with
a generated, safely rerouting spatial maze and run the same complete match through
it. The FPS and Hybrid maze arcs are complete.

## The first-person pivot

The move to first-person 3D is a **presentation + controller** change, not a
rewrite of the game. The reuse map:

| Layer | Lab(s) | First-person 3D |
| --- | --- | --- |
| Observe/decohere graph, constraint spine | `observation_lab`, `constraint_lab` | **As-is** (pure logic) |
| Competition, director (AI adversary), routes, incentives, hazards | `competition/director/route/incentive/hazard_lab` | **As-is** |
| Replay tape, spectator, scrubber | `replay_lab`, `match_replay` | **As-is** — becomes the in-game map / spectator / killcam |
| Match resolution (graph-position progress) | `competitive_facility` | **As-is** |
| Line-of-sight observation | `fps_observation_lab`, `fps_visibility_lab` | **New, proven** (Phases 19 and 21) |
| Player controller (move/climb) | `movement_lab`, `climbing_lab` | **Rebuilt** in 3D; deterministic design philosophy carries |
| World geometry / collision, presentation | `room_lab`, `fps_facility_lab` | **Rebuilt and proven** in 3D (authored modules + typed ports + graph traversal) |

The three signature first-person feasibility risks are now proven: deterministic
real-time control (Phase 20), continuous partial-room visibility (Phase 21), and
rewire-while-unobserved rendering without visible swaps or doorway stranding
(Phase 22). Phase 23 instantiates the proven graph as navigable typed 3D room
modules, and Phase 24 integrates everything into a first-person competitive match
that records and replays exactly. The first-person prototype is now end-to-end.
The **Hybrid maze arc** (Phases 25–27) then made that coherence concrete: the
proven graph is embedded in space as a generated maze, its passages reroute safely
when unobserved, and the complete competitive match is physically played and
replayed in that shifting labyrinth. Deterministic lockstep now carries the same
fixed-step controller and replay discipline across a hostile datagram link.
Matchmaking/session formation is now proven on top of that boundary; progression
is the final carried-forward phase.

## Guiding principles (carry into every change)

- **Prove with a small lab before generalizing.** One primary technical question
  per lab; pure logic split out and unit-tested; clear debug visualization;
  reset without restart; no leaked entities.
- **Separate input / simulation / presentation.** Logic never reads hardware or
  depends on meshes/sprites; presentation is a projection of simulation state. *This
  is what makes the 2D → 3D move tractable — protect it.*
- **Production crates, lab harnesses.** Reusable game behavior belongs in
  `crates/*`; labs prove, debug, and demonstrate it. The assembled game should not
  depend directly on `labs/*`.
- **Stable domain identifiers**, not Bevy `Entity` values, for game identity.
- **Readable constraints over hidden complexity** — authored ports, discrete graph
  connections, persistent route spines.
- **Deterministic end to end** — the property that powers replay and will power
  networking. Carrying it into a real-time 3D controller is a first-class concern,
  not an afterthought.
- **Each new system (2D or 3D) gets its own isolated feasibility lab first.**

## Coverage of AGENTS.md "Later Gameplay Systems"

| Deferred system | Status |
| --- | --- |
| Quantum room states / observation & decoherence | ✅ `observation_lab` (+ FPS sight: `fps_observation_lab`, `fps_visibility_lab`) |
| Mutable graph constraints | ✅ `constraint_lab` |
| Persistent route infrastructure | ✅ authored spine in `constraint_lab`; player routes in `route_lab` |
| Team splitting and backtracking incentives | ✅ `incentive_lab` |
| Cooperative megastructure hazards | ✅ `hazard_lab` |
| Multiple competing teams / capacity-limited exits | ✅ `competition_lab` |
| Eliminated teams joining the facility AI / director controls | ✅ `director_lab` |
| Replay and spectator presentation | ✅ `replay_lab`, `match_replay` |
| Online networking | ✅ lockstep protocol + UDP packet compatibility in `network_lab` |
| Matchmaking | ✅ deterministic queue/lobby lifecycle in `session_lab` |
| Progression and cosmetics | ✅ `progression_lab` |

---

## Forward plan

The **active priority is now the architecture refactor** described in
[architecture.md](architecture.md). The project has outgrown the "labs as libraries"
phase: the assembled game currently depends directly on many `labs/*` crates, and
`game/src/screens.rs` owns too many unrelated responsibilities. No new gameplay-depth
work should start until the game composes stable `crates/*` libraries, the lab crates
return to being harnesses, and the presentation code is split by responsibility.

### Architecture refactor arc (active)

Goal: keep every proven system, but move reusable behavior into production crates so
the dependency graph expresses the intended architecture:

```text
game -> crates/* -> pure domain crates -> observed_core / player_input
labs -> crates/* + lab-only presentation/debug harnesses
```

**Dependency guardrails (the rules every refactor phase enforces):**

1. `crates/*` must never depend on `labs/*` or `game`.
2. `game` must depend on `crates/*`, not `labs/*` (currently violated — 10 edges; cut in R10).
3. Labs may depend on `crates/*` (and, for now, other labs); production code must not depend on labs. Cross-lab edges are deleted as the shared behavior is promoted.
4. Pure simulation crates avoid Bevy; Bevy enters through adapter crates/modules.
5. Domain identity stays in stable `observed_core` newtypes, never Bevy `Entity`.

**Refactor Phase R0 - Guardrails and dependency audit.** ✅ *(completed 2026-06-25 —
audit [docs/refactor_r0_audit.md](docs/refactor_r0_audit.md).)* The rules above are
now recorded in the roadmap and architecture plan, and the current dependency reality
is captured as architectural evidence: `observed_game` has **10 direct `game -> labs/*`
dependencies** (14 labs transitively) and only **2 `game -> crates/*`**; the two
production crates are clean (no Rule-1 violation); and the full cross-lab map is
recorded to inform the promotion sequence. First extraction target selected:
`observed_style` from `style_lab` (R1) — a clean leaf with no workspace deps that no
other lab depends on. Exit criteria met: `cargo metadata --no-deps` lists the current
`game -> labs/*` dependencies; the target dependency direction is documented; the first
extraction target is selected.

**Refactor Phase R1 - Promote `observed_style`.** ✅ *(completed 2026-06-25.)* The
pure visual-language module moved verbatim out of `labs/style_lab` into the new
`crates/observed_style` (Bevy with only the `bevy_color` feature — render-free data,
no `["3d", …]`). Its 9 pure-logic tests came with it. `style_lab` now `use
observed_style as style;` and stays the visual proof app (2 lifecycle tests green);
`game` imports `observed_style` directly and **drops its `style_lab` dependency
entirely** — so `game -> labs/*` fell from 10 to **9** and `game -> crates/*` rose to
3. The game still renders every surface/marker through the shared semantic treatments
(53 game tests green); no presentation code invents gameplay colours locally. `fmt`
and `clippy` clean across the affected packages. Exit criteria met: style tests live
in the new crate, `style_lab` still launches, the game renders through the shared
treatments, and no presentation code invents colours locally.

**Refactor Phase R2 - Promote `observed_assets`.** ✅ *(completed 2026-06-25.)* The
drop-in slot manifest (`AssetKind`/`AssetSlot`, the slot list, `assets_root` /
`slot_present` / `slot_full_path`) moved out of `labs/asset_lab` into the new
**`crates/observed_assets`** — a pure crate with **zero dependencies** (just
`std::path`, no Bevy). Its 4 manifest tests came with it (+1 new lookup-consistency
test = 5). Each slot is now also a named `pub const`, so `game` references paths
semantically (`observed_assets::CEILING.path`) — **all 14 of `game/src/screens.rs`'s
hard-coded asset-path literals are gone**, replaced by const slot references (the
optional `door` sound was folded into the manifest as its 21st slot, so the game holds
no literal asset path at all). `asset_lab` re-exports the crate as `manifest` and stays
the visual proof app (2 lifecycle tests green); `assets/README.md` and the lab README
now point at `crates/observed_assets` as the canonical slot list. Procedural/placeholder
fallbacks unchanged (53 game + 5 crate + 2 lab tests green; `fmt`/`clippy` clean). Since
`asset_lab` was never a `game` dependency, `game -> labs/*` stays at 9; `game ->
crates/*` rose to 4. Exit criteria met: `asset_lab`, `game`, and the asset docs consume
one slot list; duplicated asset paths in game presentation are reduced to semantic slot
references; procedural fallbacks still work.

**Refactor Phase R3 - Split the game shell without behavior changes.** ✅ *(completed
2026-06-25.)* The 2204-line `game/src/screens.rs` god file was split into a `screens/`
module tree by runtime responsibility — `menu` (splash/main-menu/results + shared nav),
`loadout`, `lobby`, `match_runtime` (lifecycle + the fixed-step teleport controller),
`place` (geometry/setpieces/camera/objective/rivals), `hud` (HUD + tac-map), `audio`,
and `input` — with the root `screens.rs` keeping only shared theme/components/resources/
UI helpers plus two composition plugins (`ScreensPlugin`, `MatchPlugin`). The submodules
are descendants of `screens`, so they reach the shared scaffolding (including
`MatchAssets`'s private fields) via `use super::*`, and the root glob-re-exports each
submodule so the flat `screens::*` paths (and the lifecycle tests) are untouched. All
`OBSERVED2_CAPTURE*` screenshot systems moved out of `lib.rs` into a new `capture` module
(`lib.rs` 1039 → 471 lines), and `ObservedGamePlugin` now reads as composition:
`init_state` + `Career` + `setup_camera` + `add_plugins((ScreensPlugin, MatchPlugin))`,
with capture wired in `run`. No gameplay diff (mechanical move; exact system order + run
conditions preserved); 53/53 game tests green; `fmt`/`clippy` clean. Exit criteria met:
no meaningful gameplay diff, the lifecycle tests still pass, and `ObservedGamePlugin`
reads as top-level composition.

**Refactor Phase R4 - Make the input boundary pure.** ✅ *(completed 2026-06-25.)* The
durable `PlayerId` / `PlayerIntent` data path no longer requires Bevy: the fields now
use `glam::Vec2` (the exact type Bevy re-exports as `bevy::math::Vec2`, so the two
unify), and the `Component` derives are gated behind an optional, **default-on** `bevy`
feature — the Bevy adapter. A pure consumer (a bot, a replay tape, a network packet, a
model test) opts out with `default-features = false` and pulls **only glam**, no Bevy
ECS (`cargo tree -p player_input --no-default-features` → `glam` alone; the unit tests
pass under `--no-default-features`). Sampling/resources already live in the consuming
labs/game (themselves Bevy adapters), so nothing moved there. Default-on means every
existing consumer (all use default features, directly or via `observed_core`'s
re-export) is unchanged — the whole workspace type-checks, and the input lab
(`control_lab`, devices + record/playback), `observed_core`, and the determinism-critical
`network_lab` (lockstep hashes/replay) + `fps_controller_lab` (exact replay) all stay
green. `fmt`/`clippy` clean in both feature modes. Exit criteria met: pure consumers
depend on the input data without pulling Bevy ECS; the input labs still demonstrate
devices and record/playback.

**Refactor Phase R5 - Promote observation and door mechanics.** ✅ *(completed
2026-06-25.)* The observe/decohere model moved verbatim out of `observation_lab` into
**`crates/observed_observation`** (with its 8 model tests) and the door-gate model out
of `door_lab` into **`crates/observed_doors`** (11 tests) — the observe/freeze/decohere
and open/closed/spine/reopen-reveals-change/dead-end semantics are now tested in
production crates. Both follow the R4 pattern: `glam::Vec2` for positions, and
`observed_observation` gates its `Resource` derive behind a default-on `bevy` feature
(it compiles `--no-default-features` with no Bevy); `observed_doors` is fully Bevy-free
(the lab wraps `DoorWorld` in its own resource). `observation_lab`'s `model` module now
re-exports `observed_observation`, so its ~11 downstream consumers (`observation_lab::
model::*`) are unchanged, and the lab is the projection. `door_lab` and
`fps_visibility_lab` were repointed at the crates directly, **cutting their
`→ observation_lab` cross-lab edges** (the audit confirms both are gone from the
cross-lab map; Rule 1 holds — the new crates depend only on `crates/*`). Whole workspace
type-checks; the three labs' lifecycle tests pass; `fmt`/`clippy` clean (incl. the pure
feature mode). Exit criteria met: the semantics are tested in production crates, and the
three named labs are debug projections over them.

**Refactor Phase R6 - Promote facility topology.** ✅ *(completed 2026-06-26.)* The
authored room/port topology moved out of `room_lab` (its `model.rs` + `world.rs`) and
the spine constraint out of `constraint_lab` into **`crates/observed_facility`** —
three modules: `room_def` (templates, typed ports, surfaces, the registry, transform/
world-port/collision helpers), `room_world` (the validated world: spawn/attach/rotate/
replace/despawn, with explicit type/position/facing/occupancy checks), and
`constraints` (the protected route spine over the observe/decohere graph). Its **16
tests** (8 room+validation, 8 constraint) — including the authored-port validation
rejection cases — now run in the production crate. Follows the established pattern:
`glam::Vec2` geometry, with the `Resource` derives and the `color()` presentation
helpers behind a default-on `bevy` feature (`bevy_color` for `Color`); the crate
compiles/tests `--no-default-features` with no Bevy. `room_lab` re-exports the crate
under its `model`/`world` paths (so its `fps_facility_lab`/`fps_match_lab` consumers are
unchanged) and `constraint_lab`'s `model` re-exports `observed_facility::constraints`
(so `mutable_facility`/`fps_maze_lab`/`fps_reroute_lab`/`competitive_facility` are
unchanged) — both labs are now projections. `constraint_lab` was also repointed off
`observation_lab` onto `observed_observation`, **cutting that cross-lab edge** (the audit
confirms it's gone; Rule 1 holds — `observed_facility → crates/*` only). Whole workspace
type-checks; both labs' lifecycle tests pass; `fmt`/`clippy` clean (incl. pure mode).
Exit criteria met: room graph + constraint behavior live in the shared crate, the room
and facility labs import it, and authored ports stay explicit and validated.

**Refactor Phase R7 - Promote traversal.** ✅ *(completed 2026-06-26.)* The deterministic
first-person controller — `step_body` (the movement kernel), substep AABB collision, the
authored step-up (`classify_horizontal_contact`/`step_onto`: stairs/raised walkways climb,
tall obstacles block), `FpsBody`/`FpsConfig`/`FpsArena`/`Aabb3`/`FpsStep`/`run_path`/
`FIXED_DT` — moved out of `fps_controller_lab/src/controller.rs` into
**`crates/observed_traversal`** with its 7 determinism/movement/step-up tests. Pattern as
before: `glam::Vec3` math, pure `player_input` (`default-features = false`), and the
`Resource` derives on `FpsArena`/`FpsConfig` behind a default-on `bevy` feature (compiles/
tests `--no-default-features` with no Bevy). `fps_controller_lab`'s `controller` module
now re-exports the crate, so its 6 lab consumers (`fps_facility_lab`, `fps_maze_lab`,
`fps_reroute_lab`, `fps_elevation_lab`, `network_lab`, `fps_hybrid_match_lab`) are
unchanged and the lab is the projection. The **game was repointed onto `observed_traversal`
directly** (teleport/screens/match_runtime) and **dropped its `fps_controller_lab`
dependency — cutting a `game -> labs/*` edge (9 → 8)**. Determinism is preserved (the
`Vec3` is bit-identical glam): `network_lab` (21, lockstep hashes/replay),
`fps_hybrid_match_lab` (15, replay), and the game (53) all stay green; Rule 1 holds
(`observed_traversal -> player_input` only); `fmt`/`clippy` clean in both modes. No physics
crate was added — the project-owned AABB constraint model is unchanged. Exit criteria met:
first-person movement, elevation, replay, and lockstep users all call one production
controller path, and physics stays behind the project-owned model.

**Refactor Phase R8 - Promote interaction and equipment.** ✅ *(completed 2026-06-26.)*
The logical interaction state machine moved out of `interaction_lab` (its `model.rs` +
the pure `engine.rs`) and the persistent-equipment model out of `equipment_lab` into
**`crates/observed_interaction`** — two modules: `interaction` (players/objects/policies +
the deterministic `tick_interactions`/`prompt_for_player` engine: activate/operate,
exclusive & shared co-op holds with quorum and interruption, carry/drop/socket/recover,
climb) and `equipment` (equipment whose carried/deployed/socketed/ground/powered state is
independent of any render entity). Its **19 tests** (10 interaction logic + 9 equipment
persistence — incl. carry, deployment, room replacement, and despawned-visual stability)
now run in the production crate. Pattern as before: `glam::Vec2`, with the `Resource`
derives on `InteractionWorld`/`EquipmentWorld` behind a default-on `bevy` feature
(compiles/tests `--no-default-features` with no Bevy). Both labs re-export the crate
under their familiar `model`/`engine` paths (so `lab.rs`/`input.rs` are untouched) and
are now projections; both keep their Bevy lifecycle tests (1 + 4). Rule 1 holds
(`observed_interaction -> observed_core` only); whole workspace type-checks; `fmt`/`clippy`
clean in both modes. Exit criteria met: the interaction + equipment models live in one
shared crate the labs (and future game puzzle work) project, and equipment state stays
stable across carry, deployment, room replacement, and despawned visuals.

**Refactor Phase R9 - Promote match, session, progression, and network models.** ✅
*(completed 2026-06-26.)* The largest promotion — three crates spanning nine labs:
- **`crates/observed_match`** absorbs the whole match brain as six modules: `competition`
  (team race), `director` (collapse/absorption), `mutable` (spine objective), `maze`
  (the seeded rerouting maze), `facility` (the competitive match resolution), and
  `hybrid` (the first-person hybrid match with round boundaries + exact replay) — moved
  from `competition_lab`/`director_lab`/`mutable_facility`/`competitive_facility`/
  `fps_maze_lab`/`fps_hybrid_match_lab`, built on the promoted `observed_observation`/
  `observed_facility`/`observed_traversal`. **64 tests**.
- **`crates/observed_net`** = `protocol` + `network` (lockstep over a hostile transport)
  + `netmatch` (the host-authoritative networked match), from `network_lab` +
  `net_match_lab`. **23 tests**.
- **`crates/observed_progression`** = `progression` (career/profile/cosmetics) + `session`
  (matchmaking/session formation), from `progression_lab` + `session_lab`. **22 tests**.

All follow the established pattern: `glam` math, `Resource` derives behind a default-on
`bevy` feature, compiling/tested `--no-default-features` with no Bevy. The nine source
labs re-export their model and are now projections (cross-cutting tests like
progression's orthogonality check stay in the lab). **Every `game -> labs/*` edge is
gone** — the game was repointed onto the crates, so `cargo metadata` shows
`game -> labs/* : 0` and `game -> crates/* : 8`. Rule 1 holds (no `crates/* -> labs/*`);
`net_match_lab`/`network_lab` also dropped their cross-lab edges onto the crates. Whole
workspace builds; **490 workspace tests pass, 0 fail** (determinism — lockstep hashes,
replay — preserved); `fmt`/`clippy` clean in both feature modes. Exit criteria met: team
race, director pressure, hybrid round boundaries, lockstep protocol, session formation,
replay tapes, and career/profile data are reusable crates with pure tests and Bevy
adapters only behind the feature.

**Refactor Phase R10 - Cut `game -> labs/*`.** ✅ *(achieved as a cascade of R9,
2026-06-26.)* Repointing the game onto the promoted crates in R1–R9 removed its last
local lab dependencies: `cargo metadata --no-deps` shows **`observed_game -> labs/* :
0`** (it depends only on the eight `crates/*` plus Bevy). Every affected lab still
launches/builds (490 workspace tests green), the game's `OBSERVED2_CAPTURE*` hooks are
intact (`capture.rs` now drives the crates), and `fmt`/clippy/test runs are green. The
remaining lab→lab edges (labs reusing other labs' re-exports) are a lab-only concern,
outside the `game -> labs/*` exit criterion. *(A follow-up could prune those re-export
chains so each lab imports the crate directly, but it is not required by R10.)*

**Refactor Phase R11 - Evaluate Bevy ecosystem replacements.** ✅ *(completed 2026-06-26 —
evaluation [docs/refactor_r11_evaluation.md](docs/refactor_r11_evaluation.md).)* With the
graph clean, the six candidate areas from [architecture.md](architecture.md) (asset
loading, debug inspection, vector shapes, input mapping, persistence, network transport)
were evaluated against the project's dependency policy. Outcome: **one dependency accepted,
five deferred.** The live ECS inspector **`bevy-inspector-egui`** is adopted behind a
**default-off `dev_tools`** feature in the new [`labs/inspector_lab`](labs/inspector_lab),
with the full policy treatment — a `DevToolsPlugin` adapter (the single seam), a guard test
for the fallback path, and a working custom-overlay fallback. Compatibility was **verified
by building**: `bevy-inspector-egui 0.36.0` + `bevy_egui 0.39.1` compile against the pinned
**Bevy 0.18.1** (`0.36.0` requires `bevy ^0.18.0`), so the evaluated `0.18.1 → 0.19`
**upgrade was rejected as unnecessary** (and would be a large breaking change for no
benefit). `bevy_screen_diagnostics` is 0.16-stuck and unneeded — Bevy's built-in
`FrameTimeDiagnosticsPlugin` covers FPS. The dependency is feature-gated off, so the
**default workspace build/test pulls no egui** and stays 490-tests green; `cargo run -p
inspector_lab --features dev_tools` launches the inspector. The other five areas are
deferred because the problem they solve does not exist in the codebase today (synchronous
asset manifest, Node/gizmo tac-map, hand-rolled input behind the pure `PlayerIntent` seam,
string-serializable progression, in-process lockstep), each with a recorded *adopt-when*
trigger + fallback. Exit criteria met: the accepted dependency has a small adapter,
verified `0.18.1` compatibility, a proving lab, and a fallback story; `fmt`/`clippy` clean
in both feature modes.

**The architecture refactor arc (R0–R11) is complete.** The dependency graph now expresses
the intended architecture: `game -> crates/* -> pure domain crates -> observed_core /
player_input`; fourteen production crates hold the reusable behavior; the labs are debug
projections; `observed_game` has **zero `labs/*` dependencies**; and the policy for growing
the dependency set is recorded. The 490-test workspace is green and `fmt`/`clippy` clean.

### Asset-integration arc (complete)

A separate, sequenced plan — [docs/bevy_asset_integration_roadmap.md](docs/bevy_asset_integration_roadmap.md)
(from [docs/bevy_assets_research.md](docs/bevy_assets_research.md)) — evaluated the ten best
Bevy ecosystem assets, each in its own isolated lab that had to preserve the project rules
(stable IDs, input/sim/presentation separation, reset safety, tests, evidence) before any
could graduate to production. **The arc is complete: ten candidates evaluated, eight proven
in isolated labs, and the A9 integration decision taken — zero promoted into the game now,
each banked lab-local behind a concrete adopt-when trigger.**

- **Phase A0 — Compatibility & lab template.** ✅ *(2026-06-26.)* `bevy_trenchbroom 0.13.0`
  (+ `bevy_materialize 0.10`, feature `client` only) verified to compile and run against the
  pinned **Bevy 0.18.1**; dependency isolated to the new lab.
- **Phase A1 — 3D map authoring.** ✅ *(2026-06-26 — [`labs/trenchbroom_lab`](labs/trenchbroom_lab/README.md),
  evidence [png](docs/evidence/trenchbroom_lab.png).)* An authored TrenchBroom `.map` is imported
  by `bevy_trenchbroom` **as a parser only** and projected into `RoomId`/`PortId`s, door state, and
  collision — editor entities never become the game model. Lab-local until a second consumer needs it.
- **Phase A2 — 2D schematic authoring fallback.** ✅ *(2026-06-26 —
  [`labs/ldtk_schematic_lab`](labs/ldtk_schematic_lab/README.md), evidence
  [png](docs/evidence/ldtk_schematic_lab.png).)* `bevy_ecs_ldtk 0.14` imports the same two-room
  topology into graph metadata + tactical-map symbols; kept **lab-local and live as a fallback**
  for design-time schematics, complementing (not replacing) TrenchBroom.
- **Phase A3 — Legibility overlay.** ✅ *(2026-06-26 —
  [`labs/outline_legibility_lab`](labs/outline_legibility_lab/README.md), evidence
  [png](docs/evidence/outline_legibility_lab.png).)* `bevy_mod_outline 0.12.1` makes every
  gameplay-critical signal punch through fog/bloom with colour/width from `observed_style::outline`.
  `bevy_color_blindness` **rejected** (Bevy 0.8 only); color-vision stays in pure style matrices.
- **Phase A4 — Semantic VFX.** ✅ *(2026-06-26 — [`labs/semantic_vfx_lab`](labs/semantic_vfx_lab/README.md),
  evidence [png](docs/evidence/semantic_vfx_lab.png).)* `bevy_hanabi 0.18` particles as deterministic,
  toggle-off, style-driven event projections that never hide gameplay signals. Lab-local.
- **Phase A5 — Evidence capture pipeline.** ✅ *(2026-06-27 —
  [`labs/capture_pipeline_lab`](labs/capture_pipeline_lab/README.md), evidence
  `docs/evidence/capture_pipeline_lab/`.)* `bevy_image_export 0.16` writes a deterministic still +
  six-frame sequence offscreen without perturbing fixed-step timing. Lab-local (`0.17` is Bevy 0.19).
- **Phase A6 — Controller & local multiplayer input.** ✅ *(2026-06-27 —
  [`labs/archie_input_lab`](labs/archie_input_lab/README.md), evidence
  [png](docs/evidence/archie_input_lab.png).)* `bevy_archie 0.2.4` feeds four local players into the
  same `PlayerIntent` through a pure per-device adapter (its global `ActionState` **not** adopted;
  MSRV bumped the toolchain to 1.96). Lab-local until controller support is committed.
- **Phase A7 — Lab config & event trace.** ✅ *(2026-06-27 —
  [`labs/lab_observability_lab`](labs/lab_observability_lab/README.md), evidence
  [png](docs/evidence/lab_observability_lab.png).)* `bevy_mod_config 0.6.2` (typed config + JSON,
  no egui) kept strictly separate from the deterministic launch manifest; `bevy_log_events`
  **rejected** (force-pulls `bevy_egui`), event tracing done lab-locally over Bevy `tracing`.
- **Phase A8 — Navigation probe.** ✅ *(2026-06-27 —
  [`labs/navigation_probe_lab`](labs/navigation_probe_lab/README.md), evidence
  [png](docs/evidence/navigation_probe_lab.png).)* `vleue_navigator 0.15` builds a navmesh as a
  **derived consumer** of facility geometry (closed doors become obstacles; cross-checked against the
  graph over all 16 door configs × every room pair). Lab-local until bots need physical routing.
- **Phase A9 — Game integration decision.** ✅ *(2026-06-27 — decision
  [docs/asset_a9_decision.md](docs/asset_a9_decision.md).)* The triage of A1–A8 for dependency cost,
  maintenance risk, and gameplay value: **zero promotions into `game` now; all eight stay lab-local,
  each with a concrete adopt-when trigger.** None clears the "concrete game use today" bar, and
  `bevy_trenchbroom` conflicts with the deliberately procedural geometry (the teleport-hallway pivot).
  Strongest standing claim — and **queued first promotion** when the paused legibility arc resumes —
  is `bevy_mod_outline`, behind an `observed_style::outline`-driven presentation adapter. No production
  crate or `game` gained an asset dependency; the `game -> crates/*` split and zero `game -> labs/*`
  edges are intact. This mirrors every lab's own promotion decision and the R11 precedent (accept only
  what solves a real current problem).

The FPS and Hybrid maze arcs are complete, and every roadmapped feasibility system
— including the final cross-cutting one, progression/cosmetics (Phase 18) — now has
an isolated lab. The remaining work is integration into a single shippable build and
production hardening; no new feasibility unknowns remain on the roadmap.

The **integration is delivered**: the [`game`](game/README.md) crate assembles the
proven systems into one cohesive, UX-first player loop — Splash → Main Menu → Loadout
→ Lobby → Match → Results — with a persistent career and strict state-scoped cleanup
(see below). It reuses `progression_lab` (career/profile) and `session_lab` (lobby)
wholesale, and its **Match is the live, first-person 3D, networked hybrid match**
(Phase 28): you walk the concrete maze in first person (the proven
`fps_hybrid_match_lab` controller + presentation), and each round you cross is
replicated to a remote peer over `network_lab`'s lockstep on a hostile transport.
The only new code is the state machine, the per-round action wire format, the
live host→replica session, and presentation. What remains is production hardening
(real sockets/relays, accounts), not feasibility.

### Integrated game (first milestone delivered)

**Cohesive whole — the assembled game.** ✅ *(completed 2026-06-20 —
[`game`](game/README.md), evidence [png](docs/evidence/game.png).)* A top-level
Bevy state machine (`GameState`: Splash / MainMenu / Loadout / Lobby / Match /
Results) strings the proven systems into the loop a player moves through, with the
emphasis on UX: one visual theme, keyboard navigation shared across every menu, a
live career banner, an in-match tac-map HUD, and an in-match pause. A persistent
`Career` resource (wrapping `progression_lab`'s `Profile`) survives every match and
awards each result exactly once; the lobby forms a real balanced session via
`session_lab`'s matchmaker; the match is `competitive_facility`'s brain stepped on
screen and resolved into a `MatchResult`. Every screen's entities carry
`DespawnOnExit`, so exactly one screen is alive at a time and transitions never leak
(a test cycles the whole loop five times asserting this). Orthogonality is
re-asserted at the integrated level: the match takes no profile, so a maxed,
fully-equipped career resolves the match identically. 5 flow + 4 lifecycle tests.

**Phase 28 — Networked first-person match.** ✅ *(completed 2026-06-20 —
[`labs/net_match_lab`](labs/net_match_lab/README.md), evidence
[png](docs/evidence/net_match_lab.png); wired into the game, evidence
[png](docs/evidence/game_match.png).)* The integration of two proven results:
`network_lab`'s deterministic lockstep over a hostile datagram transport and
`fps_hybrid_match_lab`'s deterministic, replayable hybrid match. Both peers run the
same `HybridMatch` and exchange the local team's per-round action over the hostile
transport; the local team's two members are modelled as two peers owning alternate
rounds, and a peer commits a round only once it holds the authoritative action
(its own, or the *received* teammate's) — so advancing genuinely needs the network.
Reliable resend/ack then converges both peers on the **identical match, maze, and
first-person pose, round-for-round**, equal to the single-player tape, despite real
loss / delay / duplication / reordering. A clean and a hostile network land on the
identical final state: the transport replicates, it does not alter. Only the
per-round `ActionPacket` and the reliable action peer are new — the transport
(`SimulatedNetwork`, now with a public `step`) and the match brain are reused
wholesale. A `LiveNetMatch` variant adds host-authoritative live play: the host
plays in first person and each resolved round replicates to a remote replica, which
stays bit-exact because every resolved round ends in a canonical pose. This is the
match the assembled **game now plays in first-person 3D** (evidence
[png](docs/evidence/game_match.png)). 12 model + 4 lifecycle tests.

### Gameplay-depth arc (paused behind architecture refactor)

Post-integration polish driven by playtest feedback: a more readable, interesting
facility and more depth. This remains important, but new gameplay-depth work waits
until the architecture refactor removes production dependence on lab crates.

- **Bigger deliberate facility.** ✅ The maze generator (`fps_maze_lab`) was enlarged
  (plots 11→15) with bigger, distinct rooms, and `fps_hybrid_match_lab` now carves a
  **wide gold spine hall** vs narrow side passages so "where do I go" reads at a
  glance; the game lights the spine floor. Determinism re-proven across the maze /
  hybrid / net / reroute / game labs.
- **Drop-in assets, placed deliberately.** ✅ The dropped CC0 assets (textures, models,
  sounds) are wired into the game; placement was reworked so props frame rooms (decor
  on walls, equipment as the control-room centrepiece, one ceiling lamp per room) and
  the broken HDRI skybox was removed (needs a `.ktx2` cubemap, not a raw `.hdr`).
- **Elevation.** ✅ *(feasibility — [`labs/fps_elevation_lab`](labs/fps_elevation_lab/README.md),
  evidence [png](docs/evidence/fps_elevation_lab.png).)* A deterministic **step-up**
  controller traverses real height — stairs, raised platforms, ledges — proven on an
  authored multi-level course (climb stairs, blocked by too-tall walls, fall off
  edges, identical path from identical inputs). It was built as an isolated
  controller variant; the validated step-up rules are now promoted into shared
  `step_body` by the multi-level maze integration. 4 model + 3 lifecycle tests.
- **Multi-level generated maze.** ✅ *(integrated 2026-06-20.)* The proven step-up
  is promoted into the shared fixed-step controller. `fps_maze_lab` now generates
  three flat room levels (0.0 / 0.9 / 1.8 m) joined by deterministic 0.3 m stair
  bands; `fps_reroute_lab` preserves the height field while corridor occupancy
  swaps atomically; `fps_hybrid_match_lab` includes elevation in exact replay and
  lockstep snapshots; and the assembled game renders matching raised floors,
  supports, walls, ceilings, fixtures, props, avatars, and hazards. Tests drive the
  real controller from the lowest to highest generated room band and re-prove
  reroute, replay, networking, and game lifecycle behavior.
- **Pressure traps + safe/risky routes.** ✅ *(integrated 2026-06-20.)* Every
  protected-spine leg now reserves enough generated space for two readable choices:
  a short red pressure-gate route and a longer cyan bypass. Gates pulse on a fixed,
  deterministic clock. Crossing while active returns the player to the current-room
  checkpoint and adds a short movement lock, but never changes earned match
  progress or health; waiting for the idle window makes the shortcut viable. The
  canonical replay path takes the safe bypass, while snapshots/network peers carry
  the exact safe/trap tile layout. The assembled game renders pulsing gate emitters
  and lights. A committed decoherence now produces an explicit first-person route-
  shift flash, camera jolt, and the existing mechanical audio cue.
- **Next:** playtest trap timing and route readability, then add a second authored
  hazard vocabulary only if the pressure-gate choice produces meaningful decisions.

### Legibility & visual-language arc (paused behind architecture refactor)

A 2026-06-21 playtest showed the assembled game's **presentation** is the weak
link, not the simulation: objective/threat markers are bare debug gizmos, rooms are
undifferentiated, floor colours flash without a legend, the proven tac-map was never
wired in, and bots teleport. See the **North Star** in [agents.md](agents.md): the
fix is integration plus a *code-as-art* visual language under the **Legibility
Contract**, not new feasibility work. Art direction: **neon-noir** (procedural; no
asset pipeline). The completed legibility work should now be preserved by extracting
`observed_style`; remaining visual polish resumes after the refactor.

**The keystone — `style_lab` + a pure `style` module.** ✅ *(completed 2026-06-21 —
[`labs/style_lab`](labs/style_lab/README.md), evidence
[png](docs/evidence/style_lab.png).)* A shared, unit-tested module mapping *semantic
state → visual treatment* (`SurfaceRole`, `MarkerRole`, `ObservedState` →
base/emissive colour, a "signal tier" that stays legible through fog/bloom, optional
neon edge). Reused as a library by `game` and the 3D labs so presentation never
invents ad-hoc colours. Tests prove the rules: every role maps to a distinct
documented treatment; every signal-tier treatment clears a minimum emissive
luminance (the Legibility Contract); atmosphere surfaces stay dark; an armed trap
stays legible even when unobserved; and `legend()` enumerates every role uniquely. A
neon-noir reference showcase renders every role with its legend (HDR + bloom + fog,
via the `bevy_post_process` feature) and captures a screenshot. 9 pure-logic + 2
lifecycle tests.

**Milestone — legibility re-skin of the match** (each step small, testable, reuses
the module):

1. ✅ `style_lab` + the `style` module (the keystone above).
2. ✅ Re-skin `game`'s match in neon-noir via the module (surfaces from
   `style::surface`, no drop-in textures); dropped the magenta placeholder for a
   quiet steel-blue fallback; added HDR + bloom (intensity 0.08) + distance fog and
   a dim match-only ambient, scoped to the Match state. Evidence
   [png](docs/evidence/game_match.png).
3. Diegetic presentation. ✅ *(completed 2026-06-21, evidence
   [png](docs/evidence/game_match.png).)* Procedural **neon doorways** replace the
   free-standing doorway GLB — a framed opening (posts + lintel) with a leaf that is
   closed by default (hides the corridor → mystery) and slides up as the player
   nears; reroutes respawn the leaves closed (the slam / re-hide); spine doorways glow
   gold. The scattered decor crates/consoles were removed. The ambiguous **gizmo
   marker lines are gone**: the objective is now a diegetic gold **next-room beacon**
   (a pulsing beam over `local_target`, from `style::marker(NextRoom)`); the exit
   (gate + light), collapse (hazard beacons), and rivals (avatars) were already
   diegetic. An on-screen **legend** explains every colour (next room / exit /
   collapse / rivals + spine / safe / gate floors) from the shared palette, so the
   red-line ambiguity is resolved and no on-screen colour is a mystery.
4. `fps_match_lab`'s Tab tac-map wired into `game`, rendered through the module.
5. Walking rival avatars. ✅ *(completed 2026-06-26, evidence
   [png](docs/evidence/game_rivals.png).)* The teleport pivot had **removed** rival
   avatars entirely (the old whole-maze view's "teleporting bots" were gone, not fixed),
   so this re-introduces them where they can be seen: a new pure `game/src/rivals.rs`
   reads the brain (`team_room` / `active_runner`) to find which rival teams share the
   player's current room, and `screens::sync_rival_avatars` renders each as a figure
   (`style::marker(Rival)` orange, matching the legend) that **walks** the room's exit
   axis on a seamless triangle-wave pace (per-team phase + lateral lane, a small bob) and
   despawns when its team moves on. Presentation-only — it never writes match state, so
   determinism / replay / lockstep are untouched. Every team starts clumped at the
   entrance, so you see all three rivals at the start, then they fall behind the
   (fastest) local team. 4 rivals unit tests + 1 game lifecycle test (avatars appear in
   the shared room and never leak past the Match).

**Teleport-hallway pivot (in progress, 2026-06-22).** The game's Match moved off the
shared hybrid maze to a **teleport model**: the player occupies one discrete place at
a time — a room box or an authored hallway piece (`game/src/hallway.rs`,
`game/src/teleport.rs`) — and crossing a doorway teleports to the next, with the match
*brain* (rounds/networking/replay) untouched and driven via `force_round` on a
spine-room arrival. This decouples each edge from one grid (so hallways become
pre-generated variations), is a purer expression of "changes when unobserved," and
ends the doorway/hall alignment problem. Stages: S1 authored hallway library + S2
place state machine + collision bridge + S3/S4 in-game swap are **done and green**
(24 game tests; evidence [png](docs/evidence/game_match.png)). MVP is spine-only
(forward doorway functional, side doorways decorative; Seize/reroute-flash deferred)
and not yet live-playtested end-to-end. See [agents.md](agents.md) nodes/edges canon.
Walking rival avatars (step 5) have now re-entered the teleport model (see above);
the Tab tac-map (step 4) is also wired into the teleport model.

**Labyrinthine hallways (2026-06-25).** ✅ The earlier corner attempt (a single
`Dogleg` flavour) did not read as a labyrinth, so a *significant portion* of hallway
pieces are now real generated **mazes**. A new pure module (`game/src/maze.rs`)
generates a deterministic, seeded grid maze per hallway — a randomized-DFS spanning
tree (every cell reachable, so the single entrance and exit are always connected,
with long winding corridors and many dead ends) plus a light braid pass that opens
some dead ends into loops (alternate routes) while structurally keeping at least one
dead end. Four of the nine hallway templates are `Maze` flavours (4×4 → 6×7). The
maze walls flow through the existing `PlaceGeom.interior` → `place_arena` →
arena-solids renderer (so collision and presentation come for free); the layout is a
pure function of the stored `Place::Hallway`, so it can't reshuffle underfoot but
still re-rolls when an edge decoheres. Backtracking out a maze's entrance now returns
to the prior room (so wandering dead ends never walks into the void). 6 maze + 2
new teleport tests (incl. a body-radius collision flood proving entry→exit is
walkable); whole workspace green. Evidence
[png](docs/evidence/game_maze_hallway.png).

**Varied geometry pass (2026-06-25).** ✅ Playtest follow-ups for a richer facility:
(1) **multiple entrances/exits** per maze — `maze.rs` now picks 1–3 spread-out door
columns on each of the entry/exit walls (all mutually connected), and `place_arena`'s
perimeter split was generalised to any number of openings per wall (`wall_spans`); the
controller already crosses/​backtracks through any of them. (2) **Varied straight-hall
lengths** — `hallway::length_scale` jitters each straight/​dogleg/​climb run to
0.55×–2.2× per edge, so repeated connectors read as distinct. (3) **Angled polygon
rooms** — rooms are now seeded convex polygons (varied rectangles + regular 5–8 gons,
`room_geom`/`room_polygon`) instead of squares. Since the shared AABB controller can't
represent angled walls, polygon rooms carry no wall solids; a convex containment clamp
(`teleport::contain`, applied after `step_body`) is the room collision, open at the
doorways, and the walls render as rotated edge panels with a custom triangle-fan
floor/​ceiling. The doorway frame/​stub/​leaf renderers are now rotation-aware (work at
any wall angle). New tests cover multi-door connectivity, length variation, polygon
shape/​door spread, and convex containment; workspace green. Evidence
[polygon room png](docs/evidence/game_polygon_room.png),
[multi-door maze png](docs/evidence/game_maze_hallway.png).

**Then — the fun pillars** (each its own integration step with tests + evidence):
exploration (dial back the gold-spine hand-holding so the maze is unknown again),
traversal (fold in `climbing_lab` + carryable equipment), and puzzle/co-op
(`route_lab` cables + `hazard_lab` two-operator relief).

**Room types & the gated objective (Betrayal-style discovery).** 🔜 The facility's
rooms become **typed** with a purpose, and the exit is **gated**: it stays locked until
the team has discovered and gathered what it needs (keystones / power / a route). Room
*types are hidden until discovered* (you only learn what a room is by reaching it) and
*shift when unobserved* — so the room you remember as a vault may be a dead end when you
look away (the "Betrayal" turn). The plan is ~10 useful room types known but shifting;
start with a **core 5**: **Power Cache** (provides power), **Keystone Vault** (provides a
keystone — the key item the gate needs), **Control** (acts on the facility — e.g. lock a
room's type / reveal), **Survey** (reveals types at range), and **Dead-end** (a bust).

- **Feasibility first — `labs/discovery_lab`.** ✅ *(completed 2026-06-25 —
  [`labs/discovery_lab`](labs/discovery_lab/README.md), evidence
  [png](docs/evidence/discovery_lab.png).)* An isolated, pure-logic lab proving one
  question: a gated exit unlocks only once the required **keystones (+power)** are
  discovered and collected from typed rooms whose types are **hidden until visited** and
  **shift when unobserved**, while a **solvability constraint** (only shift types among
  *unharvested* rooms, so a keystone can never strand on a spent room — the analogue of
  `constraint_lab`'s spine) guarantees the objective is *never* made impossible. The
  decohere conserves the type multiset (vaults relocate, never vanish); a debug schematic
  shows each room's known/unknown type, collected keystones/power, the gate state, and a
  live `still solvable` readout. 9 model + 3 lifecycle tests prove: gate logic, discovery,
  observed-room freezing, multiset conservation, determinism, *always solvable with the
  constraint*, *can strand without it*, and Control/Survey effects.

- **Vocabulary expansion — core 5 → 8 (2026-06-25).** ✅ The lab's type vocabulary grew
  by three, each with its own behavioural question and all preserving the solvability
  invariant: **Reactor** (yields **2** power, making the power economy *yield-based*, not a
  count), **Sensor** (reveals types *at range* — only the 4-neighbour rooms, vs Survey's
  facility-wide reveal), and **Decoy** (the deepest Betrayal: *displays* as a Keystone
  Vault when revealed remotely but yields nothing on a visit — a new `displayed_type`
  split, and it is **never** counted as a real keystone, so deception misleads the player
  yet can never strand the run). The schematic colours/glyphs and legend were extended; the
  tight 2-keystone gate keeps the constraint's value crisp. 12 model + 3 lifecycle tests
  (added: reactor yield + summed collectable power, sensor reveals only neighbours, decoy
  lies-on-reveal/yields-nothing/never-a-keystone). Evidence
  [png](docs/evidence/discovery_lab.png). **Next:** the remaining candidates toward ~10
  (Anchor — pin one room; Trap — scramble memory; Relay — calm the shifting) can be added
  the same way, then fold the gated exit into the teleport facility (reusing
  `competitive_facility` + `equipment_lab` + `incentive_lab`).

- **Game wire-in — keystone-gated exit (2026-06-25).** ✅ The gated exit is now in the
  game's Match, kept deliberately simple per playtest feedback: **keystones are pickup
  items; the gate is a plain inventory check** (no room-type discovery / visit tracking in
  the game — that stays in the lab). `game/src/keystones.rs` places `REQUIRED=3` keystone
  items deterministically in spine rooms (always incl. the last room before the exit, so
  it's attainable on the way through); walking over one collects it (`keystone_pickup`
  proximity system). The exit door **physically locks**: `teleport::GapKind::LockedExit` +
  `Nav.exit_locked` make a hallway into `EXIT_ROOM` render a solid red door (walled by
  `place_arena`, no void-walk) until `held >= required`, at which point it becomes a normal
  crossable `Exit` and the brain receives the final `Advance` (the match brain / networking
  / determinism are untouched — the gate only withholds the local crossing). HUD shows
  `keystones X / 3` + `EXIT LOCKED/OPEN`; legend updated. 6 keystone + 1 LockedExit + 1
  gate-lifecycle test; workspace green. Evidence [png](docs/evidence/game_keystone.png).
  **Deferred:** making the gate *bite* via off-path/shifting placement, larger
  requirements, and the other room-type effects.

**Design canon — nodes vs edges (adopted 2026-06-21).** Rooms = decide / observe /
co-op; corridors = traverse / danger / mystery; puzzles in rooms, twitch-dangers in
corridors (see the North Star in [agents.md](agents.md)). Doors are the threshold
and the diegetic face of observe/decohere.

**Doors as a mechanic — `door_lab` (feasibility).** ✅ *(completed 2026-06-21 —
[`labs/door_lab`](labs/door_lab/README.md), evidence
[png](docs/evidence/door_lab.png).)* An isolated lab proves the mechanic on the
proven observation graph (reusing `observation_lab`'s structure, with a new
door-driven pinning rule): a player-operated door gates freezing (open =
observed/frozen; closed = free to rewire); rewiring happens *only* behind closed
doors; a protected spine keeps the exit reachable through any rewiring; reopening a
closed door can reveal a changed partner (the "path changed" loop); traversal
requires an open door; and dead-end pockets are detected and never sever the exit —
all deterministic. 11 pure-logic + 2 lifecycle tests. The in-game doorway
presentation (step 3 of the re-skin) folds in next: closed leaves hide the layout,
opening freezes a route, a slam signals a reroute.

### FPS arc (completed)

**Phase 19 — FPS observation.** ✅ *(completed 2026-06-19 —
[`labs/fps_observation_lab`](labs/fps_observation_lab/README.md), evidence
[png](docs/evidence/fps_observation_lab.png).)* Drives the observed set from the
first-person camera's line of sight over `observation_lab`'s graph: the rooms you
see freeze their connections, the rest rewire, and looking through a doorway follows
its current link (so you can see and freeze a far room). The only new logic is a
pure line-of-sight function; it feeds the proven, deterministic graph wholesale.
First 3D lab (Bevy `["3d", "png"]`). 7 vision + 3 lifecycle tests.

**Phase 20 — Deterministic first-person controller.** ✅ *(completed 2026-06-19 —
[`labs/fps_controller_lab`](labs/fps_controller_lab/README.md), evidence
[png](docs/evidence/fps_controller_lab.png).)* A 3D first-person controller (look,
facing-relative movement, sprint, jump) on the shared `PlayerIntent` boundary, the
3D analogue of `movement_lab`: a pure `step_body` advanced at a **fixed timestep**
with substep integration and axis-by-axis AABB collision (floor, walls, pillars).
Because the step depends only on its inputs, the same intent sequence yields an
identical path — exercised both by a determinism unit test and by the lab's live
record→replay (a recorded run retraces exactly, `MATCH ✓`). Decision on the
"no third-party physics" rule: hand-rolled AABB collision was sufficient for this
feasibility lab, so no physics crate was added. Phase 23 subsequently reused it
successfully for authored 3D module geometry and graph doorways. 6 controller + 3
lifecycle tests.

**Phase 21 — Continuous line-of-sight observation.** ✅ *(completed 2026-06-19 —
[`labs/fps_visibility_lab`](labs/fps_visibility_lab/README.md), evidence
[png](docs/evidence/fps_visibility_lab.png).)* Promotes Phase 19's room-granular
visibility to a deterministic field of stable 5 by 5 sub-room cells. Every cell
passes range, frustum, and authored wall-segment occlusion tests, so a doorway can
reveal only a slice of the next room. "Freeze what you see" is defined at doorway
endpoint granularity: a directly visible endpoint freezes its graph connection,
while other connections in a partially seen room remain free. Camera stepping uses
the shared `PlayerIntent` boundary at a fixed timestep; replaying the same intent
sequence reproduces identical poses and visible sets. Seeded decoherence re-pairs
only connections whose endpoints are unseen. 10 pure-logic + 3 lifecycle tests.

**Phase 22 — Rewire-while-unobserved rendering.** ✅ *(completed 2026-06-19 —
[`labs/fps_rewire_lab`](labs/fps_rewire_lab/README.md), evidence
[png](docs/evidence/fps_rewire_lab.png).)* Implements the out-of-view-only strategy
over Phase 21 visibility. Decoherence proposes a deterministic permutation of
hidden portal modules; replacement is one atomic batch that rechecks every affected
doorway aperture is still outside the frustum and clear of traversal. Only then
does presentation replace the actual 3D module entities. Traversal captures the
currently rendered destination and blocks a touching batch until arrival, so the
route cannot disappear beneath the player. The turn-away → commit → turn-back test
reveals changed geometry with zero visible commits; lifecycle coverage also proves
a visible module keeps its entity identity and a hidden one is replaced. 9
pure-logic + 3 lifecycle tests.

**Phase 23 — 3D facility from the room graph.** ✅ *(completed 2026-06-19 —
[`labs/fps_facility_lab`](labs/fps_facility_lab/README.md), evidence
[png](docs/evidence/fps_facility_lab.png).)* Promotes all eight `room_lab`
templates into authored 3D definitions with bounds, rotated collision solids, four
graph-facing Passage ports, and the complete Door/Ladder/Machinery/Equipment/
Grapple/Observation fixture vocabulary. All nine stable graph rooms render as
module instances. A strict projection maps each of `observation_lab`'s 36
`DoorId`s uniquely to a transformed Passage port and every open partner pair to one
rendered connection; graph-sealed ports create real collision panels. The Phase 20
fixed-timestep controller walks through actual doorway gaps, is stopped by sealed
panels, and on threshold crossing follows the current graph partner—even after
decoherence redirects it to a non-adjacent room. Projection, collision, panels, and
presentation rebuild from the same graph state. 12 pure-logic + 3 lifecycle tests.

**Phase 24 — First-person competitive match.** ✅ *(completed 2026-06-20 —
[`labs/fps_match_lab`](labs/fps_match_lab/README.md), evidence
[png](docs/evidence/fps_match_lab.png).)* The full match (`competitive_facility`
brain + director AI) played in first person over `fps_facility_lab`'s 3D facility,
with the `match_replay` schematic promoted to an in-3D tac-map / spectator. The
integration boundary is one local round action: `Advance` is emitted only when the
player physically crosses the highlighted protected-spine Passage (following the
current graph partner), and `Seize` is gated to the control-room console; bots and
the director are deterministic, so the 3D facility re-synchronizes from the exact
competitive graph after each round. A `MatchTape` of local actions reconstructs both
the match *and* the first-person pose from a fresh session, so the recorded match
replays — result and camera alike — bit-for-bit (`replay check MATCH`, `graph
projection exact`). Reuses `competitive_facility`, `fps_facility_lab`,
`match_replay`'s tape approach, and the Phase 20 controller. 8 model + 4 lifecycle
tests. This completes the FPS arc.

### Hybrid maze arc (completed)

The FPS arc proved the mechanic over a deliberate **scaffold**: nine rooms on a
fixed grid, connected by portal doorways (you cross a doorway and you are in its
current graph partner, with empty space — no hallways — between modules). That made
the coherence logic testable but leaves it reading as *symbolic*. The agreed next
direction is to make it **concrete**: a **hybrid maze** — the proven room graph
stays the topology, but its connections become **actual walkable passages** laid out
in space by a maze generator, and those passages **re-route when unobserved** (the
graph edges rewire) using the proven off-camera swap. Rooms are dynamically spaced
and distributed; the labyrinth is real and it shifts behind you.

This deliberately supersedes the fixed-grid portal embedding for the real game. The
simulation brain (graph, observation, spine, competition, director, replay) is
unchanged — this arc is about *embedding the graph in navigable space and making the
embedding dynamic*.

**Phase 25 — Spatial maze layout.** ✅ *(completed 2026-06-20 —
[`labs/fps_maze_lab`](labs/fps_maze_lab/README.md), evidence
[png](docs/evidence/fps_maze_lab.png).)* A deterministic (seeded) generator embeds
the proven room graph in a tile maze: the nine rooms are placed with seeded
size/position, and every open graph connection is routed (BFS through non-room
space) as a real walkable corridor — no portal teleports. The result is connected
and navigable (BFS reaches all nine rooms on foot), rooms never overlap, generation
is deterministic per graph + seed and varies across seeds, and a **decohered** graph
(non-adjacent connections) still embeds navigably — the generality Phase 26 needs.
The protected spine is routed and highlighted. First-person walk with tile
collision + a top-down map. 6 generator + 2 lifecycle tests. *Scope note:* this is
the static embedding; corridors realize the graph's connections and may share
junction tiles, so navigable connectivity is a superset of the graph edges — strict
edge-faithful routing and rerouting are Phase 26.

**Phase 26 — Rerouting passages.** ✅ *(completed 2026-06-20 —
[`labs/fps_reroute_lab`](labs/fps_reroute_lab/README.md), evidence
[png](docs/evidence/fps_reroute_lab.png).)* Makes the Phase 25 maze live: when an
unobserved region decoheres, the affected corridors re-route in space, but the
spatial change is committed as **one atomic swap that only happens off-camera and
never under the player's feet** (Phase 22's discipline). The model keeps a
`rendered` layout and a `target` layout; each decohere updates `target` and
`try_commit` reconciles to it only when every changing tile is unseen and player-
clear. After an off-camera commit a corridor leads to a different room; the room you
stand in stays observed/frozen; every reroute leaves a navigable maze; deterministic.
Reuses `fps_maze_lab` (`place_rooms`/`route_corridor`) and the off-camera-swap
discipline of `fps_rewire_lab`. 8 model + 3 lifecycle tests.

**Phase 27 — First-person hybrid match.** ✅ *(completed 2026-06-20 —
[`labs/fps_hybrid_match_lab`](labs/fps_hybrid_match_lab/README.md), evidence
[png](docs/evidence/fps_hybrid_match_lab.png).)* Integrates the Phase 24
competitive brain with the generated, rerouting maze. The local player uses the
Phase 20 fixed-step controller against tile-derived collision and emits `Advance`
only after physically entering the next protected-spine room. Each authoritative
graph change builds a target maze; the rendered/collision maze switches to it only
as one atomic off-camera, player-clear commit, preserving Phase 26's safety
contract. Bots, shared control, capacity-limited exits, collapse, absorption, and
director actions still resolve deterministically. The action tape reconstructs
team state, graph links, rendered and target corridor routes, committed tiles, and
the canonical first-person pose exactly. 11 model + 4 lifecycle/integration tests.
This completes the Hybrid maze arc.

### Carried-forward systems

**Phase 15 — Cooperative megastructure hazards.** ✅ *(completed 2026-06-19 —
[`labs/hazard_lab`](labs/hazard_lab/README.md), evidence
[png](docs/evidence/hazard_lab.png).)* A deterministic three-zone pressure front
can be steered by the facility director before each round. Four players across
two teams stage independent intents; containment requires distinct `VENT A` and
`VENT B` operators in the active zone, and operators may come from different
teams. Failed containment stalls advancing occupants and increments delay, but
never damages players or removes earned progress. The 2D lab projects the pure
logic as authored zone bounds, pressure state, valve assignments, route progress,
and entity-health diagnostics. 9 model + 3 lifecycle/schedule tests. Phase 23
provides the 3D facility target for later integration.

**Phase 16 — Deterministic networking.** ✅ *(completed 2026-06-19 —
[`labs/network_lab`](labs/network_lab/README.md), evidence
[png](docs/evidence/network_lab.png).)* Two peers each own one quantized
`PlayerIntent` and both simulate both bodies through Phase 20's fixed-step
controller. Frames commit only after both inputs arrive. A manual checksummed
packet codec carries input, cumulative ACK, committed frame, and state hash;
resend repairs a deterministic hostile transport with loss, delay, duplication,
and reordering. Both peers reach frame 240 with identical hashes and committed
input tapes, and replaying either tape from a fresh world reproduces the final
hash. Deliberate divergence is detected at the matching frame. The packet codec
also crosses real standard-library UDP loopback. 10 protocol/model + 4
lifecycle/schedule/input tests. Internet relay/NAT traversal remains a deployment
concern rather than being hidden inside this feasibility lab; Phase 17 owns the
queue/lobby/session lifecycle.

**Phase 17 — Matchmaking / session formation.** ✅ *(completed 2026-06-19 —
[`labs/session_lab`](labs/session_lab/README.md), evidence
[png](docs/evidence/session_lab.png).)* A deterministic matchmaker selects the
oldest compatible four-account group under explicit region, build, roster-size,
and rating-spread constraints; incompatible tickets remain queued. The formed
lobby assigns stable `PlayerId`s by account and balanced two-player `TeamId`s by
rating. A full connected ready roster drives a cancellable countdown into a
validated launch manifest containing protocol/build, host, seed, lockstep session,
and account→player/team mapping. In-match disconnect pauses at an exact frame,
host migration chooses the lowest connected account, reconnect preserves identity
and resumes the frame, timeout closes cleanly, and normal post-match returns to an
unready rematch lobby. 15 pure model + 4 lifecycle/schedule/input tests. Production
account/authentication services, relay allocation, and deployed lobby discovery
remain deployment concerns outside this feasibility boundary.

**Phase 18 — Progression & cosmetics.** ✅ *(completed 2026-06-20 —
[`labs/progression_lab`](labs/progression_lab/README.md), evidence
[png](docs/evidence/progression_lab.png).)* A persistence layer for unlocks and
cosmetics that **never touches simulation determinism**. A `Profile` earns XP from
match placements, levels up, unlocks cosmetics by level/win thresholds, equips them
one per slot (Color/Trail/Badge), and serializes to a compact save string that
round-trips (malformed saves rejected). Orthogonality is proven concretely: the
match is the `competitive_facility` brain, which takes no profile, so a test plays
the deterministic match before and after a fully-progressed, fully-equipped profile
and asserts the placements are identical. 8 model + 3 lifecycle tests.

---

## Working a phase

For refactor phases, the artifact is architectural evidence: dependency graph output,
the moved crate/module boundaries, preserved tests, and proof that affected labs still
launch or at least build. Screenshot evidence is only required when the refactor
changes visible presentation.

For every phase: build the smallest thing that answers its technical question;
keep input / simulation / presentation separate; add debug visibility for new
invisible state; add pure-logic and lifecycle tests; capture a screenshot for an
integration or visible-mechanic phase. Then run `cargo fmt`,
`cargo clippy --workspace --all-targets`, and `cargo test --workspace`, verify the
lab against its README, and mark the phase here with its evidence. Integration
phases reuse the proven labs as libraries; feasibility phases stay isolated until
they prove out. 3D labs add the `["3d", "png"]` Bevy features and keep their 3D
presentation a projection of the same dimension-agnostic simulation.
