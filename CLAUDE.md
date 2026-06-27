# CLAUDE.md

Day-to-day working reference for Claude Code in this repository. For the
long-range design rationale see [AGENTS.md](agents.md); for the current
workspace structure see [Catalogue.md](Catalogue.md); for what is done and what
comes next see [ROADMAP.md](ROADMAP.md). This file is *how to work here* and
reflects the current state of the workspace.

## What this is

**Observed 2** is an experimental PC game built with **Rust + Bevy 0.18** â€” a
competitive traversal game set in an out-of-control megastructure whose
connections change when unobserved. Because that concept is unproven, development
proceeds through small, independently testable prototypes ("labs"), not a
top-down build of the whole game.

**Direction:** the 2D labs were the *proof of concept*; the target is a **3D
first-person game**. This works because the game's whole simulation layer is
dimension-agnostic â€” the observe/decohere graph, constraint spine, competition,
director (AI adversary), replay, routes, and incentives are pure logic with no
rendering dependency, so they carry into 3D unchanged. The 2D schematic/spectator
becomes the in-game map/spectator. What gets rebuilt for 3D is the *projection*:
the player controller and the world geometry/presentation. See ROADMAP.md (the
"first-person pivot" + completed FPS/Hybrid arcs) before selecting the next phase.

The reliable **2D technical foundation** (app/menu states, input abstraction,
movement, climbing, interaction, modular rooms, carryable equipment, team
contention, debug tooling), every **higher-level system** (observation &
decoherence, mutable-graph constraints, competition, the facility director,
replay/spectator, routes, incentives, cooperative hazards, deterministic
lockstep networking, matchmaking/session formation), and the **integration arc** (mutable â†’
competitive â†’ replayed match) are **complete and proven**. The **first-person
path and **Hybrid maze arc** are complete: deterministic movement, continuous
visibility, safe off-camera replacement, graph-backed 3D modules, a full
first-person competitive match, a concrete spatial maze, live corridor rerouting,
and the same complete match played and replayed in that shifting maze.

## Workspace Orientation

Review [Catalogue.md](Catalogue.md) before selecting files. It is the
authoritative overview of the refactored workspace: promoted production crates,
lab/debug projections, the assembled `game`, docs, assets, evidence screenshots,
and per-file ownership. Treat any compact tree in this file as an orientation aid
only; if it disagrees with `Catalogue.md` or `Cargo.toml`, the catalogue and
manifest win.

```text
/
|-- agents.md                 # full design doc and working contract
|-- CLAUDE.md                 # Claude-oriented working reference
|-- Catalogue.md              # current structure overview and per-file catalogue
|-- ROADMAP.md                # status and forward plan
|-- Cargo.toml                # workspace root
|-- crates/                   # promoted production models (observed_* plus player_input)
|-- labs/                     # independently runnable debug projections/prototypes
|-- game/                     # assembled player-facing loop
|-- assets/                   # optional drop-in assets
`-- docs/evidence/            # screenshot evidence
```

AGENTS.md sketches a larger historical multi-crate split (`app_core`, `input`,
`player`, `world`, `ui`, `debug_tools`). That sketch is not the exact current
layout. The refactor promoted a broader set of production crates under `crates/`
(`observed_*` plus `player_input`); `Catalogue.md` enumerates their ownership and
the labs that project them. Keep lab-specific presentation/debug code in the lab;
promote reusable logic only after a second consumer needs it.

## Commands

Every lab launches independently:

```powershell
cargo run -p movement_lab      # ...or any lab listed above
```

Capture a lab's evidence screenshot (renders the showcase, writes the PNG, exits):

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/<lab>.png"; cargo run -p <lab>
```

Before claiming any work is complete:

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets   # address warnings; don't suppress without reason
cargo test --workspace                   # or: cargo test -p <lab>
```

Do **not** claim completion solely because the project compiles. Verify the
affected lab and confirm that resetting/exiting it removes all of its entities
and resources. Each lab README documents its success conditions and a manual
verification procedure â€” use it.

## Lab conventions

Every lab follows the same shape (see `movement_lab` or `climbing_lab` as
references):

- `src/main.rs` is a one-liner calling `<lab>::run()`.
- `src/lib.rs` defines a Bevy `Plugin`, a public `run()` that builds the `App`,
  an optional `OBSERVED2_CAPTURE` screenshot path, and `#[cfg(test)]` tests.
- Pure logic lives in its own module (`model.rs` / `simulation.rs` / `tape.rs`,
  etc.) so it is unit-testable without a running app.
- Tests cover both pure logic and Bevy lifecycle (entity counts across a repeated
  reset loop, using `MinimalPlugins` + targeted plugins).
- A debug overlay visualizes the otherwise-invisible state, with a
  `[PASS]`/`[FAIL]` entity-health line.

A lab must: test one primary technical question, launch independently, **reset
without restarting the app**, avoid depending on unfinished systems, and define
observable success/failure conditions. Common keys: `R` reset, `F1` toggle debug,
`1`â€“`4` select a player/team.

## Feasibility-lab & integration pattern

- Each deferred higher-level system gets its **own isolated, pure-logic
  feasibility lab** proving one question (validity, determinism, the rule holds)
  before it touches the real game.
- A feasibility lab may **reuse a proven lab as a library** when the question
  genuinely builds on it (e.g. `constraint_lab` reuses `observation_lab`'s graph;
  `replay_lab` records a `competition_lab` match). **Integration** phases reuse
  the proven labs wholesale (e.g. `facility_sandbox` reuses `climbing_lab`'s
  controller).
- Keep lab-specific identifiers local; promote one into `observed_core` only when
  a second system actually shares it.

## Architectural rules (carry into every change)

1. **Separate input from behavior.** Gameplay systems never read keyboard/gamepad
   directly. Hardware input becomes the shared `PlayerIntent`; systems consume
   that. This keeps controllers, bots, recorded/replayed input, and network
   clients possible.

2. **Separate simulation from presentation.** Logical state must not depend on
   sprites, cameras, or UI. Presentation is a *projection* of simulation state â€”
   a room exists logically without being rendered; equipment state stays valid
   while its visuals are despawned; replay/spectator renders from replayed state.

3. **Stable domain identifiers, not Bevy `Entity` values, for identity.**
   `PlayerId(u16)` lives in `player_input`; `RoomId`, `PortId`, `EquipmentId`,
   `TeamId` are in `observed_core`. Add canonical IDs there only when a system
   first needs one; keep lab-local IDs (e.g. `DoorId`, `StationId`) local until a
   second consumer shares them.

4. **Explicit, validated connections.** Rooms and equipment connect through
   authored, typed ports/sockets; connections are validated (type, position,
   facing, occupancy), never inferred from approximate placement.

5. **Data-driven definitions.** Topology and gameplay metadata live in data, not
   scattered through spawning systems. Hand-authored templates first; no
   procedural mesh generation.

6. **Multiplayer-shaped from the start.** Every system supports up to four local
   players/teams. No global single-player resources, no queries assuming one
   player, no hard-coded keyboard ownership.

7. **Keep the simulation deterministic.** Seed any RNG and resolve contention in
   a stable order (by `PlayerId`/`TeamId`). Determinism is now load-bearing â€” it
   powers replay (`replay_lab`) and will power networking â€” so a model's outcome
   must depend only on its inputs, not iteration order or wall-clock dt.

8. **Visual identity is code-as-art, through one shared module.** Don't author or
   source textures/meshes; generate the look from primitives + color/emission/
   light/fog. The art direction is **neon-noir**, and all visual work goes through
   the shared `style` module (semantic state â†’ visual treatment, proven in
   `style_lab`) â€” presentation never hard-codes ad-hoc colors. Honor the
   **Legibility Contract**: gameplay signals always punch through the atmosphere,
   and every on-screen state has a documented meaning. See the North Star in
   [agents.md](agents.md).

## Coding conventions

- Standard Rust formatting (`cargo fmt`); edition 2024; resolver 3.
- Clear names over terse abstractions; single-responsibility systems.
- Components for entity-local state, resources for world-level state,
  events/observers for meaningful transitions.
- Avoid oversized systems mixing input/sim/render/audio; avoid premature plugin
  fragmentation and speculative abstraction. Prove the smallest useful version
  first.
- Document invariants and non-obvious assumptions. Don't suppress warnings
  without explaining why. Don't present placeholder systems as finished.

## Dependencies

- Prefer the Rust std library and Bevy's built-in systems. Collision/movement has
  been hand-rolled and testable so far. Phases 20 and 23 proved the existing
  fixed-timestep AABB controller sufficient for authored 3D modules and graph
  doorways; revisit a vetted physics crate only if later mesh/capsule complexity
  creates substantial risk.
- Bevy is pinned at `0.18.1`, `default-features = false`. 2D labs add
  `["2d", "png"]`; 3D labs (the first-person path) add `["3d", "png"]` (`png`
  enables the screenshot capture). **Do not upgrade Bevy or other major deps**
  unless requested or required by the task.
- Add a major dependency (physics, networking, UI, serialization) only when it
  removes substantial technical risk â€” explain the benefit and maintenance cost
  first.

## Debugging requirements

Invisible mechanics need visible debug representations. Labs should visualize the
relevant subset of: player intent, velocity, ground/collision, climb detection,
interaction range and target, room bounds/ownership, port types and connections,
moving-platform attachment, equipment ownership/socket state, observation &
decoherence, route connectivity, team standings, the collapse line, replay
cursor, and entity counts before/after reset. Prefer an on-screen overlay over
console output.

## Non-goals (unless explicitly requested, or the ROADMAP reaches them)

Do not build progression/cosmetics, full rope physics, universal surface
climbing, complex enemy AI, combat, or polished art pipelines yet. Phase
16's networking scope is the proven lockstep protocol and UDP packet compatibility;
Phase 17 adds deterministic queue/lobby/session formation but not production
account services, relay allocation, authentication, or deployed discovery.
Do not refactor the whole workspace while implementing one lab, and do not create
abstractions for hypothetical future requirements.

**Procedural geometry** is no longer a blanket non-goal: the **Hybrid maze arc**
(ROADMAP Phases 25â€“27) scopes a *deterministic, seeded* maze generator that embeds
the proven room graph in navigable space (rooms + walkable passages that re-route
when unobserved). Keep it authored/seeded and testable â€” it is the spatial embedding
of the existing graph, not free-form generation, and the simulation brain is
unchanged.

The higher-level systems are **no longer deferred at all** â€” observation/
decoherence, mutable-graph constraints, competition, the facility director,
replay/spectator, routes, incentives, cooperative hazards, and deterministic
lockstep networking plus matchmaking/session formation each have a feasibility lab; the
integration arc (mutable â†’ competitive â†’ replayed match) is complete; and the
**first-person path** (FPS arc, Phases 19â€“24) is complete end-to-end â€” a first-person
competitive match that records and replays exactly. The **Hybrid maze arc**
(Phases 25â€“27) is also complete: the graph is a generated walkable maze, passages
reroute safely when unseen, and the full match plays and replays in that concrete
space. The **networked first-person match** (Phase 28, `net_match_lab`) is complete:
`network_lab`'s lockstep over a hostile transport carries `fps_hybrid_match_lab`'s
deterministic match so two peers reconstruct the identical match/maze/pose. Its
`LiveNetMatch` variant powers the assembled `game`, whose **Match is now played in
first-person 3D** (you walk the maze; each crossed round replicates to a remote peer
over the lockstep). The remaining work is production hardening, not feasibility; any
genuinely new system must still get its isolated feasibility lab before being folded
into integration.

## Working method

Before changing code: review [Catalogue.md](Catalogue.md) for the current
workspace structure, then read the relevant crate/lab and the active ROADMAP
phase; identify the smallest system needed; check whether an existing abstraction
(or a proven lab) already owns the behavior; state any assumption that affects
architecture.

While changing code: keep the change scoped to the milestone; preserve the
input / simulation / presentation separation; add debug visibility for new
invisible state; add or update tests; avoid unrelated refactoring.

After changing code: `cargo fmt`, then run relevant tests and Clippy; launch or
validate the affected lab (capture a screenshot for visible-mechanic/integration
work); report what changed, what was tested, known limitations, and the next
smallest step.

