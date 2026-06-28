# navigation_probe_lab (Phase A8)

Bevy asset-integration roadmap, Phase A8 - **Navigation Probe**.

> Can imported/authored geometry produce useful bot or debug navigation **without
> taking ownership of the facility graph?**

## Asset

`vleue_navigator 0.15.0` is the Bevy `0.18` line: it declares `bevy ^0.18.0` with
the `bevy_render`/`bevy_asset`/`bevy_log` features, so it links cleanly against
this workspace's pinned `0.18.1`; the later `0.16+` line targets newer Bevy and is
rejected. Pinned `default-features = false` to drop the crate's
`debug-with-gizmos` default, which would otherwise pull `bevy/bevy_gizmos` as a
hard dependency of the asset.

The crate is used as a **navmesh builder + polyanya path query only**
(`NavMesh::from_edge_and_obstacles` + `NavMesh::path`). The auto-updater plugin
(`NavmeshUpdaterPlugin`, built to rebuild obstacle meshes from physics colliders)
is **not** adopted: this lab has no physics layer and authors its obstacles
directly. That mirrors the prior asset labs' stance: consume the asset's data
model, not the part that fights the architecture.

## What It Proves

The lab keeps a hard, one-way split between truth and derivation:

- **`facility.rs` - authoritative.** Four rooms (A/B/C/D) divided by a cross of
  walls and joined by four doors (`AB`, `AC`, `BD`, `CD`), forming a 4-cycle.
  Connectivity is decided here by breadth-first search over open doors.
- **`nav.rs` - derived consumer.** Builds a `vleue_navigator` navmesh from the
  facility's permanent walls plus a plug for every closed door, then routes over
  it with polyanya. It reads door state and never writes back.
- **`threshold.rs` - WFC-shaped continuity contract.** Room templates expose a
  fixed set of threshold slots. The live graph assigns destinations to slots.
  Pressing `T` anchors room A, collapsing its complete visible assignment table:
  A keeps exactly those thresholds, reciprocal endpoints keep pinned relations,
  and no other room can grow a new inbound threshold into locked A.

Because the navmesh is rebuilt from the authoritative door state, a closed door
becomes a solid obstacle: the route either detours through the other side of the
loop or fails entirely. The navmesh can never route through a door the facility
says is shut. The headline test sweeps all 16 door configurations across every
ordered room pair and asserts the navmesh agrees with the graph about reachability
and never produces a walk that crosses a closed door.

A debug **bot** walks the current route to the goal, demonstrating the derived path
is physically traversable. It consumes the route; it is never authoritative.

## Run

```powershell
cargo run -p navigation_probe_lab
```

Capture evidence:

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/navigation_probe_lab.png"; cargo run -p navigation_probe_lab
```

The capture demo closes door `AB` and anchors room A, so the A-to-D route is shown
rerouting through C while the threshold overlay shows A's collapsed assignment.

## Controls

| Key | Action |
| --- | --- |
| `1`-`4` | Set goal room A / B / C / D (start is fixed at A) |
| `Z` `X` `C` `V` | Toggle door AB / AC / BD / CD |
| `T` | Toggle room A anchor/threshold collapse |
| `R` | Reset (all doors open, probe A-to-D) |
| `F1` | Toggle the debug overlay |

## Reading The View

- Blue wall blocks = permanent structure; green door = open, red door = closed.
- Gold polyline = the navmesh route (`vleue_navigator`).
- Cyan diamond/square = the probe start / moving bot; green diamond = goal.
- Purple threshold diamonds = collapsed/tethered assignments. Green threshold
  diamonds = live assignments.
- Overlay reports the navmesh route, the authoritative graph route, whether they
  agree, the threshold assignment table, room anchors, the bot state, live entity
  counts, and a `[PASS]/[FAIL]` health line.
- `[PASS]` requires one camera, one UI root, the full static scene,
  `nav == graph` agreement for the current query, and the threshold continuity
  audit passing.

## Manual Verification

1. Launch the lab. The overlay shows `A -> D`, a navmesh route such as `A>B>D`
   or `A>C>D`, matching the graph route, `nav==graph agree`, `[PASS]`.
2. Press `Z` to close door `AB`. The AB door turns red, the gold route snaps to
   `A>C>D`, the bot re-walks the detour, and the overlay still reads `[PASS]`.
3. Press `X` to also close door `AC`. Room A is now isolated: the navmesh route
   reads `unreachable`, the graph route also reads `unreachable`, agreement holds,
   `[PASS]`.
4. Press `R`, then press `X` to close `AC`, then press `T` to anchor room A with
   only `AB` visible. Press `X` again to reopen `AC`, and press `Z` to close `AB`.
   The overlay still shows room A collapsed to `B`; room B keeps the reciprocal
   threshold to A; room C does not gain a new threshold into locked A; `[PASS]`.
5. Press `R`. All doors reopen, the probe returns to `A -> D`, anchors clear, and
   every entity count returns to its baseline.

## Tests

```powershell
cargo test -p navigation_probe_lab
```

- `facility.rs` - graph is a 4-cycle, closing a door forces the detour, isolating
  a room disconnects it, obstacle set grows per closed door, open-walk validation.
- `nav.rs` - navmesh agrees with the authoritative graph for every door config,
  reroutes around closed doors, blocks fully disconnected routes, and is
  deterministic.
- `threshold.rs` - threshold slot counts stay fixed across graph changes, room
  anchors freeze the exact visible assignment table, reciprocal endpoints retain
  pinned relations, new inbound thresholds into locked rooms are rejected, and
  preview/cross/arrival use the same collapsed assignment.
- `lib.rs` - boots with camera/UI/scene and a valid agreeing route, closing a door
  in-app reroutes in agreement, isolating the start is unreachable in agreement,
  threshold locking works inside the app resource flow, reset rebuilds without
  leaking entities or anchors, and the bot walks the route to the goal.

## Promotion Decision

Per the roadmap, game adoption of `vleue_navigator` is deferred until bots/AI need
physical routing in the first-person facility. The threshold rule model is lab-local
for now but is intentionally shaped like a future production constraint system:
fixed slots, mutable assignments, explicit collapse/anchor tables, and audits that
prove preview, crossing, and arrival read the same assignment.
