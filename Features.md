# Gameplay Features

This inventory was built by scanning `Catalogue.md`, `README.md`, `ROADMAP.md`,
`game/README.md`, the promoted crates under `crates/`, the runnable labs under
`labs/`, and the assembled game source under `game/src/`.

It is a status document, not a roadmap. "Game-integrated" means the feature is in
`cargo run -p observed_game`. "Production logic" means the rules live in a promoted
crate and are testable without presentation. "Lab-proven" means the feature has an
isolated runnable proof but is not necessarily part of the assembled game.

## Status Legend

| Status | Meaning |
| --- | --- |
| Game-integrated | Playable in the assembled game. |
| Game-integrated, local/presentation | Active in the game, but outside the deterministic match brain. |
| Production logic | Promoted pure crate logic, generally projected by one or more labs or the game. |
| Lab-proven | Isolated runnable prototype or debug projection; not currently a shipped game surface. |
| Lab-proven, superseded | Valid proof that has been replaced by a newer integration path. |
| Tooling/support | Authoring, debugging, evidence, asset, or presentation support; not direct player-facing gameplay. |
| Deferred/not implemented | Explicitly out of scope, not adopted, or only a future trigger exists. |

## Current Playable Snapshot

The assembled game is a complete local loop:

`Splash -> Main Menu -> Loadout -> Lobby -> Match -> Results -> Main Menu`

The current match is a first-person, network-replicated competitive hybrid match
presented through the teleport place model: the player occupies one room or one
hallway piece at a time, crosses thresholds to transition, sees only the current
place, and unobserved connections can re-roll their destination or hallway
variation. The game currently includes keystone-gated exit progress, anchor
torch threshold pinning, teleport pads, pressure-gate route risk, safe bypass
signaling, rival avatars, a guardian, specialized rooms, a TAC-MAP, audio and
route-shift feedback, progression, loadout, lobby projection, results, and
drop-in asset fallbacks.

## Assembled Game Loop

| Feature | Status | Implementation | Notes |
| --- | --- | --- | --- |
| App state flow | Game-integrated | `game/src/flow.rs`, `game/src/screens.rs` | Splash, menu, loadout, lobby, match, results, pause, and return-to-menu are wired as Bevy states. |
| Main menu and navigation | Game-integrated | `game/src/screens/menu.rs`, `game/src/screens/input.rs` | Keyboard and controller-style menu navigation share one UI flow. |
| Loadout screen | Game-integrated | `game/src/screens/loadout.rs`, `observed_progression` | Lets players browse/equip unlocked cosmetics; cosmetics remain orthogonal to match results. |
| Lobby screen | Game-integrated | `game/src/screens/lobby.rs`, `observed_progression::session` | Projects a deterministic balanced session into a playable launch surface. |
| Match runtime | Game-integrated | `game/src/screens/match_runtime/`, `observed_net`, `observed_match::hybrid` | Hosts live first-person play and replicates resolved rounds to a remote peer over the hostile lockstep transport. |
| Results screen | Game-integrated | `game/src/screens.rs`, `game/src/flow.rs` | Shows escaped/absorbed/placement outcome and awards progression once. |
| In-match pause | Game-integrated | `game/src/screens.rs`, `game/src/screens/input.rs` | Releases/re-grabs cursor and allows quitting back to menu. |
| State-scoped cleanup | Game-integrated | `DespawnOnExit` in `game/src/screens.rs` | Tests assert screen entities/resources do not leak across repeated state cycles. |
| Match HUD | Game-integrated | `game/src/screens/hud.rs` | Shows match status, gate/keystone/item state, network state, guardian/log status, and controls relevant to the current match. |
| TAC-MAP overlay | Game-integrated | `game/src/tacmap.rs`, `game/src/screens/hud.rs` | `Tab` projects player location, rivals, collapse, spine route, keystones, and exit lock/open state from live sim state. |
| Audio cues and ambience | Game-integrated | `game/src/screens/audio.rs`, `game/src/screens/match_runtime/ambience.rs` | Includes ambience, footsteps, escape/success, door/reroute cues, and route-shift feedback. |
| Capture scenarios | Tooling/support | `game/src/capture/` | Bot POV, tour, room, ceiling, and event capture paths produce visual evidence. |

## Input, Identity, and Determinism

| Feature | Status | Implementation | Notes |
| --- | --- | --- | --- |
| Abstract player intent | Production logic; game-integrated | `crates/player_input` | `PlayerIntent` carries movement/look/action intent so hardware, bots, replay, and network can feed the same player systems. |
| Stable player/team/room/equipment IDs | Production logic; game-integrated | `crates/observed_core` | `PlayerId`, `TeamId`, `RoomId`, `PortId`, `EquipmentId`, and related helpers keep simulation identity separate from Bevy entities. |
| Shared deterministic PRNG utility | Production logic | `observed_core::prng` | Centralized SplitMix-style generator used by deterministic rewiring/selection code. |
| Deterministic replay model | Production logic and lab-proven | `observed_match::hybrid::replay`, `replay_lab`, `match_replay`, `fps_hybrid_match_lab` | Exact replay exists for scalar competition, integrated competitive facility, and hybrid match snapshots. |
| Deterministic lockstep | Production logic; game-integrated | `crates/observed_net`, `labs/network_lab`, `labs/net_match_lab` | Complete-frame gating, resend/ACK, checksums, and hashes keep peers synchronized through hostile simulated transport. |
| Live networked hybrid match | Game-integrated | `observed_net::netmatch`, `net_match_lab`, `game/src/flow.rs` | The assembled game uses `LiveNetMatch` to replicate host-resolved rounds to a remote peer. |
| Rich controller/remap adapter | Lab-proven; deferred | `labs/archie_input_lab` | `bevy_archie` can feed `PlayerIntent`, but the adapter stays lab-local until controller rebinding becomes a committed feature. |

## Traversal and Space

| Feature | Status | Implementation | Notes |
| --- | --- | --- | --- |
| 2D kinematic movement | Lab-proven | `labs/movement_lab` | Walk/run, acceleration, jumping, coyote time, jump buffering, slopes, stairs, moving platforms, respawn, and four bodies. |
| Authored climbing modes | Lab-proven | `labs/climbing_lab` | Ladders, ledge grab/hang/pull-up/drop/shimmy, and socket-based grapple traversal. |
| Shared 3D FPS controller | Production logic; game-integrated | `crates/observed_traversal`, `labs/fps_controller_lab`, `game/src/screens/match_runtime/` | Fixed-step AABB controller with facing-relative movement, sprint, jump, replay-safe pose, and wall/floor collision. |
| 3D elevation and stairs | Production logic; game-integrated | `crates/observed_traversal`, `labs/fps_elevation_lab`, `observed_match::maze` | Step-up over authored stair bands and multi-level generated routes are integrated into hybrid match snapshots and the game. |
| Teleport place model | Game-integrated | `game/src/teleport/` | Player occupies `Place::Room` or `Place::Hallway`; threshold crossing swaps to the next local place while preserving deterministic geometry. |
| Doorway/threshold identity | Game-integrated | `game/src/teleport/mod.rs`, `game/src/teleport/transition.rs` | Uses explicit room/hall threshold IDs and slot-aware alignment instead of approximate placement inference. |
| Room geometry | Game-integrated | `game/src/teleport/geom.rs`, `game/src/screens/place/` | Polygonal room footprints, doorway gaps, previews, collision containment, walls, ceilings, lights, props, and special room interiors. |
| Hallway template library | Game-integrated | `game/src/hallway.rs` | Straight, long, chicane, pressure-gate, climb, colonnade, and labyrinth hallway personalities. |
| Per-hallway labyrinths | Game-integrated | `game/src/maze.rs`, `game/src/hallway.rs` | Randomized-DFS plus braid pass creates deterministic dead ends and loops inside hallway pieces. |
| Whole-graph spatial maze | Production logic and lab-proven | `observed_match::maze`, `labs/fps_maze_lab` | Embeds the nine-room graph as real corridors; still important to the hybrid model, but the current game presentation uses the teleport place renderer. |
| WFC hallway/interior generation | Tooling/support; not currently used by game runtime | `labs/wfc_proc_gen_lab`, `game/src/wfc_maze.rs` | Candidate WFC generator and helper exist; `game/src/wfc_maze.rs` is present but no current game callsite uses it. |
| Local bot navigation | Game-integrated as debug/capture | `game/src/bot.rs`, `game/src/navmesh.rs` | Derived navmesh pathing consumes current place geometry for capture/debug; it is not authoritative simulation. |

## Observation, Rewiring, and Facility Topology

| Feature | Status | Implementation | Notes |
| --- | --- | --- | --- |
| Observe/decohere graph | Production logic | `crates/observed_observation`, `labs/observation_lab` | Observed rooms pin doorways; unobserved doorways deterministically re-match. |
| Protected route spine | Production logic | `crates/observed_facility::constraints`, `labs/constraint_lab` | Keeps the structure connected while non-spine links remain mutable. |
| Door as observation gate | Production logic and lab-proven | `crates/observed_doors`, `labs/door_lab` | Open doors freeze; closed doors hide/free connections; protected spine stays reachable. Not the main control surface of the current game presentation. |
| Authored room definitions | Production logic and lab-proven | `crates/observed_facility`, `labs/room_lab` | Explicit bounds, typed ports, collisions, rotation, attach/replace/despawn, and validation. |
| ASCII topology parser and validators | Production logic/tooling | `observed_facility::room_world::parse_ascii_map`, `ROADMAP.md`, `labs/topology_lab` | Completed Phase 32 capability: parse text topology, validate overlaps, port alignment, and short-wall constraints. `labs/topology_lab` has source/tests but no README in this worktree. |
| Continuous FPS visibility field | Lab-proven | `labs/fps_visibility_lab` | Frustum/range/wall occlusion tracks sub-room cells and freezes directly visible doorway endpoints. |
| Off-camera 3D replacement | Lab-proven | `labs/fps_rewire_lab` | Atomic hidden swaps defer while visible or underfoot to avoid popping or stranding. |
| First-person graph projection facility | Lab-proven, superseded | `labs/fps_facility_lab` | Authored 3D modules plus portal traversal over the observation graph; superseded for main play by concrete maze/hybrid and later teleport presentation work. |
| Rerouting concrete passages | Production logic and lab-proven | `observed_match::hybrid`, `labs/fps_reroute_lab`, `labs/fps_hybrid_match_lab` | Graph changes update target routes; commits are deterministic, safe, and replayable. |
| Teleport hallway rerolling | Game-integrated | `game/src/teleport/`, `game/src/hallway.rs` | Unobserved room/hall edges can re-roll destination/variation when the player is not inside them. |
| Anchor torch threshold pinning | Game-integrated, local/presentation | `game/src/items.rs`, `game/src/screens/match_runtime/teleport.rs` | One carried anchor torch can freeze a room's threshold table or a hallway edge until picked up. |
| Room lock rejecting new inbound relations | Game-integrated, local/presentation | `game/src/items.rs` | A room-level anchor keeps the exact threshold set visible at drop time and blocks new inbound relations. |
| Doorway previews | Game-integrated | `game/src/screens/place/preview.rs` | Renders current/adjacent place previews so threshold destinations are readable. |

## Objectives, Puzzles, and Items

| Feature | Status | Implementation | Notes |
| --- | --- | --- | --- |
| Keystone-gated exit | Game-integrated | `game/src/keystones.rs`, `game/src/teleport/` | Deterministic keystone placement on intermediate spine rooms; exit stays locked until required keystones are held. |
| Keystone pickups | Game-integrated | `game/src/keystones.rs`, `game/src/screens/place/items.rs` | Walking over a room's uncollected keystone collects it once and removes it from the TAC-MAP. |
| Locked exit doorway | Game-integrated | `game/src/teleport/`, `game/src/screens/place/strategies.rs` | Hallways toward the exit show a solid locked gate until keystone inventory opens it. |
| Teleport pads | Game-integrated, local/presentation | `game/src/items.rs`, `game/src/screens/match_runtime/mod.rs` | Two dropped pads form a reusable bidirectional link activated from either pad. |
| General interaction framework | Production logic and lab-proven | `crates/observed_interaction`, `labs/interaction_lab` | Instant, sustained, exclusive, shared, interrupted, carry, socket, and climb interactions with stable IDs. |
| Persistent equipment framework | Production logic and lab-proven | `crates/observed_interaction::equipment`, `labs/equipment_lab` | Batteries, structural jacks, cable spools, deployable lights, grapple devices, sockets, power drain, handoff, recover, and room replacement persistence. |
| Full equipment set in assembled game | Deferred/partial | `game/src/items.rs`, `crates/observed_interaction` | The game currently has a smaller local item layer: anchor torch, teleport pads, and keystones. Battery/jack/cable/light/grapple remain lab/prod features, not current game equipment. |
| Carryable power cell and powered door | Lab-proven | `labs/facility_sandbox`, `labs/equipment_lab` | End-to-end sandbox objective proves power source and powered-door gating. |
| Structural jack bridge | Lab-proven | `labs/facility_sandbox`, `labs/equipment_lab` | Deployable jack bridges a pit in the sandbox; not in current assembled game. |
| Player-built route cables | Lab-proven | `labs/route_lab` | Budget-limited team cables pin graph connections; opponents can cut them. Not integrated into the assembled game. |
| Team cooperation mechanics | Lab-proven | `labs/team_lab` | Four players, two teams, item contention, narrow passage capacity, multi-player climb point, two-operator machine, separation/reunion tracking. |
| Cooperative pressure hazard | Lab-proven | `labs/hazard_lab` | Director-steered pressure front requires two relief roles, can cross team boundaries, stalls advancement without damage. |
| Pressure-gate route risk | Game-integrated | `observed_match::hybrid`, `game/src/hallway.rs`, `game/src/screens/place/preview.rs` | Red shortcut can pulse active and reset the body to checkpoint without reducing match progress. |
| Safe bypass routes | Game-integrated | `observed_match::hybrid`, `observed_style::SurfaceRole::SafeBypass` | Cyan route avoids pressure tiles and is longer than the risky direct line. |
| Discovery/gated room types | Lab-proven, partially integrated | `labs/discovery_lab`, `game/src/keystones.rs` | Lab proves hidden room types, shifting types, survey/sensor/decoy/reactor/power/keystone gate. The assembled game currently integrates the simpler keystone gate, not the full hidden-type discovery economy. |

## Competition, Director, and Match Rules

| Feature | Status | Implementation | Notes |
| --- | --- | --- | --- |
| Capacity-limited exit race | Production logic; game-integrated | `observed_match::competition`, `observed_match::facility` | Multiple teams race; exits fill; remaining teams are locked out/absorbed. |
| Deterministic standings | Production logic; game-integrated | `observed_match::competition`, `observed_match::facility` | Ties and finish order resolve deterministically by team/state. |
| Contested shared control | Production logic; game-integrated | `observed_match::competition::RaceAction::Seize`, `game/src/screens/place/mod.rs` | Seizing costs a round and grants a speed/control advantage; game also has Guardian Control Room interaction. |
| Indirect-only interference | Production logic; game-integrated | `observed_match::competition`, `observed_match::facility`, tests | Player/team progress is monotonic; opponents can win shared advantages but not directly lower progress. |
| Facility director collapse | Production logic; game-integrated | `observed_match::director`, `observed_match::facility`, TAC-MAP/HUD | Collapse chases the leader, absorbs teams that fall behind, and marks swallowed rooms. |
| Absorbed teams join director | Production logic | `observed_match::director`, `observed_match::facility` | Absorbed teams become director members and can accelerate collapse in the model. |
| Competitive mutable facility | Production logic and lab-proven | `observed_match::facility`, `labs/competitive_facility` | Observation, protected spine, competition, director, capacity exits, and rewiring compose as one deterministic match. |
| First-person competitive match over portal facility | Lab-proven, superseded | `labs/fps_match_lab` | Full FPS competitive match and replay over the portal/module facility; superseded by hybrid maze and game presentation. |
| First-person hybrid match | Production logic; game-integrated | `observed_match::hybrid`, `labs/fps_hybrid_match_lab`, `game` | Spatial action boundary, safe/risky routes, traps, reroute feedback, deterministic snapshots, and replay. |
| Networked hybrid match | Production logic; game-integrated | `observed_net::netmatch`, `labs/net_match_lab`, `game` | Clean and hostile transports converge to identical match/maze/pose snapshots. |
| Rival team markers/avatars | Game-integrated, local/presentation | `game/src/rivals.rs`, `game/src/screens/place/mod.rs`, TAC-MAP | Rivals are projected from deterministic team rooms and rendered only when co-present in the current room, plus map pips. |
| Guardian AI | Game-integrated, local/presentation | `game/src/guardian.rs`, `labs/guardian_ai_lab` | Moves room-by-room toward target, freezes under gaze/anchor/rival observation, banishes under anchor light, teleports player on touch, and can be reassigned to rivals. |
| Guardian Control Room | Game-integrated | `game/src/screens/place/mod.rs`, `ROADMAP.md` | Room 3 console reassigns guardian target to rival teams or local player. |
| Guardian Observation Room | Game-integrated | `game/src/screens/place/mod.rs`, `ROADMAP.md` | Room 6 monitor grid flashes for the guardian's current room. |
| Tether Camera Room | Game-integrated | `game/src/screens/place/mod.rs`, `ROADMAP.md` | Room 5 monitor grid lights rooms that contain an anchor torch. |
| Master Room one-way exits | Game-integrated | `game/src/teleport/`, `ROADMAP.md` | Room 4 has direct one-way exits to other rooms. |
| Interior room collisions | Game-integrated | `game/src/teleport/geom.rs`, `game/src/screens/place/mod.rs`, `ROADMAP.md` | Console and other interior obstacles physically block player movement. |

## Progression, Session, and Meta Systems

| Feature | Status | Implementation | Notes |
| --- | --- | --- | --- |
| Profile XP and levels | Production logic; game-integrated | `observed_progression::progression`, `game/src/flow.rs` | Placements award XP; levels advance deterministically. |
| Cosmetic unlocks/equip | Production logic; game-integrated | `observed_progression`, `game/src/screens/loadout.rs` | Level/win thresholds unlock cosmetics; one equipped per slot; cosmetics do not affect match outcomes. |
| Save-string serialization | Production logic and lab-proven | `observed_progression`, `labs/progression_lab` | Profiles round-trip as deterministic compact strings; malformed saves are rejected. |
| Matchmaking queue | Production logic and lab-proven | `observed_progression::session::matchmaking`, `labs/session_lab` | Deterministic compatible roster selection by enqueue order, region/build/rating spread. |
| Balanced team assignment | Production logic; game-integrated projection | `observed_progression::session::lobby`, `game/src/screens/lobby.rs` | Four accounts become stable player/team seats with balanced teams. |
| Lobby readiness/countdown/manifest | Production logic and lab-proven | `observed_progression::session`, `labs/session_lab` | Launch manifest contains build/protocol/seed/host/lockstep/session/roster data. |
| Host migration/reconnect/rematch | Production logic and lab-proven | `observed_progression::session` | Model supports host migration, reconnect continuity, post-match rematch, and terminal closure. The assembled game projects a local session rather than an online service. |

## Presentation, Legibility, Assets, and Authoring

| Feature | Status | Implementation | Notes |
| --- | --- | --- | --- |
| Neon-noir semantic style | Production logic; game-integrated | `crates/observed_style`, `labs/style_lab` | Maps semantic roles to colors/emission/legend; tests enforce signal brightness and contrast. |
| Gameplay visual legend | Production logic; game-integrated | `observed_style::legend`, `game` consumers | Marker/surface roles have documented meanings rather than ad-hoc colors. |
| Semantic outlines | Lab-proven; deferred | `labs/outline_legibility_lab` | `bevy_mod_outline` works with Bevy 0.18.1 and style-owned outlines, but promotion waits for a second consumer. |
| Semantic particle VFX | Lab-proven; deferred | `labs/semantic_vfx_lab` | `bevy_hanabi` can project semantic events; stays lab-local until another visible consumer needs it. |
| Drop-in asset slots | Production logic; game-integrated | `crates/observed_assets`, `labs/asset_lab`, `game` | Texture/model/sound/HDR slots load if present and use procedural fallbacks if absent. |
| Textures/models/sounds/HDR in game | Game-integrated with fallbacks | `assets/`, `game/src/screens.rs`, `game/src/screens/place/` | Floors/walls/ceilings, fixtures, props, player/bot/equipment models, sounds, and environment are planned slots with fallbacks. |
| TrenchBroom import | Lab-proven; deferred | `labs/trenchbroom_lab` | `.map` imports project to domain topology/collision; dependency remains isolated to the lab. |
| LDtk schematic import | Lab-proven; deferred | `labs/ldtk_schematic_lab` | Useful for 2D tactical maps/route sketches; promotion deferred until durable schematic input is needed. |
| Navigation probe with third-party navmesh | Lab-proven; deferred | `labs/navigation_probe_lab` | Proves derived navmesh can agree with authoritative graph/door state; game adoption deferred until bots/AI need it. |
| Live ECS inspector | Tooling/support | `labs/inspector_lab` | `bevy-inspector-egui` accepted behind default-off `dev_tools`; not gameplay. |
| Lab config and event tracing | Tooling/support | `labs/lab_observability_lab` | Typed debug knobs, JSON persistence, trace logging, and config/manifest boundary; lab-local. |
| Evidence capture pipeline | Tooling/support | `labs/capture_pipeline_lab`, game capture modules | Deterministic screenshot/sequence/GIF workflows support visual review. |

## Lab Coverage Index

| Lab | Main feature status |
| --- | --- |
| `menu_lab` | Lab-proven screen lifecycle, pause/reset/menu cleanup; concepts integrated into game. |
| `control_lab` | Lab-proven `PlayerIntent`, player/device assignment, recording/replay, rebinding. |
| `movement_lab` | Lab-proven 2D movement course. |
| `climbing_lab` | Lab-proven ladder/ledge/grapple traversal. |
| `interaction_lab` | Lab-proven interaction state machine; promoted to `observed_interaction`. |
| `room_lab` | Lab-proven modular rooms/ports/collisions/replacement; promoted to `observed_facility`. |
| `equipment_lab` | Lab-proven persistent equipment; promoted to `observed_interaction`. |
| `team_lab` | Lab-proven multi-player/team contention and cooperation. |
| `facility_sandbox` | Lab-proven first integrated objective with power cell, powered door, jack, map, and spectator camera. |
| `observation_lab` | Lab-proven observe/decohere graph; promoted to `observed_observation`. |
| `constraint_lab` | Lab-proven protected spine/connectivity; promoted to `observed_facility`. |
| `door_lab` | Lab-proven doors-as-observation-gates; promoted to `observed_doors`. |
| `competition_lab` | Lab-proven capacity-limited race and contested control; promoted to `observed_match`. |
| `director_lab` | Lab-proven collapse/absorption director; promoted to `observed_match`. |
| `replay_lab` | Lab-proven exact replay over competition. |
| `route_lab` | Lab-proven player-laid route cables; not game-integrated. |
| `incentive_lab` | Lab-proven splitting/backtracking scoring; not game-integrated. |
| `hazard_lab` | Lab-proven two-role cooperative pressure hazard; not game-integrated except the game has a different pressure-gate route risk. |
| `network_lab` | Lab-proven deterministic lockstep; promoted to `observed_net`. |
| `session_lab` | Lab-proven deterministic matchmaking/lobby lifecycle; promoted to `observed_progression`. |
| `mutable_facility` | Lab-proven objective over observe/decohere/spine; promoted to `observed_match`. |
| `competitive_facility` | Lab-proven integrated competition/director/mutable graph; promoted to `observed_match`. |
| `match_replay` | Lab-proven replay/spectator over integrated match. |
| `fps_observation_lab` | Lab-proven camera-driven observation; superseded by later FPS/hybrid integrations. |
| `fps_controller_lab` | Lab-proven deterministic 3D controller; promoted to `observed_traversal`. |
| `fps_visibility_lab` | Lab-proven continuous visibility field; not current game control surface. |
| `fps_rewire_lab` | Lab-proven atomic off-camera replacement; informs rerouting/hybrid work. |
| `fps_facility_lab` | Lab-proven 3D typed room graph projection; superseded by hybrid/teleport game presentation. |
| `fps_match_lab` | Lab-proven first-person competitive match and replay; superseded by first-person hybrid match. |
| `fps_maze_lab` | Lab-proven graph-to-spatial-maze embedding; promoted to `observed_match::maze`. |
| `fps_reroute_lab` | Lab-proven live rerouting passages; promoted into hybrid match logic. |
| `fps_hybrid_match_lab` | Lab-proven full hybrid match; promoted to `observed_match::hybrid` and game. |
| `net_match_lab` | Lab-proven networked hybrid match; game uses the live variant. |
| `fps_elevation_lab` | Lab-proven vertical step-up; promoted to shared traversal and game/hybrid route heights. |
| `progression_lab` | Lab-proven profile/cosmetics/save orthogonality; promoted to `observed_progression` and game. |
| `asset_lab` | Lab-proven asset-slot manifest; promoted to `observed_assets` and game. |
| `style_lab` | Lab-proven semantic visual language; promoted to `observed_style` and game. |
| `discovery_lab` | Lab-proven hidden room types and gated objectives; partially represented by the game's keystone gate. |
| `guardian_ai_lab` | Lab-proven guardian behavior; game has an integrated guardian variant. |
| `topology_lab` | Source/test lab for topology editing/validation; no README in this worktree. |
| `trenchbroom_lab` | Lab-proven `.map` authoring import; deferred for game adoption. |
| `ldtk_schematic_lab` | Lab-proven 2D schematic import; deferred for game adoption. |
| `navigation_probe_lab` | Lab-proven derived navmesh and threshold collapse; deferred for game adoption. |
| `wfc_proc_gen_lab` | Lab-proven WFC map generation candidate; not current game runtime. |
| `outline_legibility_lab` | Lab-proven outline readability; deferred for promotion. |
| `semantic_vfx_lab` | Lab-proven semantic particle VFX; deferred for promotion. |
| `capture_pipeline_lab` | Tooling proof for deterministic offscreen evidence capture. |
| `archie_input_lab` | Lab-proven controller/remap/haptics adapter; deferred for production input adoption. |
| `lab_observability_lab` | Tooling proof for debug config and event traces. |
| `inspector_lab` | Tooling proof for optional live ECS inspector. |

## Known Deferred or Partial Areas

- Real online transport, relay/NAT traversal, authentication, lobby discovery,
  and cross-platform floating-point certification are not implemented; current
  networking is deterministic lockstep over simulated hostile transport plus UDP
  codec tests.
- Universal climbing, full rope physics, and arbitrary procedural mesh geometry
  remain explicit non-goals. Traversal uses authored ladders/ledges/sockets,
  AABB step-up, deterministic hallway/maze generation, and typed thresholds.
- Full discovery-room type economy is lab-only. The assembled game currently
  uses keystone-gated exit inventory, anchor torch tethering, and special rooms,
  but not the full shifting hidden-room-type system.
- Full equipment framework from `observed_interaction` is not yet the game
  inventory. The game has a small local item layer for keystones, anchor torch,
  and teleport pads.
- Two-operator cooperative hazards and route cable competition remain lab-only.
  The game has pressure-gate route risk and anchor pinning, but not the full
  cooperative hazard/cable budget/cut loops.
- Authoring imports (`TrenchBroom`, `LDtk`, WFC) are proven candidates, not
  production game content sources.
- Semantic outlines and particle VFX are proven readability candidates but are
  not promoted into the assembled game yet.
