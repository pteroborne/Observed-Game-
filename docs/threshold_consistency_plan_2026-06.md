# Transparent Thresholds + Consistency — and a Scoped Forward Roadmap

> The earlier comfort/lighting pass (wider/taller/longer halls, seamless doors, diegetic
> decoherence, colonnades, district palettes, unified preview lighting + flickering fixtures
> + accent trim) is **implemented and merged into the working tree**. This plan covers the
> next body of work.

## Context

Playtesting surfaced a real consistency bug: **what you see through a doorway is not the
place you teleport into**, and bots **pop in** when you cross a threshold. Root cause —
the simulation decides when to reroute from *its own* decoupled view, not from where the
player actually is in the teleport place-model, so the doorway *preview* (rendered once,
from the old graph) and the *arrival* (recomputed after a reroute fires mid-cross) diverge;
and neighbor occupants are never rendered, only the empty neighbor geometry.

The fix aligns with the game's own fiction — *observed things can't change*. We make "what
the player can see" (the current place **and** each neighbor visible through an open
threshold) the thing that reads as **observed → frozen**, and render those neighbors for
real (including their bots). Then preview == arrival by construction and nothing pops.

**Decisions (from the user):** thresholds are **always-open / transparent** (no hiding door
leaf); the first build also **standardizes threshold width** and gives every doorframe a
**tether-light** (colored by whether the edge is anchored by a torch). The larger guardian /
AI / topology / map-editor ideas are **scoped here as a backlog but not built yet.**

Everything in Phase 1 is **presentation-only** — the deterministic brain (rounds /
networking / replay) is untouched, so determinism and lockstep are preserved (CLAUDE.md
rules 2 & 7). The freeze-when-observed mechanic already exists in `observed_observation`
(`is_pinned`/`observed`/`decohere`); Phase 1 realizes it on the presentation side without
brain surgery.

---

## Phase 1 — Always-open transparent thresholds + consistency (BUILD NOW)

The existing passage-preview already renders each neighbor's real geometry aligned through
the doorway ([place.rs](game/src/screens/place.rs): `spawn_passage_preview` /
`spawn_hallway_preview` / `spawn_room_preview`). Phase 1 makes that neighbor render
**authoritative, frozen-while-observed, populated, and always-open.**

### 1a. Freeze each doorway's destination at place-entry (the consistency fix)

The bug is that crossing recomputes `apply_crossing` with the *current* nav (which may have
rerouted since the place was rendered). Fix: **resolve each passage gap's destination once,
when the place is entered, and freeze it** — the preview renders from it AND the crossing
honors it, so they can't diverge.

- In [match_runtime.rs](game/src/screens/match_runtime.rs) `place_body`, after building
  `geom`, compute the resolved destination `Place` for each passage gap via
  `teleport::apply_crossing(place, gap, nav)` and store them on `TeleportState` (new field
  `gap_dests: Vec<teleport::Place>` aligned to `geom.gaps`, in [screens.rs](game/src/screens.rs)).
- `teleport_sim`'s crossing detection ([match_runtime.rs](game/src/screens/match_runtime.rs)
  Room + Hallway arms) looks up the crossed gap's **frozen** `gap_dests` entry instead of
  re-calling `apply_crossing`; falls back to `apply_crossing` only if absent.
- `rebuild_place`'s preview loop reads the same frozen destination so the preview is the
  exact place you enter. (The seamless remap `crossing_alignment` + entry-door `open_entry`
  from the prior pass stay as-is and now operate on consistent geometry.)
- Because presentation honors the observed snapshot, a brain reroute *under* an observed
  neighbor is simply ignored until you look away (re-entry rebuilds from the live graph) —
  which is exactly "observed = frozen, unobserved = free to change." Brain untouched.

### 1b. Render neighbor occupants so bots don't pop in

- In `spawn_room_preview` ([place.rs](game/src/screens/place.rs)), after the room geometry,
  spawn a posed rival avatar for each team in that neighbor room — reuse
  `rivals::rivals_in_room(&game.competitive, dest_room)` ([rivals.rs](game/src/rivals.rs))
  and the existing `rival_body_mesh` / `rival_material`, parented by the preview transform,
  tagged `PassagePreview` for teardown. They're visible through the open threshold, so
  crossing in shows the same figures in place — no pop. (Continuous walk-through-the-door
  animation is later polish; static presence kills the pop-in.)

### 1c. Always-open transparent thresholds

- In `rebuild_place`'s gap loop ([place.rs](game/src/screens/place.rs)), passage gaps no
  longer get a hiding/openable leaf — the neighbor is always visible. Keep the neon
  doorframe (`spawn_place_frame`); keep sealed `Side` leaves and the red `LockedExit` leaf.
- Drop the passage branch from `animate_doors` (it only needs to handle the locked/sealed
  cases now, or be removed if nothing remains). Remove `DoorLeaf`/`DOOR_OPEN_RADIUS` logic
  that hid corridors. The "slam shut on reroute" in `sync_decohere_fx` now only affects any
  remaining sealed leaves.

### 1d. Standardize threshold width

- Add a single `THRESHOLD_WIDTH` constant in [teleport.rs](game/src/teleport.rs) and route
  every crossable gap's width through it: room gaps (`room_geom`), and hallway entry/exit
  mouths (`hallway_geom` straight/chicane/colonnade/maze), clamping to the edge/cell where a
  narrower space forces it. Replace the ad-hoc `GAP_WIDTH` / per-template widths at the
  doorway. Update the dimension assertions in `teleport.rs` tests.

### 1e. Tether-light doorframes

- Add a per-gap "tethered" read: a helper in [teleport.rs](game/src/teleport.rs) like
  `is_tethered(nav, a, b)` over `Nav.pins` (`PinnedEdge`, already used by `effective_version`).
- `spawn_place_frame` ([place.rs](game/src/screens/place.rs)) takes the tethered bool and
  adds a small frame `PointLight` + emissive: **tethered → the anchor/Control color**
  (`style::marker(MarkerRole::Control)`), **untethered → a neutral cool tone**. So a glance
  at a doorway shows whether that edge is pinned by a torch.

### Phase 1 verification
1. `cargo fmt --all`; `cargo clippy --workspace --all-targets` (no new warnings);
   `cargo test --workspace` (update `teleport.rs` width tests; add a test that a gap's frozen
   `gap_dests` entry equals what crossing produces).
2. Run the game (`cargo run -p observed_game`) and walk a full match: through every doorway,
   what you see is exactly what you enter (no variation/maze change, no room swap); rival
   bots in the next room are visible *before* you cross and don't pop; thresholds are always
   open and uniform width; tethered edges glow differently at the frame (drop an anchor torch
   and confirm the doorframe light changes).
3. Capture evidence (room + doorway + a populated neighbor) under `docs/evidence/`.

---

## Backlog — scoped, sequenced, NOT yet built

### B1. Guardian / AI arc (feasibility-lab-gated, per project method)
Reuses: the room/edge graph + `is_pinned`/`observed`/`decohere`
([observed_observation](crates/observed_observation/src/lib.rs)), BFS routing
(`route_corridor` in [maze.rs](crates/observed_match/src/maze.rs)), the protected `spine`,
and `rivals.rs` avatar projection. Sequence:
- **`guardian_ai_lab` (new lab):** one "weeping-angel" guardian — moves toward its target
  player **only while that player isn't observing it** (reuse the observation/visibility
  check), graph **Dijkstra/BFS pathfinding** toward the player's room using current+known
  edges, **teleport-the-player-to-a-random-room on touch**. Prove determinism (seed+input →
  identical trace) so it survives replay/lockstep.
- **Promote** to an `observed_guardians` crate once a second consumer needs it.
- **Game integration:** difficulty option = **guardian count per player** (a new settings/
  config surface — `lab_observability_lab`'s typed config or `Career` persistence is the
  pattern); **per-character assignment** with **reassignment only in a specific room**;
  **camera-display rooms** (one showing all tethered rooms, one showing guardians) rendered
  from the same live brain state the tac-map uses.

### B2. Deferred teleport/pad polish (small, presentation-only)
- **~2s teleport animation** (screen-wipe / light effect) for pad + on-touch + random
  teleports, so a teleport reads as a deliberate event.
- **Pad Stargate glow + auto-trigger:** rising/pulsing glow on `spawn_teleport_pad`
  ([place.rs](game/src/screens/place.rs)); auto-fire on step by moving pad activation from
  the `activate_pad` keypress intent ([match_runtime.rs](game/src/screens/match_runtime.rs)
  `item_actions`) to a proximity trigger.

### B3. Topology & authoring (largest; each its own design pass)
- **Min-wall-length rule:** add a validator (no wall segment shorter than X) to room/hallway
  geometry generation; partly implicit today via `MIN_HALL_LENGTH` and `ROOM_MARGIN`.
- **Many-to-many room↔hallway via standard thresholds:** today the graph is one-hallway-per-
  edge ([teleport.rs](game/src/teleport.rs) `Place::Hallway{from,to}`); a true many-to-many
  topology (a hallway opening onto several rooms and vice versa) is a traversal-model redesign
  — own feasibility lab.
- **ASCII map editor:** a layered text format (walls / thresholds / rooms / hallways) + import
  into the `RoomId`/graph model. No data-driven map format exists today (geometry is
  procedural); this is a separate tool sub-project. The proven-but-deferred `trenchbroom_lab`
  / `ldtk_schematic_lab` parsers are prior art to consult.

**Recommended order:** Phase 1 → B2 (cheap polish) → B1 guardian lab → B1 integration → B3.
