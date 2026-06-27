# Bevy Asset Integration Roadmap

Source research: [docs/bevy_assets_research.md](bevy_assets_research.md).
Current workspace Bevy version: `0.18.1`.

This roadmap picks the ten best candidates for integration labs. A candidate is
not approved for production use just because it appears here. Each asset starts
in an isolated lab, proves one technical question, and only graduates if it
preserves the existing project rules: stable domain IDs, input/simulation/
presentation separation, explicit ports/sockets, reset safety, tests, and
screenshot evidence where visible.

Before starting any phase, verify the exact crate version against Bevy `0.18.1`.
The official Bevy Assets page is useful triage, but Cargo compatibility is the
authority for the actual integration.

## Top 10 Candidates

| Rank | Asset | First lab | Why it is a strong candidate | Adoption gate |
| ---: | --- | --- | --- | --- |
| 1 | `bevy_trenchbroom` | `trenchbroom_lab` | Best map-editor fit for 3D rooms, corridors, doors, and explicit marker entities. | Imported map metadata converts cleanly into `RoomId`, `PortId`, door state, room/corridor type, and collision without presentation owning simulation. |
| 2 | `bevy_mod_outline` | `outline_legibility_lab` | Direct support for the Legibility Contract: players, doors, hazards, interactables, and objectives can punch through fog/bloom. | Outline treatments are selected through `observed_style`, not ad-hoc colors or per-system material choices. |
| 3 | `bevy_hanabi` | `semantic_vfx_lab` | GPU particles can make reroutes, door slams, pressure gates, pickups, and machinery state readable. | Effects are deterministic event projections and every color/brightness tier comes from semantic style. |
| 4 | `bevy_image_export` | `capture_pipeline_lab` | Could make evidence screenshots and visual-regression captures less manual. | A lab can write expected evidence frames without changing gameplay timing or requiring hidden manual state. |
| 5 | `bevy_archie` | `archie_input_lab` | Strong Bevy `0.18` input candidate for controller assignment, remapping, haptics, and local multiplayer. | It produces `PlayerIntent` only; gameplay systems still never read devices directly. |
| 6 | `bevy_mod_config` | `lab_config_lab` | Useful for persistent lab knobs: seeds, capture settings, debug overlays, visual tuning, and accessibility toggles. | Config stays outside deterministic match state unless values are explicitly part of a recorded launch manifest. |
| 7 | `bevy_log_events` | `event_trace_lab` | Makes invisible event flows inspectable for doors, observation, interactions, progression, and match transitions. | Event tracing is dev-only and can be disabled without behavior changes. |
| 8 | `bevy_color_blindness` | `color_legibility_lab` | Helps test whether the neon-noir semantic palette remains readable under color-vision deficiencies. | Failing states produce concrete style-module changes or documented limits, not one-off presentation fixes. |
| 9 | `vleue_navigator` | `navigation_probe_lab` | Good future bot/debug-navigation candidate for authored or imported 3D layouts. | Nav data is a consumer of world geometry; it must not become the source of room graph truth. |
| 10 | `bevy_ecs_ldtk` | `ldtk_schematic_lab` | Best 2D editor fallback for schematics, tactical maps, and graph/room-plan authoring. | It proves a useful 2D data path that complements, not competes with, `bevy_trenchbroom`. |

## Not In The Top 10

- `bevy_ecs_tiled`: solid, but mostly redundant with `bevy_ecs_ldtk` for this
  project's near-term schematic needs.
- `bevy_rapier`: valuable only if the custom deterministic controller/collision
  stops being good enough. Keep it as a later comparison spike, not an early
  integration lab.
- `lightyear`, `naia`, and `bevy_ggrs`: important later networking candidates,
  but the current priority is making the local first-person game more fun and
  authorable.
- `bevy-yoleck`: attractive in-game editor direction, but the researched listing
  is Bevy `0.19`; track it after a Bevy upgrade or verified older release.
- Blender/glTF metadata workflows: useful only if authored props/scenes become a
  deliberate pipeline again. They do not fit the current code-as-art priority.

## Phase A0 - Compatibility And Lab Template ✅

*(Completed 2026-06-26, folded into the A1 lab below.)* The first asset's exact
version was pinned and proven to build against the pinned Bevy: **`bevy_trenchbroom
0.13.0`** (with **`bevy_materialize 0.10`**, feature `client` only — no `bsp`/qbsp/
ericw-tools, no physics integration) compiles and runs against **Bevy `0.18.1`** (the
crate's own table lists Bevy 0.18 ↔ bevy_trenchbroom 0.12–0.13). The dependency is
isolated to [`labs/trenchbroom_lab`](../labs/trenchbroom_lab/README.md); no production
crate or `game` depends on it, and `quake-map 0.6` is a `dev-dependency` only. The lab
README names the technical question, controls, reset path, and manual verification.
Dependency impact recorded: the `client` feature pulls `bevy_materialize`, `quake-map`,
and a handful of small support crates (`jzon`, `ndshape`, `disjoint-sets`,
`enumflags2`, `float-ord`, `image`).

Question: can the candidate be evaluated without contaminating production code?

Scope:

- Pick the first asset's exact crate version and feature set.
- Create a minimal lab with no `game` dependency and no production-crate changes
  except optional pure data adapters if they are already justified.
- Record dependency impact with `cargo tree -p <lab>`.
- Add the lab to the workspace only after it builds.

Exit criteria:

- The lab builds with Bevy `0.18.1`.
- The dependency is isolated to that lab.
- The README names the technical question, controls, reset path, and manual
  verification procedure.

## Phase A1 - 3D Map Authoring ✅

*(Completed 2026-06-26 — [`labs/trenchbroom_lab`](../labs/trenchbroom_lab/README.md),
evidence [png](evidence/trenchbroom_lab.png).)* `bevy_trenchbroom 0.13` is used as the
**importer/parser only**: it parses an authored `.map` (two rooms, one corridor, two
door thresholds, a three-step elevation change, and `room`/`port`/`door`/spawn marker
entities) into brush geometry + entity properties. A pure
[`project`](../labs/trenchbroom_lab/src/project.rs) function turns that into stable
`RoomId`/`PortId`s, room/corridor classification, door state, and collision `Aabb3`s —
so **editor entities never become the game model; they are imported data the
projection owns**. Brush AABBs come from the importer's `as_cuboid` (already in Bevy
space); the box `.map` is generated from a typed layout with provably-correct winding.
Presentation is a projection of the domain model rendered through `observed_style`
(map materials decide no colours — every brush is textured `__TB_empty`, which the
importer skips, so it does zero material work). The player walks the imported
collision with the shared `observed_traversal` controller (through the open door, up
the imported stairs; walls and the closed door block), and a door's collision is gated
by its projected `DoorState`, not the map material. Reset re-projects to the authored
state with no entity leaks. **8 projection + 4 lab (3 logic + 1 lifecycle) tests**;
`fmt`/`clippy`/`test` clean. Exit criteria met (see below).

Promotion decision taken: kept **lab-local** — the importer adapter is not promoted to
a `crates/observed_authoring_trenchbroom` until a second consumer exists (per A9). A
recorded follow-up finding: `bevy_trenchbroom`'s own scene/material path uses a magenta
*missing-texture* material when texture files are absent, which would violate the
Legibility Contract — hence this lab renders from the projection through
`observed_style` rather than spawning the imported scene.

Asset: `bevy_trenchbroom`

Lab: `trenchbroom_lab`

Question: can an authored 3D map become this game's room/corridor/door topology
without making editor entities the game model?

Scope:

- Import one tiny TrenchBroom map: two rooms, one corridor, two door thresholds,
  one elevation change, and marker entities for ports/sockets.
- Convert map metadata into existing domain IDs and semantic classifications.
- Spawn render/collision as presentation from the imported projection.
- Show debug overlays for room bounds, corridor edges, ports, socket types, and
  imported entity ownership.

Exit criteria:

- Reset/despawn leaves no imported entities behind.
- The player can traverse imported collision through the corridor and doors.
- Closed/open doorway states remain driven by the door model, not by map
  material names.
- Screenshot evidence is captured.

Promotion decision:

- If this works, create a narrow authoring adapter such as
  `observed_authoring_trenchbroom` or keep it lab-local until a second consumer
  exists.

## Phase A2 - 2D Schematic Authoring Fallback ✅

*(Completed 2026-06-26 -- [`labs/ldtk_schematic_lab`](../labs/ldtk_schematic_lab/README.md),
evidence [png](evidence/ldtk_schematic_lab.png).)* `bevy_ecs_ldtk 0.14.0` is the
Bevy `0.18`-compatible LDtk line and is isolated to the lab with
`default-features = false` plus `internal_levels`; the lab uses the importer/schema
but does **not** adopt LDtk tile rendering as gameplay presentation. The authored LDtk
project contains the same two-room/one-corridor topology as the TrenchBroom lab: a
`Room`/`Port` entity layer projects into stable `RoomId`/`PortId` graph metadata, and
an IntGrid layer projects into tactical-map symbols for room fill, corridor, door
thresholds, spawn, and objective. The runtime view is a projection of that pure
schematic model through `observed_style`; LDtk entities never become game entities.
Exit criteria met: the topology round-trips to the same graph expectations, the
output is useful as a tactical/design schematic, reset reprojects without visual
leaks, and the README documents the decision. Promotion decision: keep LDtk
**lab-local and live as a fallback** until a second consumer needs editable
tactical-map or route-plan data; it complements, but does not replace,
TrenchBroom's first-person geometry authoring.

Asset: `bevy_ecs_ldtk`

Lab: `ldtk_schematic_lab`

Question: is LDtk useful for tactical maps, route plans, or room-graph sketches
that do not need full 3D geometry?

Scope:

- Author the same two-room/one-corridor topology in LDtk.
- Convert LDtk layers/entities into room IDs, ports, and schematic map symbols.
- Compare what LDtk captures better than TrenchBroom and what it cannot express.

Exit criteria:

- The LDtk topology round-trips into the same pure graph expectations as the
  TrenchBroom lab.
- The output is useful for a tactical map or design-time schematic.
- The lab documents whether LDtk remains a live candidate or is only a fallback.

Promotion decision:

- Promote only if it provides durable schematic authoring value. Do not add both
  LDtk and Tiled unless they answer different project needs.

## Phase A3 - Legibility Overlay âœ…

*(Completed 2026-06-26 -- [`labs/outline_legibility_lab`](../labs/outline_legibility_lab/README.md),
evidence [png](evidence/outline_legibility_lab.png).)* `bevy_mod_outline 0.12.1`
is the Bevy `0.18`-compatible outline candidate and is isolated to the lab with
`default-features = false`; the lab uses the core mesh-outline pass only, with no
scene inheritance, flood fill, or interpolation features. Every gameplay-critical
signal in the showcase -- open/closed doors, interactables, hazards, rivals,
objective beacons, pickups, and the local player proxy -- receives its outline
colour and width from the shared `observed_style::outline` semantic table. The
scene deliberately stacks the worst cases A3 asked for: foggy corridor, bright
bloom glare, overlapping console/pickup/hazard signals, a partly occluded moving
rival, and a distant objective. Reset despawns and rebuilds the projection without
leaking scene entities, and the lab overlay reports `[PASS]` only when every
signal has an active outline, one camera, one UI root, and the style contrast
floor passes.

Compatibility finding: the researched `bevy_color_blindness` crate is **not**
adopted directly. The only current published version is `0.2.0`, and its manifest
depends on Bevy `0.8.0`, so linking it would pull an incompatible Bevy line rather
than satisfy this workspace's Bevy `0.18.1` gate. The color-vision check remains
covered by pure `observed_style` preview matrices (normal, protanopia,
deuteranopia, tritanopia, achromatopsia), and tests assert every semantic outline
stays above the simulated luminance floor. Promotion decision: keep
`bevy_mod_outline` lab-local until a second visible consumer needs semantic
outlines; reject direct `bevy_color_blindness` integration until a compatible
crate version exists.

Assets: `bevy_mod_outline`, `bevy_color_blindness`

Lab: `outline_legibility_lab`

Question: can gameplay-critical state stay readable in neon-noir presentation
under fog, bloom, motion, and color-vision simulation?

Scope:

- Render doors, interactables, hazards, rivals, objective beacons, and pickups
  with outline treatments selected from `observed_style`.
- Test normal view plus color-blindness preview modes.
- Include worst-case scenes: foggy corridor, bright bloom, overlapping signals,
  and a distant objective.

Exit criteria:

- Every gameplay-critical signal has a documented semantic treatment.
- No system invents local colors or outline widths outside the style module.
- Color-vision checks identify either passing contrast or a concrete style fix.

Promotion decision:

- Promote style-facing helper APIs only if multiple game/lab presenters use them.

## Phase A4 - Semantic VFX ✅

*(Completed 2026-06-26 -- [`labs/semantic_vfx_lab`](../labs/semantic_vfx_lab/README.md),
evidence [png](evidence/semantic_vfx_lab.png).)* `bevy_hanabi 0.18.0` is the Bevy
`0.18`-compatible Hanabi line and is isolated to the lab with
`default-features = false` plus `3d`; no production crate or `game` depends on it.
The lab treats particles as **presentation-only projections** from a deterministic
semantic timeline: door open, pressure gate, reroute flash, keystone pickup, door
slam, and exit unlock are one-shot events, while equipment power is a continuous
toggled projection. Every effect hue is derived from `observed_style` treatments
and normalized down from HDR emission so particles remain accents rather than
screen-filling bloom.

Exit criteria met: `V` toggles VFX projection off without changing the event
timeline or removing gameplay signal anchors; the pure model tests cap particle
count, lifetime, and screen-space size; the screenshot capture fires the crowded
six-event state and keeps doors, the pressure gate, the player proxy, the pickup,
and the objective readable; reset rebuilds without leaking cameras, UI roots,
anchors, or active effects. Dependency impact recorded with `cargo tree -p
semantic_vfx_lab -e normal`: the new direct asset dependency is `bevy_hanabi`,
which adds Hanabi's shader/effect stack (`naga`, `naga_oil`, `wgpu`, `rand`,
`rand_pcg`, `ron`, `serde`, `thiserror`, `anyhow`, `fixedbitset`, `bytemuck`,
`bitflags`) on top of the existing Bevy 0.18 graph. Promotion decision: keep
Hanabi **lab-local** until a second visible consumer needs the same semantic VFX
projection.

Asset: `bevy_hanabi`

Lab: `semantic_vfx_lab`

Question: can particle effects improve readability without becoming decorative
noise or nondeterministic gameplay state?

Scope:

- Add event-projected effects for door slam/open, reroute flash, pressure gate,
  keystone pickup, exit unlock, and equipment power.
- Keep effect triggering from existing events or derived presentation state.
- Drive effect color/emission/timing from semantic style where practical.

Exit criteria:

- Effects can be toggled off with no simulation change.
- Effects never hide doors, hazards, players, or objective signals.
- Screenshot evidence shows the most crowded state remains readable.

Promotion decision:

- Promote a tiny `SemanticVfx` presentation layer only after it is used by more
  than one visible lab or the assembled game.

## Phase A5 - Evidence Capture Pipeline ✅

*(Completed 2026-06-27 -- [`labs/capture_pipeline_lab`](../labs/capture_pipeline_lab/README.md),
evidence directory `docs/evidence/capture_pipeline_lab/`.)* `bevy_image_export
0.16.0` is the Bevy `0.18`-compatible line and is pinned exactly; the latest
`0.17.0` line depends on Bevy `0.19` and is rejected for this workspace. The lab
uses the crate as an offscreen render-target exporter rather than replacing the
existing `OBSERVED2_CAPTURE` screenshot hooks globally. One capture command writes
a deterministic still frame to `docs/evidence/capture_pipeline_lab/still/00001.png`
and a six-frame transition sequence to
`docs/evidence/capture_pipeline_lab/sequence/00001.png` through `00006.png`.
Those lab-owned directories avoid collisions with unrelated evidence files.

Exit criteria met: the pure model samples fixed ticks across a door-open and
reroute-flash transition; tests assert capture sampling does not advance or pause
the fixed-step timeline; the Bevy lifecycle test covers reset without leaking the
window camera, export camera, UI root, or door panels; `fmt`/focused `test`/focused
`clippy` are clean. Dependency impact from `cargo tree -p capture_pipeline_lab -e
normal --depth 1`: direct dependencies are `bevy 0.18.1`, `bevy_image_export
0.16.0`, and `observed_style`; the export crate adds its GPU readback/image
sequence stack (`futures`, `futures-lite`, `image`, `thiserror`, `bytemuck`,
`wgpu`) on top of the existing Bevy graph. Promotion decision: keep
`bevy_image_export` **lab-local** until a second lab adopts the same offscreen
capture helper shape.

Asset: `bevy_image_export`

Lab: `capture_pipeline_lab`

Question: can the existing screenshot evidence loop become more repeatable?

Scope:

- Capture one still frame from a deterministic scene.
- Capture a short sequence for a timed state transition such as door open or
  reroute flash.
- Standardize output paths under `docs/evidence/` without overwriting unrelated
  evidence.

Exit criteria:

- Captures are reproducible enough for review.
- Capture does not alter fixed-step simulation, input replay, or match timing.
- The README documents the command and expected output.

Promotion decision:

- Promote shared capture helpers only if at least two labs adopt them.

## Phase A6 - Controller And Local Multiplayer Input ✅

*(Completed 2026-06-27 -- [`labs/archie_input_lab`](../labs/archie_input_lab/README.md),
evidence [png](evidence/archie_input_lab.png).)* `bevy_archie 0.2.4` is the Bevy
`0.18` line (the latest `0.3.0` targets Bevy `0.19`, and the `0.1.x` line targets
Bevy `0.17`), pinned exactly with `default-features = false` and isolated to the
lab. The lab adds archie's real `ControllerPlugin` (device detection, controller
ownership, haptics, the data-driven `ActionMap`) and routes keyboard, gamepad,
scripted, and replayed control for four local players into the **same**
`player_input::PlayerIntent`; gameplay reads no device directly.

Compatibility finding: **`bevy_archie 0.2.x` declares MSRV `1.94`.** The workspace
was on Rust `1.92`, so building the lab required updating the toolchain to `1.96`
(stable). That toolchain bump is the one real cost of the dependency and is the
recorded adoption gate.

Architecture finding: archie's `ActionState` resource is **global** — its
`update_action_state` merges every gamepad and the keyboard into one action set, so
it cannot answer "what does *player 2's* controller want?". The lab therefore keeps
archie's genuinely valuable pieces (the remappable `ActionMap`, the `GameAction`
vocabulary, the `ControllerConfig` deadzone model, the `ControllerOwnership`
assignment store, and `RumbleRequest` haptics) and evaluates them against **one
device sample at a time** in a pure adapter ([`adapter.rs`](../labs/archie_input_lab/src/adapter.rs)),
yielding per-player isolation through a single code path that ends in
`PlayerIntent`. This mirrors the trenchbroom lab's "use the importer, not the
scene" stance: consume the asset's data model, not the part that fights the
architecture.

Exit criteria met: gameplay consumes only `PlayerIntent`; four players are
assignable to keyboard, controller (claimed through archie ownership, reverting to
a scripted pattern on disconnect), or scripted control with no single-player
assumptions; `F7` runtime remapping edits archie's `ActionMap`; focus loss
neutralizes every intent; recording/replay works at the intent layer; haptics are
presentation-only (a test asserts a hazard pulse never changes `PlayerIntent`).
**6 adapter + 7 lab (lifecycle/integration) tests**; `fmt`/`clippy -D warnings`/
`test` clean; screenshot captured (a live vJoy controller even claimed a slot
during capture). Promotion decision: keep `bevy_archie` **lab-local**; its global
`ActionState` is not adopted, and a narrow device-to-intent adapter should be
promoted only when controller support becomes a committed feature (per A9). Do not
replace `player_input`.

Asset: `bevy_archie`

Lab: `archie_input_lab`

Question: can richer device support feed the existing `PlayerIntent` boundary
without rewriting gameplay systems?

Scope:

- Map keyboard plus at least one controller to separate local player IDs.
- Test assignment, disconnection, focus loss, remapping, and scripted fallback.
- Optionally test haptics for door slam or hazard feedback as presentation only.

Exit criteria:

- Gameplay still consumes only `PlayerIntent`.
- Four local players can be assigned to human, controller, keyboard, or scripted
  control without single-player assumptions.
- Recording/replay remains possible at the intent layer.

Promotion decision:

- Promote only the device-to-intent adapter. Do not replace `player_input`
  wholesale unless the lab proves a clear long-term benefit.

## Phase A7 - Lab Config And Event Trace ✅

*(Completed 2026-06-27 -- [`labs/lab_observability_lab`](../labs/lab_observability_lab/README.md),
evidence [png](evidence/lab_observability_lab.png).)* One asset is adopted and one
is rejected, A3-style. **`bevy_mod_config 0.6.2`** is the Bevy `0.18` line (core
deps `bevy_app`/`bevy_ecs ^0.18.0`; the `bevy_egui` editor and `serde` persistence
are *optional*) and is pinned `default-features = false` with
`["std", "serde", "serde_json"]` — the lab takes its typed schema, change
detection, and JSON persistence but **leaves the egui editor off**, rendering its
own neon-noir overlay. **`bevy_log_events 0.7.0`** (the only Bevy `0.18` release;
`0.6.0` targets Bevy `0.17`) is **not adopted**: its only logging-capable feature
`enabled` force-pulls `bevy_egui`, and `LogEventsPlugin::build` does
`assert!(app.is_plugin_added::<EguiPlugin>())`, so it cannot be used without an
egui UI stack — against the project's dependency rule (egui was kept off in A3 and
A6). The event-trace capability is implemented **lab-locally** over Bevy's own
`tracing` log instead, exactly mirroring A3's `bevy_color_blindness` rejection.

The lab's spine is the boundary the phase asks about. A single config root holds
the launch `seed` plus debug knobs (fog, bloom, overlay, color-vision preview,
trace verbosity, capture warm-up). The running simulation is driven by an
`ActiveManifest`, not by the live config: editing the seed only marks a *pending
relaunch*, and `Enter` **commits** it into the manifest (the one explicit path a
config value enters deterministic state); at startup a persisted seed becomes the
manifest the same way. Every other knob is applied live and is structurally
incapable of perturbing the simulation, because the pure `model::simulate` takes
only a `LaunchManifest`. The lab-local tracer mirrors semantic events (doors,
observation, interaction, match rounds, pickups, reroutes) to `tracing` gated by
the verbosity knob, always draining the message stream so logging toggles change
nothing about the sim. The overlay shows a colour-coded ring of recent events with
their rooms — faster to read than scrolling console output — plus the active
manifest, stream checksum, and every knob value.

Exit criteria met: config changes are clearly separated from the deterministic
launch manifest (a test edits the seed and sees the running sim unchanged until
commit); event logging disables with no behaviour change (a test runs the same
seed loud vs. silent with every other knob changed and asserts identical stream
checksum and step count, only the log-line count differing); and the live event
overlay is the bug-hunting workflow that beats console-only inspection. JSON
persistence round-trips through the Serde manager (`F2`/`F3`), defaulting to a
temp path (override `OBSERVED2_CONFIG`) so the repo is never littered. **5 pure
model + 5 lab (lifecycle/integration) tests**; `fmt`/`clippy -D warnings`/`test`
clean; screenshot captured. Dependency impact from `cargo tree -p
lab_observability_lab -e normal --depth 1`: direct deps are `bevy 0.18.1`,
`bevy_mod_config 0.6.2`, `observed_style`, and `player_input`; `bevy_mod_config`
adds a small render-free stack (`bevy_mod_config_macros`, `derivative`,
`hashbrown`, `serde`, `serde_json`, `variadics_please`) with **no egui and no GPU
crates**. Promotion decision: keep `bevy_mod_config` **lab-local** until a second
consumer needs the same typed-config + persistence shape (per A9), and keep
runtime game settings separate from debug configuration; reject `bevy_log_events`
until a release drops the mandatory egui plugin.

Assets: `bevy_mod_config`, `bevy_log_events`

Lab: `lab_observability_lab`

Question: can labs expose useful knobs and event traces without making debug
state part of the game simulation?

Scope:

- Add config-backed toggles for seed, fog/bloom intensity, overlays, capture
  frame, color-vision preview, and event trace verbosity.
- Trace representative events from doors, observation, interactions, match
  rounds, and pickups.
- Show the active config in a debug overlay.

Exit criteria:

- Config changes are clearly separated from deterministic launch manifests.
- Event logging can be disabled without behavior changes.
- The lab demonstrates one bug-hunting workflow that is faster than current
  console-only inspection.

Promotion decision:

- Promote dev-only helpers if they simplify multiple labs. Keep runtime game
  settings separate from debug configuration.

## Phase A8 - Navigation Probe ✅

*(Completed 2026-06-27 -- [`labs/navigation_probe_lab`](../labs/navigation_probe_lab/README.md),
evidence [png](evidence/navigation_probe_lab.png).)* `vleue_navigator 0.15.0` is
the Bevy `0.18` line (it declares `bevy ^0.18.0` with the
`bevy_render`/`bevy_asset`/`bevy_log` features, so it links cleanly against the
pinned `0.18.1`; the later `0.16+` line targets newer Bevy and is rejected). It is
pinned `default-features = false` to drop the crate's `debug-with-gizmos` default,
which would otherwise pull `bevy/bevy_gizmos` as a hard dependency of the asset.
The crate is used as a **navmesh builder + polyanya path query only**
(`NavMesh::from_edge_and_obstacles` + `NavMesh::path`); the `NavmeshUpdaterPlugin`
auto-updater (built to rebuild obstacle meshes from physics colliders) is **not**
adopted — this lab has no physics layer and authors its obstacles directly. That
mirrors the prior labs' "use the data model, not the part that fights the
architecture" stance.

The lab keeps a hard, one-way split. [`facility.rs`](../labs/navigation_probe_lab/src/facility.rs)
is the **authoritative** model: four rooms (`A`/`B`/`C`/`D`) divided by a wall
cross and joined by four doors forming a 4-cycle, with connectivity decided here
by BFS over *open* doors. [`nav.rs`](../labs/navigation_probe_lab/src/nav.rs) is a
**derived consumer**: it builds the navmesh from the facility's permanent walls
plus a plug for every *closed* door, routes over it with polyanya, and never
writes back. Toggling a door rebuilds the navmesh, so a closed door becomes a
solid obstacle and the route detours through the other side of the loop or fails
outright — the navmesh can never route through a door the facility says is shut. A
debug **bot** (the "agent" the phase asks for) walks the current route to the goal,
proving the derived path is physically traversable; it consumes the route and is
never authoritative.

Exit criteria met: navigation respects closed doors and blocked corridors (the
headline test sweeps **all 16 door configurations × every ordered room pair** and
asserts the navmesh agrees with the graph about reachability and never produces a
walk that crosses a closed door); nav data updates/invalidates cleanly when a
route changes (door toggles set a dirty flag that rebuilds the navmesh and reroutes
in-frame, an in-app test confirms `A->D` snaps from `A>B>D` to `A>C>D` when `AB`
closes); the room graph stays authoritative (it owns reachability; the navmesh is
only a derived consumer, asserted by the cross-check). The top-down schematic
renders structure through `observed_style` — blue wall cross, green/red door
state, gold route, cyan probe/bot, green goal — and the overlay's `[PASS]` line
requires `nav == graph` agreement plus full entity health. **7 facility + 6 nav +
5 lab (lifecycle/integration) tests**; `fmt`/`clippy -D warnings`/`test` clean;
screenshot captured (it shows `AB` closed and the `A->D` route rerouting through
C). Dependency impact from `cargo tree`: the new direct dependency is
`vleue_navigator`, which adds its pathfinding stack (`polyanya`, `geo`,
`spade`, `hashbrown`, `smallvec`, `itertools`) on top of the existing Bevy graph;
no GPU/egui crates and no auto-updater plugin. Promotion decision: keep
`vleue_navigator` **lab-local**; per the roadmap, game adoption stays **deferred**
until bots/AI need physical routing in the first-person facility (the current match
brain does not). No production crate or `game` depends on it.

Asset: `vleue_navigator`

Lab: `navigation_probe_lab`

Question: can imported/authored geometry produce useful bot or debug navigation
without taking ownership of the facility graph?

Scope:

- Build a navmesh from a small imported or procedural facility.
- Ask a bot/debug agent to route from room A to room B through doors/corridors.
- Compare route results against the authoritative room graph and door state.

Exit criteria:

- Navigation respects closed doors and blocked corridors.
- Nav data updates or invalidates cleanly when a route changes.
- The room graph remains authoritative; navmesh is only a derived consumer.

Promotion decision:

- Defer game adoption until bots need physical routing in the first-person
  facility. This is not needed for the current match brain.

## Phase A9 - Game Integration Decision ✅

*(Completed 2026-06-27 — decision [docs/asset_a9_decision.md](asset_a9_decision.md).)*
The triage is recorded: **all eight lab-proven assets (A1–A8) stay lab-local with a
concrete adopt-when trigger each; zero are promoted into the assembled `game` now.**
None clears the "concrete game use today" bar, and `bevy_trenchbroom` actively
conflicts with the game's deliberately procedural geometry (the teleport-hallway pivot:
seeded mazes, jittered hallways, polygon rooms). The strongest standing claim is
`bevy_mod_outline`, queued as the **first** promotion (behind an `observed_style::
outline`-driven presentation adapter) when the paused legibility arc resumes; the rest
defer behind their triggers (see the decision doc's summary table). This mirrors every
lab's own "keep lab-local … per A9" decision and the R11 precedent (accept only what
solves a real current problem). The two researched-but-rejected crates stay rejected on
compatibility grounds: `bevy_color_blindness` (Bevy 0.8 only) and `bevy_log_events`
(force-pulls `bevy_egui`). No production crate or `game` gained an asset dependency; the
`game -> crates/* -> pure domain` split and zero `game -> labs/*` edges are intact.
Exit criteria met (see below): `ROADMAP.md` records the outcome, `Catalogue.md` gains
the two post-catalogue labs (`lab_observability_lab`, `navigation_probe_lab`) plus this
decision doc, and the "promoted asset" criterion is vacuously satisfied since nothing is
promoted (all eight labs nonetheless carry tests + README + evidence).

Question: which lab-proven assets should enter the assembled game?

Scope:

- Review Phases A1-A8 for dependency cost, maintenance risk, and gameplay value.
- Promote only assets with at least one passing lab and a concrete game use.
- Prefer small adapter crates over direct `game` integration when the behavior
  has multiple consumers.

Likely first promotions:

1. `bevy_trenchbroom`, if authored maps beat the current procedural/hard-coded
   geometry workflow.
2. `bevy_mod_outline` plus color-vision fixes, if they materially improve
   gameplay readability.
3. `bevy_image_export`, if it makes evidence capture more reliable.

Likely deferred:

- `vleue_navigator`, until bots or AI need physical pathfinding.
- `bevy_archie`, until controller support is a committed feature.
- `bevy_hanabi`, unless readability improves more than the added render
  complexity costs.

Exit criteria:

- The main `ROADMAP.md` is updated only with proven outcomes, not intentions.
- `Catalogue.md` is regenerated or updated if new labs/crates become permanent.
- Every promoted asset has tests, a README, evidence where visible, and an
  owner boundary that matches the production/lab split.
