# Phase 75 — Room/Hall Variation Parity & Consumer Migration

**Status:** `[ ]` — next after Phase 74 (`013af3a`).
**Branch:** `codex/rapier-threshold-integration`.
**Nature:** a serial **spine** (75a) followed by a parallel **consumer fan-out**
(75b). The spine redefines how connectivity is stored; the fan-out migrates the
readers. Do **not** run fan-out clusters before the spine has landed and
verified — they would migrate against a moving contract.

---

## Goal

Run the production Rapier path across the complete procedural corpus and migrate
**every remaining pair-assuming consumer** onto stable place/threshold
identities, so no gameplay/presentation/evidence code decides connectivity from a
`(from, to)` room pair. Prove — with corpus tests — that rendered and collidable
structural segments agree, every active aperture crosses both ways, sealed
sockets cannot be crossed, reroutes stay solvable, and reset/exit leaves no Rapier
bodies behind.

Phase 74 established the contract (`CorridorId` identity, `JunctionTopology::
partner`-resolved crossings, socket-sourced apertures) but deliberately left the
connectivity **authority** pair-keyed and the consumers unmigrated behind a
documented shim. Phase 75 finishes that.

---

## 75a — The spine (serial, one agent, lands first)

**Move the reroute/connectivity authority onto a persistent, sim-owned
`JunctionTopology`.** Today `teleport::Nav` is a per-frame read-only snapshot the
producer rebuilds each tick from pair-keyed state; Phase 74 rebuilds a throwaway
`JunctionTopology` per crossing. The spine makes the topology the durable source
of truth and has the producer derive `Nav` (or its replacement projection) from
it.

### Files (the spine owns these — no fan-out cluster may touch them)
- `game/src/sim/nav.rs` — the primary `Nav` producer (`~lines 217–280`); pulls
  `items.pins()`, `connection_slots`, `sealed_slots` from sim state.
- `game/src/teleport/nav.rs` — `Nav`, `PinnedEdge`, `effective_version`,
  `is_tethered`, `slot_for` (the pair-keyed API being retired/reshaped).
- `game/src/items.rs` — anchor-torch pins as the pin source (`items.pins()`).
- `game/src/sim/state.rs` — wherever the persistent topology should live.
- The match director / brain reroute path that decoheres the graph and bumps
  `version` (find via `version` bumps + reroute; keep determinism identical).
- `game/src/screens/match_runtime/crossing.rs::frozen_nav` and
  `game/src/evidence/snapshot.rs` `Nav {...}` producers — reshaped to match.

### Requirements
1. A persistent `JunctionTopology` (or an equivalent sim-owned attachment ledger)
   is the connectivity authority. Reroutes mutate it **atomically** (both sockets
   of an attachment move together or neither) — reuse `JunctionTopology`'s
   existing half-rewire rejection rather than a parallel invariant.
2. Anchor-torch pins (frozen variation) and collapse seals express in terms of
   attachments/sockets, not `(a, b)` keys. Preserve exact behavior: a pinned edge
   still freezes its variation; a sealed socket still renders rubble and is
   un-crossable; `is_tethered` still lights the anchored doorway frame.
3. **Retire the Phase-74 pair-reconstruction fallback** in
   `teleport::transition::apply_crossing` (the `corridor_id_for(room, gap.target)`
   / `gap.target` branches). After 75a, the topology is always populated, so the
   `partner()` lookup is the only path.
4. **Determinism is sacred.** The default-map replay hash
   (`all_on_default_match_director_digest_is_pinned`) must stay unchanged, or be
   re-pinned with an explicit written reason. `generated_maps_run_complete_
   matches_across_a_seed_corpus` stays green.
5. Reshape `Nav`'s public surface so consumers can read stable identities
   (`PlaceId`/`CorridorId`/`ThresholdId`) — but keep thin pair accessors alive
   **only** if a fan-out cluster still needs them for one wave; note each one so
   the owning cluster removes it.

### 75a success criteria
- [ ] Sim owns a persistent `JunctionTopology`; `Nav` derives from it.
- [ ] Pins + seals expressed as socket/attachment state; behavior identical.
- [ ] `apply_crossing` has no pair-reconstruction branch left.
- [ ] Default-map determinism digest unchanged (or re-pinned w/ reason).
- [ ] fmt clean, clippy `--workspace --all-targets` 0 warnings, `cargo test
      --workspace` green, `arch_check` ratchets hold.

---

## 75b — Consumer fan-out (parallel, after 75a verifies + commits)

Four **file-disjoint** clusters, each a separate agent in an **isolated
worktree**. Each migrates its files off any remaining pair-assumption onto the
stable identities the spine exposes, and each keeps the full suite green.

### Cluster A — AI & pathing
`game/src/bot.rs`, `game/src/guardian.rs`, `game/src/evidence/capture/bot_pov.rs`.
Bot pathing, guardian targeting, and the bot-POV capnav read connectivity by
pair; move them to place/threshold identity + `JunctionTopology` queries
(`corridor_rooms`, `reachable_rooms`, `partner`).

### Cluster B — Presentation
`game/src/screens/place/{preview,factory,animate}.rs`,
`game/src/screens/match_runtime/{crossing,ambience}.rs`,
`game/src/screens/audio.rs`, `game/src/screens/hud.rs`, `game/src/tacmap.rs`.
Doorway/passage previews, crossing spawn, ambience/audio keyed by hall, tac-map
edges, HUD readouts — all read the active socket set, never a reconstructed pair.
(`crossing.rs::frozen_nav` is spine-owned; this cluster consumes it.)

### Cluster C — Evidence & diagnostics
`game/src/evidence/{audit,snapshot}.rs`,
`game/src/evidence/capture/scenarios.rs`.
Visual audit, evidence snapshots, capture scenarios — migrate their pair reads;
keep captures byte-comparable where the arc's evidence gate depends on them.

### Cluster D — Sim, validation & the corpus gate
`game/src/sim/{knowledge,sightings,replay}.rs`, `game/src/map_validation.rs`,
`game/src/hallway.rs`, **plus the new corpus parity tests** (see below). Observation
knowledge, sightings, replay, and map validation onto stable identities; this
cluster also owns the corpus test gate because it is the parity proof.

### Corpus parity tests (Cluster D deliverable — the falsifiable evidence)
Across every room role and seeded footprint, and Chicane / Colonnade / Maze (DFS
and WFC interiors) / Gantry / Wellshaft at every supported elevation and entrance:
- rendered structural segments and Rapier structural segments agree;
- every active aperture is crossable in **both** directions;
- sealed sockets cannot be crossed;
- reroutes keep the graph solvable (existing solvability invariant);
- reset/exit leaves **no** Rapier bodies or colliders behind (lifecycle).

---

## Worktree & commit discipline (parent-enforced)

- **Agents never commit.** The parent reviews each agent's diff, runs the full
  verification itself, and commits. 75a is one commit; each 75b cluster is its
  own commit (parent resolves any incidental overlap at commit time).
- 75b clusters run in **isolated worktrees** (`isolation: worktree`) so they don't
  see each other's uncommitted edits. Their file sets are disjoint by design; the
  only shared file is `game/src/tests.rs`, which is **append-only** for concurrent
  clusters — never edit an existing test another cluster might also touch.
- If a cluster discovers it needs a spine-owned file, it **stops and reports** —
  the parent folds that into 75a or serializes it, never lets two agents edit one
  file.

## Verification (parent, before each commit)

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets   # warnings are failures
cargo test --workspace
```

Plus, for Cluster D / the arc-facing evidence: the corpus parity suite green and a
determinism note (default-map hash unchanged or re-pinned).

## Out of scope (Phase 76)

Shipping a *playable* multi-exit fixture (Gantry side exit to a different room,
multi-threshold Wellshaft), the viewed first-person traversal capture through
every branch, and the user playtest. Phase 75 must make the corpus **provably
correct**; Phase 76 makes a multi-exit map **playable** and closes the arc.

### ⚠️ Phase 76 prerequisite discovered in 75a (must land before the gate)

`Place::Hallway { from, to, variation }` still structurally holds exactly **two**
rooms, and `place_junction` attaches exactly the `(from, to)` pair for a hallway.
The identity/topology layer (`CorridorId`, `JunctionTopology`) supports N sockets,
but the **runtime `Place` enum cannot represent a 3+-exit corridor**. Phase 74's
"support corridors with two or more exits" is met at the contract level only.
Before Phase 76 can ship a genuine multi-exit fixture, `Place::Hallway` must be
generalized to carry `{ corridor: CorridorId, entered_socket }` (with `from`/`to`
derived for the deferred pair-shaped accessors, or those accessors retired). This
is a **contract change**, so it lands as Phase 76's opening step (or a 75c spine
addendum) — never mixed into a parallel consumer cluster. The entire current
corpus is 2-exit, so 75b's parity work is unaffected.
