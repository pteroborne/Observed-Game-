# AGENTS.md

## Project Overview

This repository contains an experimental multi-team LAN game built with **Rust and
Bevy**.

Teams ascend a shared, out-of-control megastructure whose architecture is played
and rewritten by dedicated Architect players while first-person Observers explore
it. Connections may change when unobserved, Guardians jail exposed Observers, and
falling through the surviving facility into true void turns a player into an
operator for the Rogue AI. Players cannot directly harm opponents; competition is
expressed through architecture, observation, doors, equipment, traversal, and
environmental systems.

## Project Structure Catalogue

Before selecting files to change, review [Catalogue.md](Catalogue.md) for the current project structure. Use it for orientation before drilling into a specific crate, lab README, or roadmap phase.

## North Star

**Canonical direction changed 2026-09-07.** The assembled race, its labs, and its
LAN/WFC infrastructure remain the technical foundation, but the game now being
built is **Architect Ascent**. The complete rules are in
[docs/architect_ascent_design.md](docs/architect_ascent_design.md); where an older
gameplay plan conflicts with that document, the new design governs. The two active
goals are:

### Goal 1 — Make a *fun* game

Fun here is a specific combination, not a vibe: **cooperative *and* competitive**
play expressed through four interlocking roles and pressures —

* **Architect play** — one dedicated player per team holds five cards and adds or
  replaces known, mutable facility tiles. Card topology, district, orientation,
  placement cooldowns, and WFC constraints turn route construction into the main
  strategic game.
* **Observation** — first-person teammates discover the board their Architect may
  use and temporarily protect tiles and Guardians by looking at them. Do not
  pre-solve or globally reveal the facility.
* **Traversal and rescue** — height, unsafe architecture, physical equipment,
  imprisonment, and team rescues make movement itself consequential rather than a
  walk between decisions.
* **Instability and opposition** — locally legal card plays may create WFC
  contradictions that visibly retract a floor toward void. Rival Architects,
  shared doors, Guardians, and the player-operated Rogue AI contest every route
  without direct combat.

The competitive frame is a race to bring every remaining loyal Observer to the
summit. The cooperative frame is the information loop between an Architect and
one-to-three Observers: explore, report, build, hold, rescue, and ascend. Existing
WFC, observation, traversal, Guardian, equipment, map, and LAN systems are proven
ingredients, but the combined card-driven loop must be proven in a dedicated lab
before production integration. The first proof is Rogue-first: one human Rogue
Architect places tiles to help autonomous Guardians reach autonomous Observers.
Every non-human decision-maker uses a deterministic behavior tree, and an optional
Rogue Architect tree can replace the human for unattended runs while emitting the
same card commands.

#### The shared facility

All teams build and traverse one continuous, multi-floor WFC facility. Every floor
has one district, and higher floors draw increasingly unsafe authored tiles. A
district-matching ascent-room card creates access to the next floor. A vertical,
central prison core crosses every floor and never collapses.

Rooms remain decision, cooperation, rescue, and machinery beats; corridors and
unsafe tiles remain traversal, commitment, and risk beats. This tension/release
distinction still governs authored content, but card placement and contradiction
pressure—not a precomposed objective route—now drive the match.

#### Observation, anchors, and doors

Base thresholds remain open physical connections. Observation temporarily freezes
tiles, connections, and visible Guardians; occupancy protects the occupied tile; an
anchor freezes its exact connection durably. Only geometry that is neither
observed, occupied, anchored, nor part of the prison core is a legal rewrite target.

A door is separate deployable equipment placed from the Architect's mixed hand.
Any loyal Observer may operate any deployed door. Open doors are traversable, pin
their current connection, and permit observation through them. Closed doors block
passage and sight, release the hidden side to mutation and Guardian action, and are
lost if a rewrite removes their host threshold. An open door is contestable; an
anchor remains the unattended permanent lock.

Teleportation remains explicit equipment or room functionality. Team pads and
stations require both endpoints to be reached physically and never reveal or skip
an unexplored floor. The former isolated-Place/preview match remains a deprecated
regression fixture and must not be used for new production features.

#### Pressure, power, and the Observer's hands

Observation alone leaves the first-person seat passive — look, stand, anchor,
operate, walk. Three canon mechanics ([the design
doc](docs/architect_ascent_design.md)) give it active verbs:

* **Disturbance releases minor Guardians.** Architecture changes raise a per-floor
  disturbance meter that decays with time and releases a wave at each threshold.
  Contradictions and retractions raise it far more than clean placement, and height
  raises the ceiling. It is a budget the Architect spends, never a per-placement fine.
* **Minor Guardians are not frozen by observation.** That immunity is deliberate:
  majors are a *looking* problem, minors are a *doing* problem. Minors belong to the
  facility, obey no Rogue directive, and chase the nearest detected Observer of any
  team.
* **Each floor has one contested generator.** Unpowered, a floor loses recharge, door
  operation, ascent rooms, pads, and — decisively — observation *range*: what cannot
  be seen cannot be frozen, so an unpowered floor is nearly all mutable and its
  majors nearly all awake. The Legibility Contract still binds. Darkness costs range,
  never legibility, and critical signals keep a documented self-lit minimum.
* **The kinetic tool, not a gun.** Observers carry a short-range push/pull that
  damages nothing and cannot touch a rival. It kills minors with the architecture —
  void, ledges, retracting tiles — so the Architect stays central to first-person
  survival. Charge is finite and restored only at Architect-placed stations on a
  powered floor: generator → station → tool → the only answer to the horde. Shove
  resolution is fixed-tick simulation, never authored physics.

An **emergency requisition** card refills a hand instantly and publicly, at the price
of one major Guardian on the floor its Observers occupy. The redraw *room* remains the
free version, earned by Observers reaching it in person.

#### Collapse, jail, and the Rogue AI

A card play need only fit its selected boundary locally. Wider WFC contradictions
are accepted and telegraphed, then retract implicated tiles toward void until a
compatible play repairs the constraint. A completely retracted floor is permanently
closed to new placement; only the prison core remains.

Guardian catches send Observers into the prison maze. Prisoners may find its hard
internal exit, while teammates can create an easier door or portal rescue after
physically reaching the core. A fall lands on lower surviving geometry when
possible. Falling through the entire surviving stack into true void irreversibly
converts the Observer into a shared Rogue operator with a disruption hand and
cooldown. Loyal teams win at the summit; the Rogue faction wins when every remaining
loyal Observer is simultaneously jailed.

### Goal 2 — Develop effectively *with agents*

Lean into what an LLM agent is good at and away from what it is not.

* **Reusable, testable modules.** Keep the lab discipline: break each concept into a small pure module that is simple to code, understand, test, and reuse (the way `observed_core` and `player_input` are shared). Protect it.
* **Code-as-art over authored assets.** Visual identity is **generated from code**:
  geometry from primitives, with **color / emission / light / fog as a deliberate
  visual language**. The chosen direction remains neon-noir, now organized around
  original geometric constructs: spherical eye Observers, pyramidal Guardians,
  and primitive-based structural decoration. "Modron" is an inspiration reference,
  not shipped terminology or copied design. The result stays verifiable through the
  existing `OBSERVED2_CAPTURE` screenshot loop.
* **The Legibility Contract (a hard rule).** Atmosphere never hides information. Gameplay-critical signals — your path, threats, interactables, and other actors — must always punch through the neon-noir fog/bloom at a guaranteed brightness and contrast. Every on-screen state must have a documented meaning (a legend); no unlabeled coloured markers.

The visual language lives in **one shared, tested module** — the `style` module, proven in `style_lab` — that maps *semantic state → visual treatment*. Presentation code asks the module how to draw a thing; it never invents ad-hoc colours.

* **Evidence capturing with FFmpeg.** To make visual evidence in walkthroughs more interactive and easier to review, agents can use `ffmpeg` to compile sequential screenshot folders (e.g. `docs/evidence/bot_pov/bot_pov_*.png`) into a single high-quality loopable animated GIF (e.g. `docs/evidence/bot_pov/bot_pov.gif`) using a palette filter, and embed it in markdown walkthrough files.

## Development Philosophy

### Build small test applications

Major systems should first be implemented as isolated labs or examples.

Each lab should:
* Test one primary technical question
* Launch independently
* Reset without restarting the application
* Include clear debug visualization
* Avoid dependencies on unfinished game systems
* Define observable success and failure conditions

Do not attempt to build the complete game as the first playable prototype.

### Prove systems before generalizing them

First make the smallest useful implementation work.

Only extract a generalized framework after:
1. The prototype functions correctly.
2. Its requirements are understood.
3. Its likely reuse has been demonstrated.
4. Its failure cases have been identified.

Avoid speculative abstractions.

### Prefer readable constraints over hidden complexity

Technical constraints may become game rules.

Examples:
* Use explicit climbing markers instead of detecting every climbable surface.
* Use grapple sockets instead of full rope physics.
* Use authored room ports instead of arbitrary procedural geometry.
* Use visible room-transition phases instead of instantaneous asset replacement.
* Use discrete graph connections instead of requiring continuous physical simulation.

A limitation is acceptable when it is consistent, readable, and capable of producing meaningful gameplay.

### Compile-Time and Linking Speedups

To optimize build and link times during active development (especially with multiple parallel worktrees):
* **Disable dependency debug info**: Add `debug = false` to `[profile.dev.package."*"]` in `Cargo.toml`.
* **A shared cache fills up, and it is never the logs.** Cargo does not garbage-collect:
  stale hashed artifacts accumulate in `debug/deps` forever. When the cache filesystem
  gets tight, prune by age rather than hunting for runaway logging — CLAUDE.md has the
  commands, and the measurements behind them. Never `cargo clean` on a shared cache, and
  delete a worktree's cache as soon as its branch merges.
* **Share target directory**: Point worktrees to a central target directory using `CARGO_TARGET_DIR` or `.cargo/config.toml`:
  ```toml
  [build]
  target-dir = "O:/Observed 2/target"
  ```
* **Fast Linker (LLD)**: Configure `.cargo/config.toml` to use the Rust toolchain's LLD on Windows:
  ```toml
  [target.x86_64-pc-windows-msvc]
  linker = "rust-lld.exe"
  ```
* **Dynamic Linking**: Use the `.cargo/config.toml` development aliases (`cargo dev-run`, `cargo dev-test`, and `cargo dev-clippy`) to enable Bevy's `dynamic_linking` feature without enabling it in release builds.
* **The gate skips long instrumentation tests.** `cargo dev-test` excludes a few tests
  marked `#[ignore = "..."]`, each carrying its cost and reason. They are evidence, not
  regression cover: they print playtest measurements and assert nothing, and leaving them
  in cost ~25 minutes a run, which meant the gate stopped being run. `cargo dev-test-all`
  includes them — run it periodically, and whenever you change the simulation they
  measure. **Anything that asserts stays in `dev-test`, however slow**; if you want to
  `#[ignore]` a test with assertions in it, make the test faster instead.

## Core Architectural Rules

### Separate input from player behavior

Gameplay systems must not read keyboard or controller input directly.

Input sources should produce an abstract intent `PlayerIntent` which character systems consume. This allows the same player systems to later support controllers, bots, recorded inputs, replays, and network clients. Do not assume the existence of only one player.

### Separate simulation from presentation

Logical state should not depend on sprites, cameras, UI entities, or rendered scenes.
* A room may exist logically without being rendered.
* Equipment state must remain valid while its visuals are despawned.
* Player ownership must not be inferred from sprite appearance.
* Map and spectator views should read simulation state rather than reconstruct it from rendering entities.

### Keep the game's module flow one-way and explicit

Inside `game/`, presentation reads simulation, never the reverse: `view/` and the
screen systems may import `sim/`; `sim/` must never import `view/` or `screens/`.
State imports explicitly from its owning module — no glob re-exports
(`pub use x::*`) between modules and no `use super::*` outside `#[cfg(test)]`
modules, so every file states what it actually depends on. These rules are enforced
by the `arch_check` ratchet tests in `game/src/arch_check.rs`; if one fails, fix the
dependency direction rather than the test.

### Use stable domain identifiers

Do not use Bevy `Entity` values as persistent game identities.
Prefer domain identifiers such as `PlayerId(pub u16)`, `TeamId(pub u8)`, `RoomId(pub u32)`, `PortId(pub u32)`, `EquipmentId(pub u32)`. Bevy entities may reference these IDs, but should not replace them.

### Use explicit ports and sockets

Rooms and equipment should connect through authored, typed connection points.
Connections must be validated rather than inferred from approximate visual placement.

### Prefer data-driven room definitions

Room topology and gameplay metadata should be represented in data rather than embedded throughout spawning systems. Begin with hand-authored templates. Do not begin with arbitrary procedural mesh generation.

### Make multiplayer-shaped assumptions early

Networking is not an early milestone, but local systems must support multiple players. Avoid global single-player resources, queries assuming one player, hard-coded keyboard ownership, or camera state mixed with player state.

## Debugging Requirements

Invisible mechanics require visible debug representations.
Relevant labs should visualize:
* Player intent, velocity, ground contact, collision shapes
* Climb detection, interaction range, current interaction target
* Room bounds, room ownership, port types and alignment, active connections
* Moving-platform attachment, equipment ownership, equipment socket state
* Pending room transitions, entity counts before and after reset

Prefer a simple debug overlay over relying entirely on console output.

## Testing Expectations

For each completed system:
* Add focused unit tests for pure logic.
* Add integration tests where Bevy scheduling or entity lifecycle matters.
* Run `cargo fmt`, `cargo clippy`, and `cargo test`.
* Verify the affected lab manually, confirming that resetting or exiting the lab removes its entities and resources.

## Dependency Policy

* Prefer the Rust standard library and Bevy’s built-in systems.
* Add third-party dependencies only when they remove substantial technical risk.
* Explain the benefit and maintenance cost before adding a major dependency.
* Treat the versions committed in `Cargo.toml` and `Cargo.lock` as authoritative.

## Coding Conventions

* Use standard Rust formatting.
* Prefer clear names over terse abstractions.
* Keep systems focused on one responsibility.
* Use components for entity-local state, resources for world-level state, and events or observers for transitions.
* Avoid oversized systems that perform input, simulation, rendering, and audio together.
* Document invariants and non-obvious safety assumptions.
* Do not leave placeholder systems presented as completed features.

## Working Method for Codex

Before changing code:
1. Review [Catalogue.md](Catalogue.md) for the current workspace structure.
2. Read the relevant crate and lab.
3. Identify the smallest system needed for the task.
4. Check whether an existing abstraction already owns the behavior.
5. State any assumption that materially affects architecture.

While changing code:
1. Keep the change limited to the requested milestone.
2. Preserve separation between input, simulation, and presentation.
3. Add debug visibility for new invisible state.
4. Add or update tests.
5. Avoid unrelated refactoring.

After changing code:
1. Format the project.
2. Run relevant tests and Clippy.
3. Launch or validate the affected lab.
4. If `Catalogue.md` was updated for the change, commit and push the verified work.
5. Report changes, testing, limitations, and the next step.

## Non-Goals

Unless explicitly requested, do not:
* Build the complete game loop, online networking, matchmaking, progression systems, or complex enemy AI.
* Implement full rope physics or universal climbing.
* Generate arbitrary procedural geometry.
* Refactor the entire workspace while implementing one lab.
* Create abstractions for hypothetical future requirements.
