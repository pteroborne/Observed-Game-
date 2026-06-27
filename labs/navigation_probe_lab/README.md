# navigation_probe_lab (Phase A8)

Bevy asset-integration roadmap, Phase A8 — **Navigation Probe**.

> Can imported/authored geometry produce useful bot or debug navigation **without
> taking ownership of the facility graph?**

## Asset

`vleue_navigator 0.15.0` (the Bevy `0.18` line: it declares `bevy ^0.18.0` with
the `bevy_render`/`bevy_asset`/`bevy_log` features, so it links cleanly against
this workspace's pinned `0.18.1`; the later `0.16+` line targets newer Bevy and is
rejected). Pinned `default-features = false` to drop the crate's
`debug-with-gizmos` default, which would otherwise pull `bevy/bevy_gizmos` as a
hard dependency of the asset.

The crate is used as a **navmesh builder + polyanya path query only**
(`NavMesh::from_edge_and_obstacles` + `NavMesh::path`). The auto-updater plugin
(`NavmeshUpdaterPlugin`, built to rebuild obstacle meshes from physics colliders)
is **not** adopted — this lab has no physics layer and authors its obstacles
directly. That mirrors the prior asset labs' stance: consume the asset's data
model, not the part that fights the architecture.

## What it proves

The lab keeps a hard, one-way split between truth and derivation:

- **`facility.rs` — authoritative.** Four rooms (A/B/C/D) divided by a cross of
  walls and joined by four doors (`AB`, `AC`, `BD`, `CD`), forming a 4-cycle.
  Connectivity is decided **here**, by breadth-first search over *open* doors.
- **`nav.rs` — derived consumer.** Builds a `vleue_navigator` navmesh from the
  facility's permanent walls plus a plug for every *closed* door, then routes over
  it with polyanya. It reads door state and never writes back.

Because the navmesh is rebuilt from the authoritative door state, a closed door
becomes a solid obstacle: the route either detours through the other side of the
loop or fails entirely. The navmesh can never route through a door the facility
says is shut. The lab's headline test sweeps **all 16 door configurations × every
ordered room pair** and asserts the navmesh agrees with the graph about
reachability and never produces a walk that crosses a closed door.

A debug **bot** (the "agent" the phase asks for) walks the current route to the
goal, demonstrating the derived path is physically traversable. It consumes the
route; it is never authoritative.

## Run

```powershell
cargo run -p navigation_probe_lab
```

Capture evidence (renders, writes the PNG, exits):

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/navigation_probe_lab.png"; cargo run -p navigation_probe_lab
```

The capture demo closes door `AB`, so the A→D route is shown rerouting through C.

## Controls

| Key | Action |
| --- | --- |
| `1`–`4` | Set goal room A / B / C / D (start is fixed at A) |
| `Z` `X` `C` `V` | Toggle door AB / AC / BD / CD |
| `R` | Reset (all doors open, probe A→D) |
| `F1` | Toggle the debug overlay |

## Reading the view (top-down schematic)

- Blue wall blocks = permanent structure; **green door = open, red door = closed**.
- Gold polyline = the navmesh route (`vleue_navigator`).
- Cyan diamond/square = the probe start / moving bot; green diamond = goal.
- Overlay reports the navmesh route, the authoritative graph route, whether they
  agree, the bot state, live entity counts, and a `[PASS]/[FAIL]` health line.
  `[PASS]` requires one camera + one UI root, the full static scene, **and**
  `nav == graph` agreement for the current query.

## Manual verification

1. Launch the lab. The overlay shows `A -> D`, navmesh route `A>B>D` (or `A>C>D`),
   matching the graph route, `nav==graph agree`, `[PASS]`.
2. Press `Z` to close door `AB`. The AB door turns red, the gold route snaps to
   `A>C>D`, the bot re-walks the detour, and the overlay still reads `[PASS]`.
3. Press `X` to also close door `AC`. Room A is now isolated: the navmesh route
   reads `unreachable`, the graph route also reads `unreachable`, agreement holds,
   `[PASS]`.
4. Press `R`. All doors reopen, the probe returns to `A -> D`, and every entity
   count returns to its baseline (no leaks).

## Tests

```powershell
cargo test -p navigation_probe_lab
```

- `facility.rs` — graph is a 4-cycle, closing a door forces the detour, isolating
  a room disconnects it, obstacle set grows per closed door, open-walk validation.
- `nav.rs` — **navmesh agrees with the authoritative graph for every door
  config**, reroute around a closed door avoids the blocked room, a closed door
  blocks the path entirely, construction/pathfinding is deterministic.
- `lib.rs` — boots with camera/UI/scene and a valid agreeing route, closing a door
  in-app reroutes in agreement, isolating the start is unreachable in agreement,
  reset rebuilds without leaking entities, the bot walks the route to the goal.

## Promotion decision

Per the roadmap, game adoption is **deferred** until bots/AI need physical routing
in the first-person facility — the current match brain does not. This lab proves
the integration shape (authored geometry → derived navmesh → route that respects
door state, graph stays authoritative) and is the reference for that future work.
Kept lab-local; `vleue_navigator` is isolated to this lab with no production-crate
or `game` dependency.
