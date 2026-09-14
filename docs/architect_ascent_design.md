# Architect Ascent — Canonical Game Design

**Status:** canonical north star, recorded 2026-09-07. Amended 2026-09-14 with
disturbance waves, minor Guardians, floor power, the kinetic tool, and the emergency
requisition; those additions are canon on the same footing as the original text.

**Supersedes:** earlier gameplay plans wherever they describe a precomposed race as
the final game. Their implementation and playtest records remain valid evidence.

**Premise:** several teams build and traverse one unstable vertical facility while
a player-operated Rogue AI tries to put every loyal Observer in jail.

## 1. Match shape

Each loyal team has one dedicated, non-embodied **Architect** and one-to-three
first-person **Observers**. The facility, its Guardians, and its mutable topology
are shared by every team. All loyal Observers begin on floor one.

The first team to bring every one of its remaining loyal Observers to the summit
objective wins. At least one loyal Observer must finish. A player corrupted by the
void leaves the team's summit quorum; a team with no loyal Observers is eliminated
and its Architect becomes a spectator.

All corrupted players operate one **Rogue AI** faction. Rogues win immediately when
every remaining loyal Observer across all teams is simultaneously jailed. Guardian
capture resolves before summit completion when both would occur on the same fixed
tick.

There is no direct player damage. Teams compete by building and destabilizing
routes, holding information in view, operating shared doors, deploying equipment,
and surviving the facility.

## 2. Architect knowledge, cards, and commands

### Team knowledge

An Architect sees only the structure and actors recorded in their team's map
knowledge. Current observation and anchor states remain visible while known, and an
unresolved edge discovered by the team may appear as a construction target. The
view never reveals hidden current structure or rival positions.

An Architect may target any known location, not merely the team's present frontier
and not merely tiles the team originally placed. The authoritative simulation
rechecks the current state. If stale knowledge offers a tile that has since become
observed, occupied, anchored, invalid, or permanently collapsed, the command is
rejected without consuming its card or starting its cooldown.

### The five-card hand

Each faction draws from its own deterministic, seeded, finite-composition deck. A
spent or discarded card enters a discard pile; an empty draw pile reshuffles that
discard pile deterministically. The hand is topped off to exactly five immediately
after a play or redraw.

One mixed loyal hand contains:

- ordinary tile cards, defined primarily by branch count and district;
- deployable door cards;
- ascent-room cards;
- redraw rooms, teleporter stations, and other special-room cards.

Refill must leave at least one card with a legal target in a currently placeable
district whenever such a target exists. Cards useful only on a fully collapsed floor
are recycled before refill. This prevents a dead hand while leaving hand quality and
redraw access strategically meaningful.

A redraw room is district-matched like any other room. Once discovered and placed,
an occupying loyal Observer may activate it once for their team. Their Architect
may discard any number of cards and immediately refill to five. This does not start
or bypass the placement cooldown.

An **emergency requisition** is the paid alternative to that room. Any Architect may
play it from hand at any time: discard any number of cards and immediately refill to
five. Its price is one additional major Guardian released on the floor that team's
Observers currently occupy, selected deterministically from that floor's Guardian
budget. The release is public to every faction. A requisition neither starts nor
bypasses the placement cooldown.

The two redraw paths are deliberately different bargains. The room is earned by
Observers physically reaching and holding a location, and costs nothing. The
requisition is available instantly and costs pressure. Because refill already
guarantees a live card, neither path exists to rescue a dead hand: both buy hand
*quality*, and only the requisition may be taken under duress. A team that panics
tells every rival it panicked.

### Placement

A loyal card play starts that team's one shared Architect cooldown. The initial lab
default is `300` fixed ticks (five seconds at 60 Hz); it remains match configuration
until playtesting establishes the production value. The Rogue faction has one shared
hand and an identical shared cooldown regardless of how many Rogue operators exist.

For an ordinary tile card:

1. The card fixes branch count and district.
2. The Architect chooses a known target and one of six rotations.
3. A new tile must attach to at least one existing boundary; a replacement must
   retain at least one locally valid attachment.
4. The target floor's district must match the card.
5. Seed, card identity, target, rotation, and world revision deterministically select
   the compatible authored WFC variant.

A target is mutable only when it is not observed, occupied, anchored, part of the
prison core, or on a completely collapsed floor. These rules apply equally to
ordinary tiles, special rooms, ascent rooms, and teleporter stations. A placed object
does not acquire hidden immunity merely because it is important.

## 3. Stability, floors, and traversal

### Contradictions are play

A placement is accepted when it fits its selected boundary locally even if its other
constraints make the wider WFC state impossible. The contradiction becomes visible
instability rather than an error dialog or silent solver failure.

After a warning, implicated tiles retract toward void in a deterministic,
outward-moving order. The initial lab cadence is one tile every `180` fixed ticks
(three seconds). Any Architect able to see and target the affected area may play a
compatible card to repair the unresolved boundary. Repair cancels retractions that
have not committed; already removed tiles must be rebuilt normally.

When every non-prison tile on a floor has retracted, that floor becomes permanently
collapsed. No faction may place new tiles there. The central prison core is excluded
from the collapse count and remains present.

### Disturbance and waves

Construction wakes the facility. Every floor carries a **disturbance** value that
rises when its architecture changes and decays with time. Crossing a threshold
releases a wave of minor Guardians (section 4) onto that floor and subtracts the
threshold, so sustained pressure produces repeated crescendos rather than one
unbounded swarm.

Contributions are deliberately uneven:

- an accepted, locally consistent placement raises disturbance slightly;
- a contradiction, and each committed retraction, raises it sharply;
- a Rogue play raises it more than the equivalent loyal play;
- height raises both the decay floor and the wave size, so upper floors ride
  permanently hotter than floor one.

Disturbance is a budget an Architect spends, never a fine for playing. Spawning
pressure from each individual placement would tax the loyal Architect for performing
their only verb, and would charge the Rogue for something they already want. The
meter exists so that building a route is affordable, and so that *instability* is
what summons the horde.

This is also what finally prices repair. Leaving a contradiction to consume a floor
and repairing it immediately are both expensive, in different currencies, and the
Architect must choose while their own Observers stand on the result.

Initial lab values are match configuration. Only the shape above — uneven
contributions, decay, repeating thresholds, height scaling — is canon.

### Districts and ascent

Every playable floor has exactly one district. District is both an architectural
identity and a card constraint; off-district cards cannot be used on that floor.
An ascent-room card matching the current floor must be legally placed to establish
the physical route upward. Reaching a higher floor makes its district available to
the team's future hand refills.

Height, rather than district identity alone, raises the hazard budget. Upper-floor
catalogues increasingly admit open ledges, narrow bridges, unrailed balconies,
retracting surfaces, and tiles that arm after being crossed. A triggered tile
telegraphs and waits until it is neither occupied nor observed before retracting, so
hazards threaten return routes without violating observation safety.

An ordinary fall lands on lower surviving structure when geometry permits. Only a
fall through the whole surviving stack into true void causes corruption.

### The prison core

The jail is a vertical, non-collapsing maze at the facility's horizontal center. A
Guardian catch sends its target to the lowest prison level. A jailed Observer remains
embodied and loyal, and may navigate the difficult internal route back out.

A teammate can create an easier rescue only after physically reaching a prison-core
boundary. A door card may then open a local exit, or a physically deployed team
portal may connect the prison to another endpoint. Teleport pads and station rooms
never provide unexplored endpoints: both sides must first be reached in person.

## 4. Observation, anchors, doors, and Guardians

Observer occupancy protects the occupied tile. Direct observation freezes visible
tiles, their current connections, and visible Guardians. Existing field-of-view,
occlusion, stable-threshold identity, and anchor rules continue to govern the exact
lock projection.

An anchor is durable unattended protection for its exact connection. It remains the
only permanent player-created lock and keeps its existing diegetic frame signal.

A base threshold is an open physical connection until an Architect deploys a door
card on a known compatible threshold. Any loyal Observer from any team may operate
any deployed door:

- **Open:** the door is traversable, pins its current threshold connection, and
  permits ordinary observation through the opening. A Guardian actually visible
  through it freezes under the normal observation rule.
- **Closed:** the door blocks traversal and line of sight, releases its threshold
  and hidden side to mutation, and allows unseen Guardians to act.
- If mutation or collapse removes a closed door's host threshold, that door is
  consumed with it. Keeping it open would have protected the connection.

The door does not give its deploying team ownership or exclusive access. Its value is
that it is reversible, remotely strategic infrastructure which any loyal first-person
player may contest; it does not replace an anchor.

Guardians use original pyramidal silhouettes and obey observation exactly like the
current physical Guardian. Their capture outcome is jail rather than generic
recovery. Guardian count, patrol behaviour, and per-floor distribution are content
and tuning variables, not new exceptions to the lock rules.

### Major and minor Guardians

The Guardian described above is a **major** Guardian, and observation governs it
exactly as stated: seen, it freezes.

**Minor** Guardians are not frozen by observation. They are weak, numerous, and
released by disturbance waves rather than placed or directed. The immunity is the
point rather than an exception grudgingly granted. A threat that stops when looked at
is answered by facing it, and a horde answered by sweeping a camera is a statue
garden. Splitting the classes gives observation a definite boundary — majors are a
*looking* problem, minors are a *doing* problem — and that boundary is what makes a
second Observer verb necessary instead of decorative. Before it, every Observer verb
is passive: look, stand, anchor, operate, walk.

Minor Guardians belong to the facility. No faction owns them, no Rogue directive
commands them, and they pursue the nearest detected Observer of any team. Rogue
directives continue to address major Guardians only. The Rogue is an architect of
pressure, not a commander of units, and minor Guardians are the neutral hazard that
both factions must route around.

A minor Guardian is destroyed by the environment rather than by damage: the kinetic
tool (section 6) commits it to void, off unrailed geometry, into a retracting tile, or
through a threshold that is then closed. Minor Guardians never enter the prison core,
and are removed with a floor that becomes permanently collapsed.

## 5. Power and darkness

Every playable floor has exactly one **generator** room. It is discovered like any
other room and operated physically, at the tile, by any Observer of any faction. It
has two states and both factions may toggle it. It is not dealt from any hand, and no
card play creates or removes it.

Losing power changes what is true, not merely what is visible:

- recharge stations (section 6) supply no charge;
- deployed doors freeze in their current state and cannot be operated;
- ascent rooms, teleport pads, and station rooms are inert;
- **observation fails at range.** What cannot be seen cannot be frozen.

That last consequence is the mechanic's reason to exist. An unpowered floor is a floor
whose tiles are almost entirely mutable and whose major Guardians are almost entirely
unfrozen. It is at once the strongest play available to the Rogue faction, a loyal
team's deliberate act of desperation, and the reason the generator is the most
contested fixed location on its floor. It is also the one objective an Architect
cannot simply hand their Observers: it must be found and held in person.

Darkness costs observation **range**. It never costs legibility. The Legibility
Contract binds here without amendment, and unpowered atmosphere may not hide
gameplay-critical information. Critical signals keep a documented self-lit minimum
that does not depend on floor power: Observers, major and minor Guardian silhouettes
within close range, anchor frames, door states, contradiction and retraction fronts,
and the generator itself. An unpowered floor is one where truth must be approached,
not one where it is absent.

## 6. The kinetic tool and charge

Every Observer carries one **kinetic tool**: a short-range push and pull acting on
minor Guardians, loose debris, deployable equipment, and teammates. It does no damage
to anything, and it has no effect whatsoever on rival Observers. This game has no
direct player damage, and the tool is chosen precisely so that the affordance to
attempt it never presents itself.

The tool is the Observer's answer to minor Guardians, and it answers them with the
facility. A shove commits a minor Guardian to void, off a ledge or unrailed balcony,
into a retracting tile, or through a threshold that an Observer then closes. What
kills is always architecture. This keeps the Architect central to first-person
survival — an Observer's best weapon is a hole their Architect built — where a
direct-damage weapon would make Observers self-sufficient and sideline the Architect
inside their own information loop.

The tool doubles as traversal and utility. Repositioning debris, boosting a teammate
across a gap, and recovering dropped equipment are intended uses rather than exploits.

### Charge and recharge stations

The tool draws from a finite **charge** pool, restored only at a recharge station:
deployable equipment placed from the Architect's mixed hand, which supplies charge
only while its floor has power.

This is the intended dependency. The generator powers the station, the station charges
the tool, and the tool is the only answer to the horde. Each link is useless alone,
each is contestable, and a break at any point is felt immediately in first person. An
Architect who never places a station has disarmed their own team; a team that loses
its generator has disarmed itself.

A scarce lethal option may exist as low-charge equipment — a hitscan lance dealt into
the mixed hand — so that direct removal is a deliberate card rather than a standing
ability. The default Observer verb remains kinetic.

## 7. The Rogue AI

True-void corruption is immediate and irreversible. The former Observer leaves
first-person play and joins every previously corrupted player at the shared Rogue
board. Corruption is public; this is an asymmetric faction change, not a hidden
traitor role.

Rogue operators see full facility topology, tile stability, contradiction fronts,
and every Guardian. They see a loyal Observer only while that Observer is detected
by a Guardian or an explicit Rogue-controlled sensor. Team Architect views never
inherit Rogue knowledge.

The Rogue deck is a distinct seeded mix of:

- locally fitting tile mutations;
- deployable doors and AI-controlled infrastructure;
- Guardian directives;
- effects that create or accelerate visible instability.

It contains no loyal ascent or rescue rooms. Rogue tile plays obey the same district,
local-fit, observation, occupancy, anchor, collapsed-floor, hand-size, and cooldown
rules as loyal plays. Rogue powers must manipulate the shared rules rather than
silently bypass them.

## 8. Presentation language

The neon-noir Legibility Contract remains binding. The new shape language uses
original geometric constructs: spherical eyeball Observers, pyramidal Guardians,
and recurring cubes, prisms, rings, and polyhedra in architecture and decoration.
Modrons are a mood reference only, not a name or design to reproduce.

Every critical state needs shape, motion, or spatial audio in addition to colour:
known versus unknown, observed versus mutable, anchored, door-open, door-closed,
contradicted, retracting, permanently collapsed, jailed, Guardian-observed, and
Rogue-controlled. Mutation and collapse are events in the world, never merely HUD
text.

The amended mechanics add their own states under the same rule: major versus minor
Guardian (silhouette, not colour alone, since only one of the two answers to being
looked at), floor powered versus unpowered, station live versus dead, remaining tool
charge, and rising disturbance. Disturbance in particular must be legible in the world
before a wave arrives — the facility audibly and visibly wakes — so that a crescendo is
something an Observer can hear coming and an Architect can choose to provoke.

## 9. Prototype and promotion sequence

### A. `architect_lab`: prove the Rogue pressure loop

Build a new resettable lab from the **Rogue Architect perspective** on the real
`HexWfcWorld`, authored catalogue, `HexObservationFrame`, and shared
schematic/cutaway renderers. Use two district floors plus the prison core. The
default session has one human Rogue Architect, autonomous loyal Observers, and
autonomous Guardians. Its question is narrow: is placing tiles and doors to help
Guardians find, isolate, and jail Observers a readable and enjoyable game?

The human consumes the real Rogue five-card hand, targets only legal known cells,
and emits the same authoritative `ArchitectCommand` shape intended for LAN. Prove
global placement and replacement, rotation, cooldowns, doors, locally valid
contradictions, staged retraction, repair, Guardian route assistance, capture, and
the all-jailed Rogue outcome. Loyal ascent construction, rescue, corruption, and
the full multi-team race remain outside this first proof.

Every non-human decision-maker runs a deterministic behavior tree evaluated from
authoritative state at the fixed tick. Trees choose intents; they do not move bodies,
mutate tiles, or bypass card legality directly:

- **Observer tree:** escape jail; hold a visible Guardian; evade immediate danger;
  rescue a reachable jailed teammate; advance toward the summit; explore an unknown
  frontier when no route is known.
- **Guardian tree:** remain frozen while observed; capture an adjacent detected
  Observer; pursue the best detected target; obey a valid Rogue directive; investigate
  the last detection; otherwise patrol.
- **Optional Rogue Architect tree:** wait for cooldown; prefer a legal play that
  completes or shortens a Guardian route; otherwise isolate an Observer, release a
  Guardian by changing a door state, create repair pressure, or hold its card.

The optional Rogue Architect tree replaces the human through a lab setting and uses
the exact same hand, knowledge, cooldown, legality query, and command application
path. This enables unattended deterministic soak and human-versus-bot comparison;
it is not a stronger director API.

Keep the behavior-tree runner lab-local. The existing hex bot intentionally uses a
small flat behavior choice; do not rewrite it or introduce a workspace-wide AI
framework merely to start this lab. Promote a shared tree representation only after
at least two lab roles use the same control nodes and the Rogue Architect bot proves
the command boundary.

Do not extend `mechanic_lab` into this role: its abstract substrate remains useful
for prison and capture experiments, but it explicitly does not run the real WFC
rules. Do not overload `tactics_lab`; it remains the turn-based readability
instrument and a reference for map projection and observation telemetry.

### B. Prove the first-person pressure loop

The horde, the generator, and the kinetic tool are first-person verbs, and
`architect_lab` is a cell-level board that cannot say whether they feel good. These
are two independent questions and may be proven in either order, or at once.

**Economy, at cell level, inside `architect_lab`.** Extend the existing simulation
with abstract Observer verbs: a finite charge pool, a shove that commits an adjacent
minor Guardian to void or to a retracting cell, per-floor power state, and the
disturbance meter with its wave thresholds. This answers whether the budget is a good
decision space, and the deterministic behavior trees already in the lab can soak it
unattended. Bot Observers gain shove, recharge, and generator intents; the optional
Rogue Architect tree gains disturbance pressure and generator plays. No new authority
boundary appears: every intent still resolves through the existing command path.

**Feel, in first person, in a dedicated lab.** Prove the kinetic tool against real
movement, real geometry, and real falls before any of it approaches production. The
question is whether committing a minor Guardian to void with a shove is satisfying,
readable, and fair at the moment of contact. `guardian_ai_lab` and `hazard_lab`
already carry the weeping-angel pursuit and the two-operator machinery precedent,
`equipment_lab` carries deployables, and `lighting_lab` carries the darkness registers
and the relative-luminance audit that the emergency emission rule requires.

Shove resolution is fixed-tick simulation, not authored physics. Impulses,
destinations, and destroyed actors must reproduce from a snapshot and an input; a
kinetic result that depends on physics-engine timing is a defect regardless of how it
looks. Treat `rapier_determinism_lab` as the precedent for why.

Neither proof promotes anything into `observed_match` before step D.

### C. Add the loyal construction loop

Add a human loyal Architect perspective, variable loyal team size, team-scoped
knowledge, ascent construction, prison self-escape and teammate rescue, unsafe falls,
true-void corruption, and loyal victory. A loyal Architect bot may be added only by
using the same command-producing tree boundary proven by the optional Rogue bot.
Allow local role cycling for testing without conflating roles in authoritative state.

### D. Promote pure simulation contracts

Only after the lab is playable, move proven state into `observed_match`. Use stable
domain identities for the equivalents of `PlayerRole`, `FactionId`, `CardId`,
`CardKind`, `ArchitectCommand`, `DoorDeployment`, `TileStability`, `FloorState`, and
`MatchOutcome`. Decks, cooldowns, legality, WFC selection, instability, role changes,
and outcomes belong to pure fixed-step simulation. Presentation consumes snapshots;
it never decides legality or reconstructs hidden state.

### E. LAN vertical slice

Run two teams with one Architect and one Observer each. The dedicated server owns
decks, commands, cooldowns, knowledge filtering, collapse, Guardian capture,
corruption, and outcomes. Bump input, snapshot, replay, and LAN compatibility
versions together so older clients fail explicitly. Prove reconnect and replay before
increasing teams, Observers, floors, or card variety.

### F. Content expansion and human gate

Add districts, special rooms, hazard tiles, Guardian behaviours, and roster scale
only after the small match works. The gate is human: Architects can explain why a
play is legal and worth making; Observers can explain what is protecting a route and
how to rescue a teammate; both sides can read a contradiction before it consumes a
floor; Rogue play remains engaging without hidden rule exceptions.

## 10. Permanent test obligations

- Seed plus ordered Observer and Architect commands must reproduce decks, selected
  WFC variants, contradiction order, Guardian decisions, role changes, snapshots,
  and outcomes exactly.
- Hands remain at five; refill supplies a live card when a live target exists;
  collapsed-district cards recycle; redraw rooms work once per team.
- Every placement path enforces knowledge, local fit, district, rotation,
  observation, occupancy, anchor, prison, collapsed-floor, and cooldown rules.
- A locally valid contradiction is accepted, warns before retracting, repairs
  deterministically, and permanently closes a fully consumed floor.
- Open doors pin; closed doors block and release; loyal rivals may toggle them; a
  rewritten closed threshold consumes its door; anchors remain durable.
- Falls prefer lower surviving structure; only true void corrupts; no floor collapse
  removes or rewrites the prison core.
- Prison maze escape, door rescue, portal rescue, corruption-adjusted summit quorum,
  zero-loyal elimination, loyal victory, Rogue victory, and same-tick capture
  precedence all have focused cases.
- Loyal knowledge never leaks undiscovered structure or actors. Rogue knowledge
  exposes facility truth but not undetected loyal positions.
- Reset and match exit remove every role-, deck-, door-, instability-, prison-, and
  Guardian-owned entity or resource.
- Headless, interactive, replay, and LAN simulation produce the same authoritative
  digest.
- Human and bot Architects consume identical knowledge, hand, cooldown, legality,
  and command paths; replacing one with the other changes no authority boundary.
- Re-running any Observer, Guardian, or Architect behavior tree against the same
  snapshot produces the same selected intent and tree trace.
- Disturbance rises, decays, and releases waves deterministically. Identical command
  sequences reproduce wave size, composition, spawn cells, and order exactly.
- Minor Guardians are never frozen by observation, never accept a Rogue directive,
  never enter the prison core, and are removed with a permanently collapsed floor.
- Major Guardians remain frozen by observation under every power state that still
  permits sight of them.
- Floor power gates recharge, door operation, ascent rooms, pads, and observation
  range. With power off, every critical signal keeps its documented self-lit minimum
  and the unpowered relative-luminance audit passes.
- Kinetic shoves are fixed-tick and reproducible: identical snapshots and inputs
  produce identical impulses, destinations, and destroyed actors, with no dependence
  on physics-engine timing.
- The kinetic tool deals no damage to any actor and has no effect on rival Observers.
- Charge is finite, restored only at a powered station, and never restored by a card
  play alone.
- An emergency requisition refills to exactly five, releases exactly one major
  Guardian on the correct floor, is visible to every faction, and leaves the placement
  cooldown untouched. The redraw room remains free and once per team.
- Reset clears every disturbance, wave, minor Guardian, power, charge, station, and
  requisition-spawned entity or resource alongside the existing roles and decks.
