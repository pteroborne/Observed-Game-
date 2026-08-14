# Project Catalogue

This catalogue provides an architectural map of the **Observed 2** workspace. It outlines the modular structure of the project, details the responsibilities of promoted production crates, groups the feasibility labs, maps the assembled game package, documents duplicate code patterns, and identifies bloated source files targeted for future modularization.

---

## Workspace Structure Overview

The project is structured into three main areas to enforce clean boundaries between pure simulation logic, isolated feasibility testing, and game assembly:

```text
/
├── Cargo.toml                # Workspace configuration listing resolver 3 members
├── agents.md                 # Project north-star goals, core architectural rules, and coding conventions
├── CLAUDE.md                 # Streamlined runbook: commands, verification, and evidence pipeline
├── ROADMAP.md                # Narrative timeline tracking completed milestones and recent phase details
├── crates/                   # Promoted production crates containing pure, deterministic simulation logic
├── labs/                     # Independently runnable feasibility prototypes and visual test showcases
├── game/                     # The assembled first-person 3D player-facing game package
├── server/                   # Bevy-free authoritative LAN server library and binary
├── deploy/                   # Container, Compose, and host automation for browser labs
├── assets/                   # Procedural fallback markers & drop-in models/sounds (observed_assets-governed)
└── docs/                     # Visual evidence (screenshots, GIFs), design plans, and audit evaluations
```

---

## Production Crates (`crates/`)

These crates represent the stable core of the game's simulation layer. They contain **no rendering or engine presentation code** and must remain fully deterministic, portable, and unit-testable. Each crate is fully documented in its local `README.md`:

1. **`player_input`** — [README](crates/player_input/README.md)
   - *Purpose:* Defines abstract `PlayerIntent` and `PlayerId` boundaries. Decouples physical input hardware (keyboard/mouse, controllers, network inputs, replay tapes, or bots) from character behavior.
2. **`observed_core`** — [README](crates/observed_core/README.md)
   - *Purpose:* Domain identifiers and basic structural helpers (`RoomId`, `PortId`, `EquipmentId`, `TeamId`, `Side`, `ThresholdSlotId`).
3. **`observed_doors`** — [README](crates/observed_doors/README.md)
   - *Purpose:* Pure logic for doors acting as observation gates. Closed doors hide and free connections; open/observed doors freeze connectivity.
4. **`observed_facility`** — [README](crates/observed_facility/README.md)
   - *Purpose:* Facility topology rules: authored room definition templates, transform alignments, port connectivity, overlapping geometry validation, static `MapSpec` WFC, the demoted square `FullWfcWorld`, and the canonical authored-tile `HexWfcWorld` with blueprint/ramp/threshold pinning, readable weighted negative space, production room quotas, connected open-volume and decision-cadence gates, bounded frontier mutation, protected halos, per-cell revisions, patch deltas, stable identity, and weighted-route guards.
5. **`observed_interaction`** — [README](crates/observed_interaction/README.md)
   - *Purpose:* Persistent equipment systems (batteries, structural jacks, light spools) and a deterministic tick-based interaction engine resolving holding, activating, quorum, interruptions, and item contention.
6. **`observed_match`** — [README](crates/observed_match/README.md)
   - *Purpose:* The competitive match brain, containing:
     - `competition`: team standings, race metrics, capacity-limited exit gates.
     - `director`: AI director pressure models, collapse scaling, and catch-up mechanisms.
     - `elimination`: multi-round elimination-series state, first-escape countdowns, adversary escalation, and team-keyed tool ownership.
     - `teamplay`: seeded two-member bot teamplay, co-op room beats, tool usage, guardian setbacks, and round outcomes for spectator-driven series play.
     - `maze`: seeded spatial labyrinth generator translating graphs into walkable corridor geometry.
     - `hybrid`: deterministic orchestration of the first-person hybrid round-stepping and matching.
     - `hex_wfc`: canonical hex-facility race state, strict cell/whole-room and typed room-socket projection, incremental Rapier deltas, ramp/shaft movement, versioned commands, contested keystone → synchronized station → team-exit objectives, caged lantern anchors, optional local monitor surveys, the physical Guardian, player-local map knowledge, content-hashed replay, bounded mutation, objective-aware bots, and deterministic snapshots.
7. **`observed_content`** — [README](crates/observed_content/README.md)
   - *Purpose:* Engine-independent, deny-unknown-fields schemas for immutable district, traversal, authored-module, port/socket, navigation, asset-provenance, convex-bake, and frozen place-layout data. Canonical SHA-256 simulation and presentation hashes keep collision/gameplay compatibility separate from optional dressing.
8. **`observed_net`** — [README](crates/observed_net/README.md)
   - *Purpose:* Hostile transport repair, wire protocol checksum verification, deterministic lockstep serialization, and the production UDP LAN client/browser protocol. LAN peers exchange input/content versions, replay server-owned authoritative frames, and reject incompatible movement/collision content before launch.
9. **`observed_observation`** — [README](crates/observed_observation/README.md)
   - *Purpose:* The underlying graph database tracking visibility state. Pinned observed rooms are frozen, while unobserved paths decohere and rewire.
10. **`observed_progression`** — [README](crates/observed_progression/README.md)
   - *Purpose:* Cosmetic profile unlocks, matchmaking queue status, lobby formation, reconnect logic, and session lifecycles.
11. **`observed_style`** — [README](crates/observed_style/README.md)
    - *Purpose:* The semantic visual design system (neon-noir district palettes, emissive intensities, signaling tiers, Outline overlay rules, and accessibility legend mappings).
12. **`observed_traversal`** — [README](crates/observed_traversal/README.md)
    - *Purpose:* Fixed-timestep traversal behind a pure `ArenaSpec`/`TraversalWorld` boundary, plus the canonical runtime profile, shared local follower, and deterministic module-local graph/cursor vocabulary. The assembled game uses the deterministic raw-Rapier KCC exclusively; the legacy backend tag remains only for replay/network compatibility fixtures.
13. **`observed_assets`** — [README](crates/observed_assets/README.md)
    - *Purpose:* Local directory asset-slot index maps, avoiding hardcoded string paths in simulation presentation.
14. **`observed_diagnostics`** — [README](crates/observed_diagnostics/README.md)
    - *Purpose:* Pure visual-audit schemas and checks for converting rendered-game state into agent-readable validation evidence.
15. **`observed_hex`** — [source](crates/observed_hex/src/lib.rs)
    - *Purpose:* The single pure source of truth for axial coordinates, eight prism faces, packed port signatures, quantized-hex metrics, grid indexing, and exact lattice-to-world mapping.
16. **`observed_authoring`** — [source](crates/observed_authoring/src/lib.rs)
   - *Purpose:* Pure TrenchBroom `.map` import and deterministic `tilec` pipeline: strict cell/whole-room schemas, explicit spatial/family/interface/traversal module contracts, SHA-256 hull catalogues, runtime rotation/register expansion, legacy-manifest compatibility, and brush-to-convex-collider projection. Also owns the on-disk authored **composition profile** (`composition.rs`) and the domain-separated fold that makes it part of the simulation content hash — tiles say what the solver *may* build, the profile says what it *tends* to build.

17. **`observed_schematic`** — [source](crates/observed_schematic/src/lib.rs)
   - *Purpose:* Line-and-band mesh construction for hex schematic views: batched line lists, low solid wall bands, floor rings, stair/ramp glyphs, compact directional level arrows, and the exact quantized prism. Colour-blind by design — callers ask `observed_style` what a semantic state looks like and supply their own materials — so two views drawing the same lattice cannot disagree about its shape. Extracted from `iso_observer_lab` once `tactics_lab` became a second consumer.

18. **`observed_cutaway`** — [source](crates/observed_cutaway/src/lib.rs)
   - *Purpose:* Shared, colour-blind authored-hull projection for isometric interior views: stable tile-key mesh caching, convex triangulation, floor/ceiling/interior/perimeter classification, camera-facing cutaways, configurable low-wall perimeter projection, focus/deck selection, and batched Bevy meshes. Extracted from Composition Studio when `tactics_lab` became its second consumer.

---

## Code Duplications & Design Overlaps

The following duplication patterns have been identified in the codebase. These should be unified or centralize in the future to keep the code DRY (Don't Repeat Yourself):

### 1. `SplitMix` Pseudo-Random Number Generator (PRNG)
- **Status:** Centralized.
- **Details:** The identical `SplitMix(u64)` structure lives in `observed_core::prng` and is imported as a shared utility throughout the production crates. The game layer had regressed with two re-duplicated stream copies (`game/src/maze.rs`, `game/src/guardian.rs`); Refactor Arc G1 converted both onto the shared `SplitMix`, with seeded determinism tests pinning the streams bit-identical before and after.
- **Not duplicates (do not "unify"):** `game/src/hallway.rs`, `game/src/teleport/geom.rs`, and `observed_style::district_for` each contain a keyed, one-shot splitmix64 **hash finalizer** (seed/key mixed once, no stateful `next_u64` advance) used to derive a stable value from a room/edge key. These have different mixing and state-advance semantics from the streaming `SplitMix` generator by design; rewriting them onto `SplitMix::next_u64()` would shift their deterministic outputs (hallway layouts, room shapes, district palettes).

### 2. Cardinal Directions representation
- **Status:** Consolidated.
- **Details:** Consolidated the overlapping compass direction representations (`Side` and `Cardinal` enums) into a single `direction` module in `observed_core`.

### 3. `PacketError` Enum
- **Status:** Consolidated.
- **Details:** Consolidated the duplicate enums from `crates/observed_net/src/netmatch.rs` and `crates/observed_net/src/protocol.rs` into a unified root `PacketError` in `crates/observed_net/src/lib.rs`.

---

## Feasibility Labs (`labs/`)

The prototype labs in `labs/` are independent Bevy applications designed to isolate and test specific technical questions. They follow a strict sandbox model, allowing full reset (`R` key) without restarting. They are grouped here by testing domain:

### Foundation & Controls
- `menu_lab`: Resettable semantic-widget and screen-lifecycle proof: stable focus IDs/order/restoration, pointer/keyboard/controller parity, accessible disabled controls, pause/resume/reset, and repeated leak-free return-to-menu cycles.
- `control_lab`: Rebind overlays, intent playback, and controller assignment.
- `session_lab`: Lobby states, team assignment, remote peer simulator.

### Traversal & Physics
- `movement_lab` & `climbing_lab`: Walk, run, jump, coyote buffers, ladders, ledge-grabbing, socket-based grapple ropes.
- `fps_controller_lab`, `fps_elevation_lab` & `gantry_lab`: 3D transition, dynamic AABB collision, elevation changes, and two-level jump-map hallway timing.
- `wellshaft_lab`: Production-controller prototype for a multi-threshold vertical silo, proving a hexagonal center pillar, six radial landing bridges, visible/collidable spiral stair treads, reset stability, and staged top/bottom/plan evidence.
- `rapier_determinism_lab`: Isolated feasibility spike (the "only if the custom controller proves insufficient" escape hatch) answering whether **rapier3d 0.34** can step convex + smooth (ball/capsule/convex-hull) colliders in fixed-dt lockstep with `enhanced-determinism` and stay bit-for-bit reproducible. Raw rapier (not `bevy_rapier`) owns the step loop; two identical worlds run side by side with a live hash-divergence monitor plus an offline bitwise-replay unit test. Verdict: bit-identical; its controller path is now promoted into the game while the spike remains the determinism baseline.
- `rapier_controller_lab`: Resettable raw-Rapier kinematic-capsule proof scene. It renders the exact oriented structural collider list the controller consumes, with an angled wall and autostep platform, while input remains a `PlayerIntent` boundary. The determinism spike promoted this controller path into the game; the lab keeps its focused manual and replay proof.
- `rapier_authoring_lab`: Combined TrenchBroom/Rapier vertical slice. A typed, editable Quake `.map` projects into stable room/port/door semantics and one static Rapier convex hull per brush; a fixed-step kinematic capsule traverses the exact sloped ramp, reacts to model-owned door collider mutation, resets cleanly, and replays scripted intents bit-for-bit. This remains lab-local pending playtest and production-boundary review.
- `rapier_portal_lab`: Authored-module/teleport vertical slice recreating Gantry and Colonnade brush kits, composing them through a deterministic two-cell WFC strip, rendering the exact frozen hallway snapshot into an isolated render-to-texture threshold, and swapping Rapier collision worlds on crossing. The preview camera follows the player as a spatial window while its meshes remain outside the playable render/collision world. It ratchets the lock contract: player observation and anchors both freeze BLAME, while only anchors illuminate the frame indicator.
- `rapier_aesthetic_lab`: Visual successor to the authored portal slice, proving the shared neon-noir language survives imported convex geometry: an Archive/BLAME Gantry uses cool structural mass and amber commitment edges; a Reactor/Silo Colonnade uses separated warm practical pools. Both retain HDR bloom, district fog, semantic-only materials, and the same anchor-purple frame signal across WFC recomposition.
- `content_manifest_lab`: Data-driven content boundary proof. A deny-unknown-fields JSON schema selects two district treatments, their authored Gantry/Colonnade modules, typed ports/WFC weights, and curated CC0 OpenGameArt dressing. Validation canonicalizes a deterministic manifest hash, checks provenance/fingerprints and Legibility Contract bounds, and requires every selected brush to remain a Rapier convex collider. Imported GLB materials are presentation-only and normalized through `observed_style`; `M` proves named procedural fallbacks without changing architecture or collision.

### Observation & Procedural Geometry
- `observation_lab` & `door_lab`: 2D graph transitions, unobserved doorway rewiring, door leaf slam animations.
- `constraint_lab`: Route-spine constraint validation over changing graphs.
- `fps_observation_lab`, `fps_rewire_lab` & `fps_reroute_lab`: 3D continuous visibility verification, off-camera replacement, and passage previews.
- `wfc_proc_gen_lab`, `ldtk_schematic_lab`, `room_lab` & `topology_lab`: Wave Function Collapse layout generation, LDtk tileset parser, resettable authored room-template catalogue, ASCII level editor, and wall alignment checks. Production `MapRoom` records a deterministic selection from the same room-template vocabulary. `wfc_proc_gen_lab` also archives the game's former hallway-interior WFC generator (`hallway_wfc.rs`, moved from `game/src/wfc_maze.rs` in Refactor Arc G1 — never called in the shipping game, whose interior mazes are the randomized-DFS + braid generator in `game/src/maze.rs`), kept compiling with a connectivity smoke test in case WFC returns for map generation.
- `full_wfc_lab`: Three-level debug projection of the continuous full-WFC experiment. It renders every level together, exposes PlayerId occupancy, observed threshold/hall-chain pins, weighted A* route and candle telemetry, timed/manual decoherence pulses, and full reset without restarting. `OBSERVED2_CAPTURE=<path>` records a settled post-pulse evidence frame and exits.
- `hex_wfc_lab`: The retained hex-WFC proof and inspection surface: compact animated collapse/relayout, plus a production-sized free-fly atlas using live room quotas and runtime room/cell catalogues. Its trace-prefix-to-cell-state fold now lives beside `SolveStep` in `observed_facility::hex_wfc::trace` (`fold_trace`/`summarise_trace`/`cells_from_world`), shared with Composition Studio's solve replay. Its whole-map room/hall LOD and camera-local exact-hull streaming expose all 10 played room roles and 8 hall archetypes without recreating a facility-wide rendering/physics cost; `Home` frames the map and `[`/`]` jump the concept index.
- `hex_tile_lab`: Authored tile intake and traversal proof. It cycles manifest tiles, renders exact convex geometry/colliders, validates quantized footprints and port signatures, and proves the shared controller walks the full two-level ramp prefab without jumping.
- `map_observer_lab`: Resettable 3D Map Observer lab that instantiates the entire continuous full-WFC facility layout in one unified 3D coordinate space. It spawns a 3D flying observer camera (WASD flight, right-click look, sprint) with snapping hotkeys (teleport directly to Start, Wellshaft Tower, Gantry Course, Climb Shaft, Keystone Room, and Exit portal) and screenshot keys [C]/[Enter] for visual map audits.

- `tactics_lab`: Turn-based tactical variant of the canonical hex facility, built as a gameplay-tuning instrument for Arc T's shallow-choices and imperceptible-change findings. Its Guided preset reveals the complete map and exit on a single floor while leaving observation/freeze semantics intact; setup uses bidirectional discrete sliders only for rules with three or more choices, binary toggle buttons for two-state rules, and explicit observation-radius plus decoherence-coverage controls. Coverage offers four increasingly broad candidate-pocket bands, reports their target cell count for the configured lattice, and remains within solver-safe bounds. A bounded readability composition profile produces at least twenty-percent true void space across the pinned seed corpus while retaining seven or more nonblank WFC archetypes and all normal route/room validation. Its primary view is a single authored deck with ceilings removed and every perimeter hull capped at one-third height; its second view is a filled top-down operations map of the active deck with literal doorway gaps and compact up/down level arrows. Both views retain style-owned architecture-register materials and local district practical pools instead of flattening ordinary structure into one orange dev-grid treatment, while gameplay signals keep semantic priority. They have independent pan/zoom poses beside a display-fixed, internally scrolling command dock with exclusive pointer ownership. Hover previews the exact AP-bounded route and names the district, movement resolves in visible beats, and floating production-runner projections interpolate along the same logical nav steps used by manual and bot play. Simulation-owned teleport plates project as persistent unlinked/linked floor assemblies, and a dedicated traversal event drives an input-gated rising-ring/light-column/descent animation without changing replay state. A deterministic objective bot can run/pause or single-step the player squad through the ordinary logged action boundary for spectating and debugging. Setup, loading/error, pause/help and results form one player-facing shell. Turn-based telegraphs reserve a one-ring cadence buffer, scale with the configured coverage, and keep routine movement from accidentally holding an oversized pocket; commits still use the production observation-safe relayout, classify zero-delta solves separately from holds, and keep successfully rewritten cells explicitly marked for the following turn.
- `iso_observer_lab`: Arc O's composition instrument and the arc's showcase surface. It solves a production-scale `HexWfcWorld`, projects its authored geometry, and draws the result as a retro-futuristic console schematic: phosphor line work under a strong HDR bloom on a dark screen, with cells meeting edge to edge and low solid wall bands at a third of cell height, with hue spent on one question — **green will not rewire, red the solver can**. Walls state connectivity directly: a sealed face is a solid band and a doorway is a real gap in it, so routes are traced by following the breaks and a multi-cell room emerges as one enclosed area because its internal faces are open. Shafts carry a stair glyph and ramps a slope-and-chevron, so a floor change is legible without selecting anything. `Tab` cycles level 0 … N-1 then all; `G` swaps the symbolic outline for the real authored hulls (structural edges only — coplanar triangulation diagonals are filtered); `D` restores the district-coloured solid massing as a dev view. Scroll zooms, right-drag pans, `Home` resets, and clicking a hex pins it and reports the resolved tile key, hull/vertex counts, archetype, ports, district, room role and whether the solver may move it. `[`/`]` cycle the five pinned seeds, `R` resolves, `Esc` clears. `OBSERVED2_CAPTURE=<dir>` walks every seed and writes a fitted stacked overview, per-level slices, a zoomed inspector frame and a census `manifest.json`. Unlike the in-game map it renders **ground truth**, because its job is to audit the solver rather than to be a survivor's sketch.

- **Tactics browser distribution:** `tactics_lab` has a validated embedded-catalog path whose cells, rooms, composition, and simulation hash match the filesystem loader. Its WebGL2 WASM shell uses one-finger tap/drag and two-finger pan/pinch, moves a compact primary-action dock below the physically inset map on portrait displays, and ships with repository build plus private-LAN gzip serving scripts. Touch activation computes its route directly from the tapped cell (never mouse-hover state), retains visible tap feedback, and keeps secondary gear/view/spectate controls behind a touch-sized disclosure. The expanded dock accepts vertical finger scrolling, exposes a dedicated swipe handle and a bottom close action, and never leaks the gesture into map movement. The setup screen uses 44-pixel slider hit regions and 52-pixel switches/presets for thumb-first configuration.

## Browser Lab Delivery (`deploy/labs/`)

- **Container boundary:** A pinned nginx image serves a mobile lab index, the tactics WASM bundle at `/tactics/`, immutable compressed browser assets, and `/healthz`. Additional browser labs extend this same path-oriented image instead of acquiring one-off servers or ports.
- **CI/CD boundary:** `.github/workflows/lab-web.yml` runs native tactics tests and the pinned WASM build on GitHub-hosted runners, retains the bundle as a workflow artifact, and publishes immutable commit plus `edge` images to GHCR after `main` succeeds.
- **LAN host boundary:** `compose.yaml` keeps the static service isolated from simulation, while a systemd oneshot/timer pulls `edge` from the Ubuntu host and health-checks any update. No public-repository workflow executes on the LAN server; nginx/openresty remains the only browser-facing routing layer and Portainer can observe the externally managed Compose service.

### Match Rules & adversarial Systems
- `competition_lab` & `director_lab`: Round standings, exit gates, collapse pressure, catch-up zones.
- `guardian_ai_lab` & `hazard_lab`: "Weeping-angel" style guardian AI pathing, two-player machinery gates.
- `replay_lab` & `match_replay`: Recording lockstep inputs, tape playback overlays, replay seek UI.

### Networking
- `network_lab` & `net_match_lab`: Simulated packet drop/jitter repair, lockstep synchronizers, live multiplayer match sessions.
- `lan_lab`: Resettable real-UDP loopback proof using the production dedicated server and client. It exposes the four stable 2v2 seats, ready/countdown state, compatibility handshake, authoritative server-thread lifecycle, and clean `R` reset.

## Authoritative LAN Server (`server/`)

- **`server/src/lib.rs`:** Bevy-free 60 Hz session host shared by dedicated and listen-server modes. It owns a host-configured N-team lobby capped at sixteen seats, bot fill/takeover, reconnect reservations, content/input compatibility, canonical hex simulation, retained authoritative command/digest history, late-join synchronization, and one-shot desync replay.
- **`server/src/main.rs`:** Headless `observed_server` executable with bind address, server name, minimum-human count, seed, tile-directory, and discovery flags.

### Presentation & Asset Integration
- `oga_25d_lab`: Proof surface for the 2.5D OpenGameArt intake metadata pipeline, showcasing directional actors, gameplay objects, animated decorations, and LAB texture samples with a debug metadata overlay and billboard vs directional toggle.
- `lighting_lab`: Nine static procedural dioramas isolating liminal registers (directionality, brightness, scale, repetition, fog, bloom, shadow quality) for lighting design validation and relative-luminance corridor audits.

---

## Assembled Game (`game/`)

The `game` package builds the final playable binary. It acts as an integration layer, composing the production crates and proven lab systems. Refactor Arc G (2026-07-02, see [docs/refactor_game_arc_plan.md](docs/refactor_game_arc_plan.md)) replaced the old flat `screens::*` grab-bag with an explicit-imports layout — presentation (`view`) reads simulation (`sim`), never the reverse, and no module re-exports its submodules with a glob:

- **`game/src/main.rs` & `lib.rs`:** `main.rs` is a one-line binary entrypoint; `lib.rs` owns Bevy app/plugin composition, the `GameState` state machine (Splash → Main Menu → preset Play/Advanced or LAN → Loading → `HexWfc` → Results/Replay, with Loadout and Settings side routes), and the top-level camera/light setup. `Match` and `FullWfc` are direct-test regression states only. `arch_check.rs` is a `#[cfg(test)]`-only ratchet: source-scanning tests fail if a glob re-export, a non-test `use super::*`, a simulation-to-presentation import, an oversized canonical WFC file, or the retired global menu action/cursor creeps back in.
- **`game/src/hex_wfc/`:** Canonical Play adapter over `observed_match::hex_wfc`. `launch` caches and validates authored content before constructing an exact/nearby deterministic match; `loading` owns immutable request identity, asynchronous preparation, stale-result rejection, retry/cancel, the LAN ready/start barrier, and a one-shot simulation hand-off; `sim` owns fixed-step versioned commands, replay/results, team knowledge, objective-aware local/server bots, local play, and authoritative LAN-frame replay/desync recovery; `overlay` gives onboarding, survivor-map, and pause explicit focus/input/simulation policy. `view/` admits only a safe nearby neighborhood on entry, then adds/removes cell parents under per-frame budgets with hysteresis and room-footprint residency; it applies changed-cell geometry using composition-aware, register-keyed `observed_style` materials, room↔corridor fog/lighting contrast, saved player FOV, and a deterministic key-light budget. `entities` projects labeled typed room mechanisms; `lantern` projects the procedural caged anchor lantern and physical Guardian. `view/map/` is the Tab-toggled full-screen isometric survivor map (Arc O Phase 105, promoted from `iso_observer_lab`), drawing shared teammate knowledge only, on its own render layer with its own key. It shows what the tiles *compose* on five non-competing channels: colour is the district, height is the archetype, footprint width separates room/hallway/vertical (rooms meet with no seam so a multi-cell room reads as one space), link bars mark every port pair seen open from both sides, and a cap marks a cell the team is holding. You/exit cells are recoloured to signal tier, while entered or locally surveyed rooms gain literal function labels. Stability and room identity are derived only from leak-free team knowledge — never the global observation frame, which would surface rival positions or undiscovered functions.
- **`game/src/full_wfc/`:** Demoted Arc-K square regression adapter, enterable by direct tests/evidence fixtures only. Its versioned simulation, shell, audiovisual projection, and regression suites stay intact; every player-facing launch targets `hex_wfc`.
- **`game/src/sim/`:** Simulation-side Bevy resources — no rendering, UI, or asset types. `director.rs` holds `MatchDirector`, the single owner of the deprecated live place-match plus its elimination series (`tick`, `run_to_completion`, `outcome`, spectator pumping, and forcing/suppressing scripted rounds for regression evidence). `state.rs` holds the retained teleport/body/intent/spectator resources (`TeleportState` [DEPRECATED], `SpectatorBot`, `MatchIntent` [DEPRECATED], `ItemIntent` [DEPRECATED], `MatchPaused` [DEPRECATED], etc.). `nav.rs` is the pure brain→`Nav` projection used for bot pathing.
- **`game/src/view/`:** Presentation building blocks that read `sim` but never write it. `theme.rs` holds the menu/HUD colour palette and UI bundle helpers. `assets.rs` is the drop-in asset slot registry plus `MatchAssets::load`; `environment.rs` is the shared repeating-texture, manifest-scene, and world-unit-UV cuboid support used by both Place and full-WFC rendering. `components.rs` holds presentation markers and feedback-state resources (camera/sun tags, teleport animation state, etc.).
- **`game/src/layout.rs`:** Game-owned spatial constants for the teleport place model — `PLACE_TILE`, `HALL_WIDTH`, `WALL_HEIGHT` — now sourced independently of the abandoned `observed_match::maze` tile grid. [DEPRECATED]
- **`game/src/content.rs`:** Production loader/projection for the committed `assets/content/content_manifest.json` plus canonical TrenchBroom convex bakes. It builds hybrid `ArenaSpec`s from the shared generated aperture boundary plus authored Gantry/Colonnade interior hulls; Rapier is canonical and there are no runtime physics/geometry selectors. The same manifest promotes only the Kenney CC0 gate and cable bundle into played threshold dressing.
- **`game/src/play_setup.rs` & `settings.rs`:** Separate persisted domains. `PlaySetupDraft` owns preset/custom match intent, validates the 16-seat cap, and records the finalized launched roster for honest results copy. `UserPreferences` owns audio/display/control/onboarding choices, normalizes corrupt or out-of-range data, migrates legacy saves, and writes under the platform user configuration directory (or `OBSERVED2_CONFIG_DIR`).
- **`game/src/lan.rs` & `screens/lan.rs`:** Persistent LAN client/listen-host ownership plus broadcast discovery, exact direct-address focus/editing, connection transitions, paged server presentation, and authoritative roster projection. `screens/lobby.rs` offers readiness and team requests across up to 16 seats, labels each open seat as human-required or bot-filled, and starts only through the idempotent ready/start generation barrier; server state remains the source of truth.
- **`game/src/screens/`:** State-scoped frontend composition. `widgets/` owns semantic focus, pointer/keyboard/controller parity, accessibility, disabled states, visible non-colour focus, and feedback; each screen owns a typed local action observer. `main_menu`, `play`, `loading`, `settings`, `loadout`, `lan`, `lobby`, `onboarding`, `results`, and `replay` implement the player-facing hierarchy and clean up on exit. `menu.rs` now owns Splash only; the isolated-Place modules remain regression fixtures.
- **`game/src/evidence/`:** Every opt-in `OBSERVED2_*` pipeline, consolidated under one tree. `capture/` holds the showcase/tour/bot-POV screenshot and GIF drivers. `audit.rs` + `snapshot.rs` + `tags.rs` are the visual audit (staged inspection scenarios, world → `observed_diagnostics` collectors, and the presentation-facing marker components the audit identifies visuals by). `driver.rs` holds helpers shared by every scripted driver. All of it is a no-op in normal play.
- **`game/src/teleport/`:** Discrete room/hallway footprint building (`geom.rs`), shared threshold-boundary partitioning (`aperture.rs`), navigation/connection tracking (`nav.rs`), and crossing/structural projection (`transition.rs`). Rendering, Rapier collision, trim, previews, and validation consume the same off-centre/elevated aperture plan. The hallway library includes the two-level Gantry jump-map hall and WFC-selected six-level Wellshaft.
- **`game/src/map_catalog.rs` & `map_validation.rs`:** Active map selection (`OBSERVED2_MAP`, defaulting to the procedurally generated `liminal_wfc_v1`; `dev`/`sector_relay_v1` selects the authored nine-room fixture) with validated `MapSpec` builder plumbing, an in-process per-`(map, seed)` build cache (generation is expensive; the test suite enters the Match ~150 times), plus pure semantic-map geometry audits.
- **`game/src/camera.rs` & `bot.rs`:** Shared viewport math (first-person, spectator, preview) and dynamic navmesh/grid-fallback bot automation for walkthrough screenshots and the `Spectate AI` body.
- **`game/src/navmesh.rs`, `guardian.rs`, `items.rs`, `keystones.rs`, `flow.rs`, `rivals.rs`, `tacmap.rs`, `maze.rs`, `hallway.rs`:** dynamic navmesh generation from the current place geometry; the "weeping-angel" guardian AI in-match; presentation-layer droppable items (anchor torch, etc.); the keystone-gated exit inventory check; the pure career/flow model tying match → progression; presentation-only rival avatars shown when sharing a room; the toggleable tac-map overlay; the per-hallway interior maze generator (randomized-DFS + braid); and the authored teleport-hallway pieces themselves.

---

## Content Tools (`tools/`)

- **`content_baker`:** Deterministic command-line TrenchBroom/Quake `.map` baker. It selects a typed `observed_module`, converts every brush into a stable convex hull, validates the result through `observed_content`, and writes canonical JSON suitable for fingerprinting and immutable production inclusion.

- **`composition_studio`:** Where a person authors the WFC composition profile and watches the facility answer. Its viewport is the **authored deck** — real catalogue hulls under a named wall projection (`C` cycles cutaway / partial / full, where partial is the third-height deck `tactics_lab` reads a board with), tinted by architecture register and lit by a key plus one practical pool per visible district, sharing `observed_style::iso::light` with `tactics_lab`. Pins, the baseline compare and the replay are treatments on that solid rather than line work. The solver's schematic is an overlay on `G`, forced on with a stated reason when there is no projected catalogue, while a replay runs, or in the neighbourhood explorer. Tunables, pins, coverage, baseline A/B compare, a first-ring neighbourhood explorer that re-opens domains through the solver's own AC-3, and a headless JSON script path for deterministic evidence PNGs. The panel is non-modal by design — keys are routed by region ownership rather than by blocking the viewport, because the loop is *change a value and watch it answer*. It now also replays the solve it drew: the traced step log it always computed is folded back into per-cell state, so the collapse can be played, stepped, and scrubbed, with collapsed/open/contradiction counts read at the cursor and a hovered cell reporting its domain at collapse (explicitly distinguished from the neighbourhood explorer's narrower number). Seeds are free values with the five pinned presets kept as bookmarks, and `Ctrl+Z` steps back one edit rather than discarding everything since the last save. Sibling binary `module-studio` views one authored module and its neighbour ring.

---

## Completed Refactorings & Modularizations

To keep code easily consumable by AI agents and developers, large singleton files have been refactored into smaller, focused modules:

### 1. `game/src/teleport.rs`
- **Status:** Completed.
- **Details:** Refactored the 2,006-line singleton file into a folder module `game/src/teleport/` split by SOLID responsibility:
  - `mod.rs`: Place/Gap types, constants, and exports.
  - `geom.rs`: Room footprint polygon builders, S-bend chicane/maze/colonnade hallway geom generation, and analytic polygon containment.
  - `nav.rs`: Navigation and connection tracking.
  - `transition.rs`: Crossing detection math, spatial 2D/3D portal alignment transforms, spawning, and Bevy FpsArena construction.
  - `test.rs`: Unit and integration test suite.

### 2. `crates/observed_match/src/hybrid.rs`
- **Status:** Completed.
- **Details:** Refactored the 1,156-line orchestrator file into a folder module `crates/observed_match/src/hybrid/`:
  - `mod.rs`: Entrypoints, re-exports, and base types.
  - `match_state.rs`: Player positions, active places, targets, and round ticking.
  - `round_step.rs`: Simulation ticking, action application, match outcomes, and objective/escaped/escaped_count calculations.
  - `replay.rs`: Record, play, replay tapes, frame ticks, and local client action simulation.
  - `test.rs`: Competitive round matching and replay tapes unit tests.

### 3. `labs/topology_lab/src/lib.rs`
- **Status:** Completed.
- **Details:** Refactored the 1,024-line monolithic test runner into a folder module `labs/topology_lab/src/`:
  - `lib.rs`: Entrypoints and re-exports.
  - `model.rs`: Graph nodes, hallways, links, slot IDs, and PRNG.
  - `logic.rs`: Connectivity validation, ASCII parser, and decoherence links shuffler.
  - `app.rs`: Bevy lab Feasibility prototype camera, movement simulation, text UI rendering, and quantum shuffle keyboard events.
  - `test.rs`: Connectivity tests and decoherence shuffling tests.

### 4. `crates/observed_progression/src/session.rs`
- **Status:** Completed.
- **Details:** Refactored the 1,022-line file into a folder module `crates/observed_progression/src/session/`:
  - `mod.rs`: Entrypoints, phase state machines, time counters, and re-exports.
  - `lobby.rs`: Account, client connection status, lobby formation, team assignments, and rematch state tracking.
  - `matchmaking.rs`: Matchmaker rating calculations, queue enqueuing/dequeuing, region pairing, and ticket matching.
  - `test.rs`: Matchmaker rating tests, matchmaking queue pairing, and lobby rematch flows.

### 5. Refactor Arc G — Game-Layer Architecture Cleanup (2026-07-02)
- **Status:** Completed. Full record: [docs/refactor_game_arc_plan.md](docs/refactor_game_arc_plan.md).
- **Details:** `game/`'s flat `screens::*` god-module (710-line `screens.rs`, 8 glob-reexported submodules, `use super::*` everywhere) was dissolved into the explicit-imports `sim`/`view`/`layout`/`screens`/`evidence` layout described above (`screens.rs` alone went 710 → 177 lines); a new `game/src/arch_check.rs` ratchets the no-glob-reexport / no-super-glob / sim-never-imports-view rules as tests. `MatchDirector` (`sim/director.rs`) replaced the four parallel, loosely-correlated match models (`MatchRuntime`'s live match, the wall-clock-timer-driven `SeriesRuntime`, the spectator `TeamplayMatch` pump, and a second headless `flow::play_match()` path) with one owner and one `outcome()` resolution rule, pinned by a headless-vs-interactive characterization test. The place renderer (`screens/place/mod.rs`, 1,288 lines) was split into `factory`/`monitors`/`animate`/`shell` with `mod.rs` down to 42 lines, and its `Box<dyn SpawningStrategy>` / `GatewayPolicy` pattern bloat was flattened into plain functions and a `ThresholdStyle` data struct. The match's resource lifecycle (13+ resources hand-removed in `cleanup_match_resources`) was enumerated exactly once (`match_runtime/session.rs`'s `for_each_match_resource!`), which surfaced and fixed four resources (`Guardian`, `ActionLog`, `TeleportAnimation`, `LastTeleportPad`) that had been leaking across match exits; a no-leak test now guards the full set. The two hand-rolled evidence pipelines (`capture/` and the old `diagnostics.rs`) were consolidated under `game/src/evidence/` behind shared `MatchDirector` scenario-staging methods. The dead `game/src/wfc_maze.rs` (zero callers, the game's only reason to depend on `ghx_proc_gen`) was archived to `labs/wfc_proc_gen_lab/src/hallway_wfc.rs` rather than deleted, in case WFC returns for map generation.
