# Architect Ascent integration

**Partial implementation. The main Play runtime remains `HexWfcMatch`; it does
not yet run the shared Ascent session.** The session now runs headless over the real
first-person facility (`observed_match::ascent::facility`); see
[The rules on the real facility](#the-rules-on-the-real-facility).

## Baseline

Branch `codex/architect-ascent-polish` starts from main `0d9eb7ea`, merges
`asymmetry-lab` `02739be8`, and snapshots its uncommitted architect-hand prototype.
The original checkouts were left untouched. The inherited equipment, window,
moonlight, and open-air work is included.

## Implemented

- Promoted the pure Architect lab rules into `observed_match::ascent`; desktop
  and browser lab views re-export those rules rather than maintaining a fork.
- Added a versioned, fixed-tick seat boundary with role authorization, isolated
  loyal Architect hands, shared Rogue authority, corruption transitions, and
  neutral human Observer input that never silently invokes a bot.
- Added read-only card inspection that checks the acting seat's knowledge, hand,
  and cooldown using the same eligibility rules as commit.
- Team knowledge stores last-seen geometry with explicit freshness. A rival's
  observation no longer updates another team's map. Seat snapshots allowlist
  visible information and omit Guardian targeting internals.
- Added bounded, expiring team requests and team-only acknowledgment. These are
  simulation commands; no main-game request UI or network encoding is wired yet.
- Added main-game contextual interaction prompts for implemented nearby actions,
  solo-aware station guidance, synchronization progress, objective/equipment
  readouts, and transient confirmations. Map, pause, onboarding, and spectator
  modes hide this HUD.
- Fixed the main-game adapter dropping the controller's one-shot interact action.
  Keyboard rebinding is reflected in prompt labels; controller actions have text
  equivalents alongside keyboard labels.
- Added persisted gameplay text scaling (90–125%) and reduced hand motion. The
  latter removes hand sway/spin, not world animation: `Spin` parts carry whether they
  are held, so a keystone or a plate on the floor keeps turning.
- Polished the in-play HUD into the menus' language (`game/src/hex_wfc/hud/play.rs`).
  The objective panel sits top left, with the team's colour as its accent bar and the
  keystones as pips. Equipment sits top right, out from over the held plate. The prompt
  sits low centre, between and above the hands: keycaps for the key and the controller
  button, the action, its detail, and a filling bar for a held action. Notices fade in
  and out at top centre, amber when something went against you. Panels size to their
  content, so text scaling wraps inside them.
- Replaced the objectives' placeholder shapes (`game/src/hex_wfc/objective_models.rs`)
  in the equipment's hexagonal language. A keystone is a crystal turning in a brass
  halo over a lit plinth. A console is a dark plinth with a lit screen. A station also
  has a column of light that fills with the team's sync progress and holds full once
  synchronized. The exit is a green column over a lit ring.

## Visual evidence

The HUD capture enters through the ordinary asynchronous launch handoff and uses
normal fixed-tick input for interaction and station holding. Staging teleports
only the evidence runner between authored mechanisms.

```bash
OBSERVED2_HEX_FACILITY=28x20x10 \
OBSERVED2_CAPTURE_HEX_WFC_HUD=docs/evidence/ascent-polish \
cargo dev-run -p observed_game
```

This larger fixture requests the objective quotas; compact fixtures may omit
objectives and cannot verify station progress. The production `arc_default`
currently uses 24x17x8, below the older quota activation threshold of 28x20x10;
this work does not silently change its objective-generation policy.

- [Keystone prompt, 1280x800](evidence/ascent-polish/interaction-1280x800.png)
- [Station, 125% text, 1280x800](evidence/ascent-polish/station-large-text-1280x800.png)

## The rules on the real facility

`AscentMatch` joins the two simulations without a second world. The first-person
`HexWfcMatch` owns bodies, physics and geometry; the Ascent session owns cards,
cooldowns, contradictions, retraction, power, sight and outcomes. Each tick:

1. The bodies move (`HexWfcMatch::step`).
2. Every Observer is put where its body is: its cell, and the lateral face it is turned
   toward. The rules never step, fall or bot-drive an embodied Observer.
3. The seats' commands apply and the rules tick (`AscentSession::advance`).
4. Everything the rules rewrote that tick, card plays and retractions alike, is committed
   to the physical match as one directed change: the same logical, geometry and collider
   deltas a relayout produces, so presentation cannot tell them apart.

The rules are the only writer. They keep the facility they reason about and the physical
match keeps the one it builds, and every rewrite goes through one choke point
(`ArchitectLab::rewrite`) that reaches both. The tests assert the two hold identical
placements after every tick, and that the physical geometry and colliders are exactly
what a fresh projection of the facility would build.

The director's scheduled relayout is switched off in such a match
(`HexWfcMatch::hand_mutation_to_architects`): the facility changes because an Architect
played a card or a contradiction retracted, and for no other reason.

A directed commit (`HexWfcWorld::commit_directed_delta`) differs from a relayout commit in
one way, on purpose: it does not refuse a boundary that does not match or a route that
breaks, because a locally valid contradiction is play (design section 3).

### Decisions this made

- **Sight is still the rules' cell sight.** The physical match has no field of view: its
  observation frame is where players stand, which room door they face, and their torches.
  So an embodied Observer sees and wards by the rules' existing model, four cells along the
  way its body faces and one warded step ahead, now driven by the real body. Real
  field-of-view sampling is its own piece of work.
- **What would redraw is warded too.** In a built facility a hall's open edges and a room's
  windows are drawn from its neighbours, so retracting the cell beside a watched room would
  open a window in front of whoever is in it. Those neighbours are warded. A lab board draws
  nothing and is unchanged.
- **Rooms and stairs are built whole.** A stamped room, and any cell a stair or ramp links
  vertically, is refused as `FixedStructure` and never retracted. One cell of a room cannot
  be rewritten alone.
- **No dead ends are dealt.** The authored corpus has no one-door flat hall, so a
  first-person deck is corridors, turns, junctions and halls (`TileShape::AUTHORED`), and
  every tile play is one the corpus is required to build (`authored_hall`, tested against
  `geometry_demands`). The lab's deck is unchanged.
- **Districts stay the rules' two.** A card's district is still level 0 Institutional and
  above LiminalGrid; a rewritten cell keeps its facility's architecture register, which
  the coverage gate guarantees every tile in.

### The prison

The prison (design section 3, amended 2026-09-25) is physical, and the rules learn it
from the bodies the way they learn positions:

- `HexWfcMatch::send_catches_to_prison` gives a match a prison from tick zero. The lobby
  is the ground-floor tile nearest the facility's centre that a body can walk to from the
  spawn, or the whole room if that tile is part of one. A tile rather than a room: the
  solver never puts a room at the ground floor's centre (at production scale the ground
  floor holds the start room at the spawn corner and one other), so "the room nearest the
  centre" was a corner. Dressing that tile as the prison's exit is presentation work.
- Every body has a `HexBodyPlace`: the facility, the prison, or the void. Everything that
  sees, is seen, collects, deploys or is hunted asks `HexPlayerState::in_facility`.
- A Guardian catch puts the body in its team's maze (`hex_wfc::model::prison`), a world
  of its own with its own geometry and colliders, stepped against its own physics scene.
  The maze is carved by `observed_facility::hex_wfc::maze::braided_maze`: the facility's
  solver makes a single corridor on one level, and the corpus has no one-door hall to end
  a dead end with, so the maze is a spanning tree whose every dead end is joined into a
  neighbour. Every hall is an authored straight, turn or junction, and a draw is kept only
  if its shortest way out is sixteen to twenty-four halls with a junction for every six
  cells. The game's bot walks one out in about thirty seconds.
- Reaching the maze's far corner releases the body into the lobby. A teammate standing
  in the lobby for `LOBBY_HOLD_TICKS` (three seconds) releases every jailed teammate;
  leaving resets the hold.
- In a prison match the Guardian hunts a lone runner too (the prison is how the Rogue
  wins), and never steps into the lobby.
- A body that falls below the facility is lost to the void and does not come back. A
  body stranded on a roof is still recovered: that is a fall onto lower structure.
- The rules see a jailed body as jailed at the lobby, and a lost one as corrupted, whose
  seat becomes a Rogue seat. All loyal Observers jailed is the Rogue's win.

### Playing it in the game

Play has a **Rules** row: *Facility race* or *Architect Ascent*. It is kept apart from
the roster preset and survives changing it. Ascent is local only for now; LAN still
plays the race.

- The runtime keeps its `HexWfcMatch` where every presentation system reads it and holds
  the rules beside it (`AscentRules`, the rules without the match inside them). Each tick
  steps both together; the rules' outcome ends the match and decides the result.
- Every team's Architect seat is held by the loyal bot (`ascent/sim/loyal.rs`): repair
  mismatched doorways, otherwise shorten the team's way to the summit without breaking
  anything, otherwise hold. It asks the human legality query and submits through the human
  path, in its team's hand context. It judges a play without making it (one search from
  the summit, a bounded one from each Observer, and the doorways round the played cell),
  which took a production beat from 388 ms to about 3 ms. A healthy facility gives it
  nothing to do; it answers damage.
- A jailed body is drawn in its team's maze, 1 km below the facility and 2 km from the
  next team's, with the camera and hands. The maze is spawned whole when the local body
  arrives. The maze is a sealed world (`HexWfcWorld::sealed`): no wall of it comes down
  onto the rock around it and no railing stands on its edge, so it is corridors, not the
  facility's walkways under a sky. Every facility stays unsealed.
- The prison has a gate, in the equipment's language: a cage of six bronze bars (the
  Guardian's trim) between a lit ring and a cap, lit with the new `MarkerRole::Prison`, a
  pale cold light apart from every saturated hue. It stands on the lobby tile, where a
  column of that light fills with the local team's hold, and in the maze's way-out hall.
- The objective panel says what the prison asks: *Find the way out of the maze* when
  jailed, *Hold the prison lobby to free a teammate* when one is, and *Climb to the
  summit* otherwise, in place of the race's keystones.

Evidence (`OBSERVED2_CAPTURE_HEX_WFC_PRISON=<dir> cargo dev-run -p observed_game`, a
production facility, the local body jailed and walked out by the game's own bot):

- [In the maze](evidence/ascent-prison/prison-maze-1280x800.png)
- [At its way out](evidence/ascent-prison/prison-way-out-1280x800.png)
- [The lobby's gate, from the hall next door](evidence/ascent-prison/prison-lobby-1280x800.png)

Measured at production scale: a tick is about 0.1 ms of rules, a bot Architect's beat
about 3 ms, and a Guardian's catch 35-40 ms, which is the new maze's geometry (17 ms) and
colliders (17 ms) built on the tick of the catch. Spreading that build over several ticks,
as a relayout is spread, is the fix when it matters.

### The Architect's seat

Play's Rules row now cycles *Facility race*, *Architect Ascent, as an Observer* and
*Architect Ascent, as the Architect*. As the Architect the local player has no body: the
team's bodies are bots, and the player sits at the desk (`game/src/hex_wfc/architect/`).

- **The board** is one floor of what the team knows, from above, drawn by its own camera
  over the world. It frames what is known rather than the whole lattice, and glides as the
  team maps more. A known cell is a slab in its district's colour, lit where the team is
  looking now and dim where it is only remembered; every doorway is a white link. On it:
  the team's Observers, the Guardians they can see, contradictions, the prison lobby, the
  summit once found, and deployed doors - a bar across the doorway when closed, two posts
  when open, so the state reads by shape first.
- **The hand** is five cards along the bottom. Each card's glyph is a hub with a spoke for
  each doorway the tile would have, turned to the rotation it would be played at.
- **Playing:** pick a card (1-5, or click it), turn it (Q / E), point at a cell. Every cell
  the rules would take the card on wears a ring, and the cell under the cursor wears the
  tile's ghost, green or red. The side panel says why a cell is refused, in the rules' own
  words. A click sends the play only if the rules' inspection would take it; the step hands
  it to the rules as the seat's command, and a refusal comes back to the desk. `[` / `]`
  change floor, R is the emergency requisition, right click puts the card down.
- Legality is never decided at the desk: every ring and verdict is
  `AscentSession::architect_refusal` for the player's own seat, the question the play asks.

Evidence (`OBSERVED2_CAPTURE_HEX_WFC_ARCHITECT=<dir> cargo dev-run -p observed_game`, a
production facility after twenty seconds of the team's bots walking):

- [The board](evidence/ascent-architect/architect-board-1280x800.png)
- [A card picked up and pointed](evidence/ascent-architect/architect-play-1280x800.png)
- [The play built, a door across its doorway, the hand recharging](evidence/ascent-architect/architect-built-1280x800.png)

### Not yet joined

- The rules' own Guardians (the lab's cell-level hunters) are not placed; the physical
  Guardian is the one that catches.
- A team's map knowledge exists twice: the rules' (what the Architect targets) and the
  physical match's (what the in-play map shows). They are fed by different sight models.
- Doors, anchors and torches are rule state and physical state respectively, not one
  thing.
- A match snapshot does not yet carry where each body is or the prison's mazes, so a
  replay or LAN peer of an Ascent match would not see them. That is part of the LAN slice.
- The Architect's desk has no controller navigation yet, no team requests, and cannot
  look through an Observer's eyes.
- A replay tape of an Ascent match samples jailed bodies at their maze coordinates.

## Remaining integration

1. ~~Reserve and generate a physically connected prison core.~~ Done differently: the
   design moved the maze out of the facility ([The prison](#the-prison)).
2. Replace the rules' cell sight with real field of view for embodied Observers. Bodies,
   card plays, the prison, falls and corruption are connected
   ([above](#the-rules-on-the-real-facility)); do not run a second race simulation beside
   the rules or convert cell steps into teleporting FPS movement.
3. Add the dedicated Architect role, mixed ascent/station hand, map placement
   UX, team request UI, power/tool HUD, role transitions, summit/results, and
   controller navigation in the main game. Architect bot seats are not driven
   by the new session yet; existing lab Rogue and Observer bots remain intact.
4. Add authoritative LAN encoding, authenticated seat routing, content identity,
   replay/resync support, and local/remote parity tests for the Ascent session.
   The versioned in-memory input struct is not a transport codec.
5. Prove full playable local and LAN matches, including physical prison rescue,
   lower-floor landings and true-void corruption, before replacing main Play.

The shared-rule extraction and polished current-loop HUD are independently
reviewable foundations, not completion of the full approved plan.
