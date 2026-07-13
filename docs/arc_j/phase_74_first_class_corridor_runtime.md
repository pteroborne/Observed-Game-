# Phase 74 — First-Class Corridor & Threshold Runtime

**Status:** `[ ]` — next after Phase 73 (`8cd5f63`).
**Branch:** `codex/rapier-threshold-integration`.
**Nature:** one atomic contract change. Do not parallelize — the `Place` enum and
the crossing state machine are shared by every consumer and must move together.

---

## Goal

Replace the pair-shaped `Place::Hallway { from, to, variation }` contract with
**stable corridor identity + threshold-socket crossing**. After this phase, a
transition is resolved by asking the junction topology "what socket partners the
one I just crossed?" — never by reconstructing a `(from, to)` room pair. This is
the change that makes it impossible for a rendered aperture, a physical aperture,
and a graph connection to disagree, and it unblocks multi-exit corridors
(Gantry side exit, Wellshaft) in Phase 76.

---

## Where Phase 73 left things (the two-worlds problem)

Phase 73 built the **canonical** identity + topology types in the crates but did
**not** rewire the game's live place model onto them. There are currently two
parallel vocabularies, and the core of this phase is fusing them:

| Concern | Canonical (crates) — target | Game-local (teleport) — legacy |
|---|---|---|
| Place identity | `observed_core::PlaceId::{Room, Corridor}` | `Place::{Room(RoomId), Hallway{from,to,variation}}` |
| Corridor identity | `observed_core::CorridorId(u32)` | `HallId { a, b }` (a *room pair*, `teleport/mod.rs:98`) |
| Threshold slot | `observed_core::ThresholdSlotId(u16)` | `teleport::ThresholdSlotId(u8)` (`teleport/mod.rs:87`) — **name-collision, different width** |
| Threshold identity | `observed_core::ThresholdId { place, slot }` | `ThresholdLink { room, hall, local_side }` (`teleport/mod.rs:130`) |
| Connectivity | `observed_facility::JunctionTopology` (reciprocal `partner()`) | `apply_crossing` reconstructs the pair from `gap.target` + `nav.effective_version(from,to)` |
| Authored map | `MapSpec.corridors: Vec<MapCorridor>` (`map_spec.rs:122`) | `MapSpec.edges: Vec<MapEdge>` (room pairs) |

`JunctionTopology` (`crates/observed_facility/src/junction.rs`) already provides
everything the runtime needs: `attach`, reciprocal `partner(ThresholdId)`,
`corridor_rooms`, `reachable_rooms`, and it already rejects half-rewires
(`RoomAttachedTwice` / `CorridorAttachedTwice`) and non-bipartite links. **Do not
reinvent it — consume it.**

---

## Scope: what Phase 74 owns vs. what it defers to Phase 75

Keeping the phase atomic-but-bounded. The seam is deliberate:

**Phase 74 owns (this doc):**
1. The `Place` contract itself: corridor identity + socket-resolved crossing.
2. **Reroute/decohere authority** moves onto `JunctionTopology` attachments —
   because "reroutes update attachments atomically, never one side" is a property
   of the topology, not of a consumer. `Nav`'s rewiring (`nav.effective_version`,
   `PinnedEdge`) becomes attachment rewiring.
3. The **geometry / render / physics aperture path**: door/threshold render gaps,
   occlusion, crossing volumes, and Rapier apertures all consume the *same active
   socket set* the topology exposes. These are already derived from `PlaceGeom`
   (`place_structural_primitives` / `place_rapier_scene`) — the job is to source
   `PlaceGeom.gaps` from the socket set rather than from the room pair.
4. Regression fixtures for the **"wall in front of a traversable threshold"**
   failure: a socket that is crossable in the topology must never render/collide
   as a solid wall, and vice-versa.

**Phase 74 explicitly defers to Phase 75** (do NOT migrate these here; leave a
compatibility shim if needed so they still compile):
- The ~12 *logic* consumers that read the pair: `bot.rs`, nav pathing, guardian
  targeting, `items.rs`, ambience/audio, observation knowledge, `tacmap.rs`,
  previews' higher-level callers, diagnostics/evidence, replay, `map_validation`.
- The full procedural corpus sweep (every room role, Chicane/Colonnade/Maze/
  Gantry/Wellshaft at every elevation).

> **Implementer judgment call to surface back to the parent:** the exact 74/75
> line for `Nav`. Item 2 moves the *authority* (who owns attachment state) in 74;
> consumers that only *read* pair-versions can keep reading through a shim until
> 75. If honoring that split forces churn that's cheaper to just finish, stop and
> flag it rather than silently expanding Phase 74's blast radius.

---

## Concrete work, in landing order

Each step should compile + keep `cargo test --workspace` green before the next.

### 1. Unify `ThresholdSlotId`
Delete `teleport::ThresholdSlotId(u8)` (`teleport/mod.rs:87`); use
`observed_core::ThresholdSlotId(u16)` everywhere. Widen the game-local
`RoomThreshold`/`HallThreshold`/`ThresholdLink` slots to match. This is the
smallest independent step and de-risks the rest.

### 2. Give `Place` a corridor identity
Change `Place::Hallway { from, to, variation }` so the *place* is identified by a
`CorridorId` (variation stays presentation state; `from/to` stop being identity).
Provide `Place::place_id() -> PlaceId` mapping `Room→Room`, `Hallway→Corridor`.
Corridors are still generated/derived from the graph — `MapSpec::corridors`
already derives "one stable two-socket corridor per legacy edge" when
`corridors` is empty (`map_spec.rs:137`), so existing single-exit maps keep
working with zero authored-map changes.

### 3. Resolve crossings through `JunctionTopology::partner`
Rewrite `apply_crossing` (`teleport/transition.rs:57`) and `Crossing`
(`transition.rs:46`) so:
- crossing a room socket → look up its `partner()` → you're now in that partner's
  corridor, entered through that corridor socket;
- crossing a corridor socket → `partner()` → arrive at the partnered room socket.
Both directions go through the *same* reciprocal lookup, so a socket that has no
partner (sealed/collapsed) is simply un-crossable — no special case.
`Crossing::EnteredHallway { from, to }` becomes socket/corridor-shaped; keep a
thin pair-shaped accessor for deferred consumers if it reduces churn.

### 4. Source `PlaceGeom.gaps` from the active socket set
The geometry builders (`teleport::geom::hallway_geom_with_slots*`, `room_geom*`)
already take slots. Feed them the corridor's authored sockets and their *current
attachment state* so an unattached/sealed socket yields a solid wall (no
`is_passage` gap) and an attached one yields a crossable gap. Because
`place_structural_primitives` and `place_rapier_scene` already derive collision
from `PlaceGeom.gaps`, render + Rapier apertures then follow the socket set for
free — that is the invariant this phase exists to guarantee.

### 5. Move reroute/decohere onto attachments
Replace `Nav`'s pair-keyed `effective_version` / pin logic
(`teleport/nav.rs`) as the *rewiring authority* with attachment updates on the
live `JunctionTopology`. A reroute detaches a room socket from corridor A and
attaches it to corridor B **as one operation** (both sides or neither) — the
topology already enforces this; the job is to route decohere through it. Keep the
variation-reroll behavior (unobserved halls re-roll presentation) intact.

### 6. Regression fixtures (the falsifiable evidence)
Add tests that would have caught the historical bug:
- **Traversable ⇒ never walled:** for every active attachment, the room-side gap
  and the corridor-side gap are both `is_passage`, and both appear as openings in
  `place_structural_primitives` (no solid segment spans the aperture).
- **Sealed ⇒ never crossable:** an unattached/collapsed socket produces no
  passable gap and no `partner()`, so `apply_crossing` cannot transition through
  it.
- **Reciprocity:** crossing a socket and crossing back returns to the origin
  place at the mirrored socket (round-trip identity).
- **Atomic reroute:** after a reroute, no socket is attached to two corridors and
  no attachment is one-sided (assert against `JunctionTopology` directly).

---

## Success criteria (parent rejects the phase if any fails)

- [ ] `teleport::ThresholdSlotId` is gone; one `ThresholdSlotId` (u16) workspace-wide.
- [ ] `Place::Hallway` no longer carries `(from, to)` as identity; corridor
      identity is a `CorridorId`, and `Place::place_id()` exists.
- [ ] Every transition resolves through `JunctionTopology::partner` — no code path
      reconstructs connectivity from a room pair to decide where a crossing goes.
- [ ] Render gaps, occlusion, crossing volumes, and Rapier apertures all read the
      same active socket set (single source: the corridor's attachments →
      `PlaceGeom.gaps`).
- [ ] The four regression fixtures above exist and pass; at least one demonstrably
      fails if you revert step 4 (prove it to the parent).
- [ ] `cargo fmt --all` clean, `cargo clippy --workspace --all-targets` warning-free,
      `cargo test --workspace` green.
- [ ] `arch_check` ratchets hold (no new sim→presentation dependency; no `Entity`
      used as durable identity).
- [ ] Existing single-exit maps play identically (determinism: the default map's
      replay hash is unchanged, or the change is explained and re-pinned).
- [ ] Reset/exit leaves no Rapier bodies or colliders behind (Match lifecycle).

## Falsifiable evidence to hand back

1. The regression-fixture run output (the four tests green) **and** a one-line
   note of which test fails when step 4 is reverted.
2. A determinism note: default-map replay hash before/after (unchanged, or
   re-pinned with reason).
3. A short prose walk of one crossing, naming the actual sockets/corridor the
   partner lookup resolved — proving the pair reconstruction is really gone.

## Out of scope (Phase 75+)

Consumer migration (bots/nav/guardian/items/ambience/observation/tacmap/previews/
diagnostics/replay/map-validation), the full procedural corpus sweep, and any
*authored* multi-exit map. Phase 74 must **support** multi-exit corridors in the
contract; **shipping** a multi-exit playable fixture is Phase 76.

## Key files

- `game/src/teleport/mod.rs` — `Place`, `DoorGap`/`ThresholdLink`, slot types.
- `game/src/teleport/transition.rs` — `apply_crossing`, `Crossing`, alignment,
  `place_structural_primitives`, `place_rapier_scene`.
- `game/src/teleport/nav.rs` — decohere/pin authority (moves onto attachments).
- `game/src/teleport/geom.rs` — slot-aware geometry builders.
- `crates/observed_facility/src/junction.rs` — `JunctionTopology` (consume as-is).
- `crates/observed_facility/src/map_spec.rs` — `MapCorridor`, `MapSpec::corridors`.
- `crates/observed_core/src/lib.rs` — canonical `PlaceId`/`CorridorId`/`ThresholdId`.
