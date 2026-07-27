# Phase 111 — Rooms Belong to Districts

**Status:** `[x]` — landed 2026-07-27. Closes bug backlog #15 and #16.

Two findings, and the first one is the kind that is embarrassing to write down.

## The geometry already existed

`blueprint_cell_archetype(role, cell_index)` discarded both of its arguments and
returned `"sanctuary"`. Every room cell of every role, in every district, was the
same single-hex shape repeated across its footprint — which is why a three-hex
Decision room read as three rooms rather than one.

The fix was not to author anything. `room_double_west`, `room_double_east`,
`room_double_nw`, `room_double_se`, `room_tri_a/b/c`, `room_fork_a/b/c/d`,
`room_atrium_lower` and `room_atrium_upper` have been generated in every register
since Arc L. Their names map one-to-one onto the blueprint footprints. Nothing
ever asked for them, and `compatibility_archetype` flattened every `room_*` name
back to `sanctuary` on the way *in*, so they could not have been reached even by
accident. Two stubs, facing each other across a seam, cancelling a whole family
of geometry.

The pairing was checked by the machine rather than by eye: Phase 110's coverage
gate demands an exact tile for every `(archetype, signature)` in every register,
and a wing paired to the wrong cell produces a signature nothing satisfies. It
passed first time on all fourteen.

## Rooms now belong somewhere

A role-to-district table, honoured while stamping. This is the legibility payoff
the arc is for — recognising a district should tell a player what it holds, so
heading somewhere is a decision rather than a wander:

| role | districts | why |
|---|---|---|
| Decision | Liminal Grid, Infinite Gallery | a hub for reading doors wants the choice visible before it is taken |
| DecoherenceFork | Shadow Screen, Thinning | an unstable junction belongs where the architecture is already uncertain |
| AnchorCheckpoint | Monolith, Facet Monument | freezing thresholds is a structural act; put it against structure |
| Keystone | Facet Monument, Wellshaft | a side objective should be somewhere you can describe to a teammate |
| DualStation | Megastructure, Institutional | two operators, industrial scale |
| GuardianControl | Institutional, Shadow Screen | redirecting guardian pressure is administration |
| Monitor | Overlit Grid, Infinite Gallery | an information room wants light and sightlines |
| Recovery | Thinning, Liminal Grid | somewhere to stop, in the quietest district |

Start and Exit are unbound: they are pinned to the spawn and exit coordinates and
moving them would break the forced route.

**It is a preference, not a constraint, deliberately.** A seed can put a role's
districts in an awkward corner, fill them with other rooms, or leave one barely
represented at the levels a room could fit. Refusing to stamp would cost the
facility a room and could fail the room-count contract outright — a much worse
outcome than a Monitor turning up somewhere unexpected. Stamping tries the
preferred districts across every candidate coordinate first, then the full field.
Two passes rather than a sort, so a role that cannot be placed in its own
districts still gets the whole grid rather than the leftovers.

Measured across twelve production seeds: **89 bound rooms, 89 in their own
district, 0 fallbacks.** The escape hatch has not yet been needed, which is the
right shape — present and unused.

## The largest authored room had never been placed

`DecoherenceFork` — four hexes, and `room_decoherence_fork.map` is the biggest
authored module in the corpus — was absent from the stamping pool. Adding it was
not enough, because of a second and quieter fault:

```rust
let span = (config.max_rooms - config.min_rooms).max(1) as u64;
let target = config.min_rooms + (rng.next_u64() % span) as usize;
```

For the production contract of `min_rooms: 9, max_rooms: 10`, `span` is 1 and
`rng % 1` is always 0. The target was **always** 9, `max_rooms` had never once
been reached, and the last role in the pool could never be drawn. An inclusive
pair of fields treated exclusively.

With the range made inclusive and the fork added at the end of the pool — last
because it is the hardest to fit — it appears in 5 of 12 seeds, in exactly its
two districts. Rare, which is right for the largest room in the game, rather than
impossible.

## TeleportRelay: decided, not deferred

**Not wired**, and the reason is recorded in the pool alongside it. Its blueprint
exists and the deprecated `full_wfc` path requires a pair of them, but the hex
match has no teleport-pad mechanic at all — `sync_teleports_to_bodies` reconciles
spawn, setback and escape moves, nothing a player can use. Stamping one would
spend a room slot on a room that does nothing, which is worse than leaving it
out. The blueprint stays because `full_wfc` still names it.

## The map already says it

The plan asked for the room-district binding to be surfaced to the Phase 105 map.
It already is: the map's colour channel is the district accent for every cell
including room cells, so a discovered Guardian Control in Wellshaft reads as a
room, in Wellshaft's colour, next to Wellshaft's corridors. Adding a second
signal for the same fact would spend a channel to say something the map says
already.

## Measured

Seed `0xa11ce3d000000008`, production `28 x 20 x 10`, 5 487 cells.

| archetype | P109 | P110 | **P111** |
|---|---|---|---|
| expanse | 21.5 % | 21.5 % | 24.6 % |
| corner | 23.3 % | 23.3 % | 23.6 % |
| shaft | 17.7 % | 17.7 % | 16.5 % |
| junction | 16.4 % | 16.4 % | 16.3 % |
| ramp (both halves) | 15.3 % | 15.3 % | 13.2 % |

Unlike Phase 110, the census *does* move here, and it should: the room-count
range now reaches its upper bound, so some seeds stamp a tenth room and the
solver has less lattice to fill around it.

## Three fixtures re-pinned

`GATE_SEED`, `MUTATION_SEED`, and three room tests. The room tests were asserting
`archetype == "sanctuary"` — they encoded the stub, so they had to change with
it; the assertions now read the expected archetype from
`blueprint_cell_archetype` rather than naming a string, so they cannot drift
again. The two seeds moved because the room-count fix changes every layout, and
were re-found with the scanners already in the test file.

## What this phase did not do

**Backlog #18 is still open** — a two-level room's internal vertical link is not
traversable — and it is now unblocked rather than fixed. The Guardian Control
atrium finally has geometry of its own (`room_atrium_lower` / `room_atrium_upper`
instead of two copies of a single-hex room), which is the thing a stair could be
hung on, and Phase 109 already made the bot able to walk any climb a tile
declares: `vertical_command` gates on the *port class*, not the archetype, so an
atrium that ships a `StairSpine` would simply be climbed.

What remains is the stair itself, and it is a geometry task the size of Phase
109's tower work rather than a wiring change. The atrium's lower cell declares
doors on all six faces and is one level tall, so a flight cannot run along a wall
without blocking a door, and cannot reach the gallery without the tile spanning
two levels. Doing it badly — a stair that blocks the room's own doors, or that
lands a step too high to climb — is worse than scheduling it. The Phase 107 guard
in `topology::is_connection_open` stays until then; it is correct, and it costs
nothing but a connection that does not exist.

## Evidence

`docs/evidence/arc_o/phase_111/`.

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/arc_o/phase_111"; cargo dev-run -p iso_observer_lab
```

## Hand-off to Phase 112

- Co-op is the last build phase, and the wire format is the real work: at sixteen
  seats a `FrameBundle` is roughly 1 808 bytes against a 1 200-byte datagram
  limit, so it does not merely degrade — `encode` returns `Oversized`.
- Nothing in this phase touched the roster or the wire, so that estimate stands
  as surveyed.
