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

- **The board** is the building itself, as the team remembers it, at the isometric
  pitch `architect_lab` and the survivor map use, drawn by its own camera over the world
  and lit like the lab (its key light, and its ambient on the camera, whatever the
  facility's own light is doing). `architect/building.rs` draws each known room from its
  real authored hulls, cut away the lab's way: ceilings dropped, the walls nearest the
  camera removed and the rest capped low, cut wall tops dark, concrete in the lab's
  district palette. **As remembered, never as it is:** a room the team saw exactly as it
  stands is drawn from the live geometry, and one that has changed since is projected
  from the placement the team remembers, so the board leaks nothing the team has not
  seen. The one thing it adds is the Architect's own work: a tile the rules took from this
  desk is drawn as built (`ArchitectDesk::built`, `believed`) until the team has seen that
  cell since, when what it saw takes over - the Architect knows what they built, and a
  rival may have rebuilt it. Rooms in view are lit; rooms only remembered are dimmed; the two floors below the
  one in view stand under it as flat context silhouettes. The board frames what is known
  of the floor in view and glides as the team maps more or the floor changes.

  The whole board scene is built 20 km from the facility (`pick::BOARD_ORIGIN`): the
  world's lamps and the Observers' torches light whatever is near them whichever layer it
  is on, and at the building's true position they blew the board out.

  On it, in the lab's unlit signal colours: the team's Observers as cyan eyes, the
  Guardians they can see as red pyramids, contradictions as red rings, the prison lobby
  violet, the summit green once found, deployed doors (a bar across the doorway closed,
  two posts open), a chevron on a stair or ramp (green up, muted down), and a dark plate
  on a known cell with nothing built.
- **The climb** (`architect/stack.rs`) is every floor at once, in the panel: the board is
  one floor with only the two below it as context, because a play needs a cell under the
  cursor with nothing in front of it, and what that costs is the rest of the climb. The stack gives it back - the
  same reading the survivor map gives an Observer, from the same knowledge the board
  draws. Each floor is a plate of the whole lattice with the team's known cells standing
  on it, pulled apart far past a storey and seen head-on rather than corner-on, so no
  floor hides another; the floor in view is lit and numbered, and the team's Observers,
  contradictions, the summit and the prison lobby stand up as pins. A click on a floor
  puts the board on it. On the board itself, a stair or ramp cell carries a chevron: green
  up, the way to the summit, and dim down.
- **The hand** is five cards along the bottom, and each card is a miniature of the real
  tile it will build (`architect/cards.rs`), as the lab draws its hand: every card has its
  own camera rendering into an image on the card, at the board's pitch and under the
  board's key light, showing the authored tile for the card's district cut away the same
  way as the board's rooms. A bar lies across each doorway, cyan in hand and amber on the
  card picked up, which turns with the desk's rotation - at card size the doorways read by
  their thresholds, not by gaps in cut walls. A door card shows a door frame.
- **The desk** is laid out as the lab lays out its own, in the lab's palette: a top bar
  (the seat and team, whether the hand is charged, and a floor switcher), a slim side
  panel (the team's Observers, the climb, the key), the hand along the bottom with the
  controls in a line beneath it, and - while a card is picked up - a card panel at the
  right: the tile large, the cell it is aimed at and its orientation, the rules' verdict,
  TURN buttons, PLAY CARD [SPACE] (amber only when the rules would take the play) and
  CANCEL [ESC]. The board frames itself inside whatever the desk leaves free, and slides
  aside as the card panel opens.
- **Playing:** pick a card (1-5, or click it), turn it (Q / E), point at a cell. Every cell
  the rules would take the card on wears a green ring. The cell the play is about wears an
  amber ring if the rules would take it and a red one if not, and a tile card shows the
  lab's amber ghost of the actual tile - its real hulls, projected for that cell at that
  rotation - standing where it would be built. Picking is exact at the angle: the
  cursor's ray is traced onto the deck of the floor in view (`pick::ray`,
  `pick::on_deck`). A click **aims** the card at a cell, where its ghost stays while the
  cursor moves on; a second click on the aim, Space, Enter or PLAY **confirms** it, and
  Esc steps back from the aim and then from the card. The card panel says why a cell is
  refused, in the rules' own words. A confirmed play is sent only if the rules'
  inspection would take it; the step hands it to the rules as the seat's command, and a
  refusal comes back to the desk. `[` / `]` or the top bar change floor, R is the
  emergency requisition, right click puts the card down.
- **Feedback** (`architect/feedback.rs`): a tile the rules take from this desk builds in -
  the room rises the last metres into place under an amber glow that fades, with the
  facility's reroute sound - and a remembered room the team finds changed builds in under
  a cyan glow. Only a changed placement builds in: a room drawn again because the team now
  sees it, or no longer does, stays put. A contradiction's red ring breathes, and so does
  the aim's ring while the play waits to be confirmed. A card picked up clicks, an aim
  ticks, and a refused play lands with a dull knock.
- **On a controller** (`architect/pad.rs`): the left stick moves a cursor across the floor
  in view in the board's own screen directions, and the cell under it is the one pointed
  at. A aims and A on the aim confirms, B steps back from the aim and then from the card,
  LB / RB turn the card, the D-pad goes along the hand (left / right) and changes floor
  (up / down), and R3 is the emergency requisition. The prompts - the card panel's
  buttons and the line under the hand - name whichever the last hand on the desk used.
- **The desk and the match's hotkeys** (`hex_wfc::input::hotkeys_beside_desk`): while the
  desk holds a card, Escape and East step back at the desk and neither pause nor go back
  in the match; with nothing in hand Escape pauses as ever, and Start always pauses. The
  survivor map never opens at the desk - the board is the Architect's map, and RB turns
  cards. While a pause page is up the desk hears nothing.
- **Team requests** (`architect/requests.rs`, `ascent::session::requests`): a body a bot
  drives asks its Architect for help when it is in trouble it can name - rescue when
  jailed (at the prison lobby, which every team knows), power on a dark floor, a route
  when it has stood on one cell for eight beats - and withdraws the ask when the trouble
  passes. It judges only what a body standing there would know, never the rules' map. The
  rules seat embodied bodies as human, so the game names the ones a bot drives
  (`AscentRules::voice`): every body at the Architect's desk, every other one when the
  player is a body. At the desk each request is a beacon over its cell in its kind's
  colour (cyan route, yellow power, violet rescue), breathing until answered, a tall pin
  on its floor of the climb, and a line in the side panel with the time it has left; a
  new one calls, low and long. F (Y on a controller, or ANSWER) answers the oldest
  unanswered one: the rules acknowledge it to the team as the Architect's seat, and the
  board goes to its floor. A bot Architect acknowledges its team's requests on its beat
  and, after repairs, builds within reach of the oldest route it was asked for.
- Legality is never decided at the desk: every ring and verdict is
  `AscentSession::architect_refusal` for the player's own seat, the question the play asks.

Evidence (`OBSERVED2_CAPTURE_HEX_WFC_ARCHITECT=<dir> cargo dev-run -p observed_game`, a
production facility after twenty seconds of the team's bots walking):

- [The board](evidence/ascent-architect/architect-board-1280x800.png)
- [A card picked up and pointed](evidence/ascent-architect/architect-play-1280x800.png)
- [The play building in under its amber glow](evidence/ascent-architect/architect-building-in-1280x800.png)
- [The play built, the hand recharging](evidence/ascent-architect/architect-built-1280x800.png)
- [A teammate jailed, asking for rescue at the lobby](evidence/ascent-architect/architect-request-1280x800.png)
- [The request answered](evidence/ascent-architect/architect-answered-1280x800.png)

### Not yet joined

- The rules' own Guardians (the lab's cell-level hunters) are not placed; the physical
  Guardian is the one that catches.
- A team's map knowledge exists twice: the rules' (what the Architect targets) and the
  physical match's (what the in-play map shows). They are fed by different sight models.
- Doors, anchors and torches are rule state and physical state respectively, not one
  thing.
- A match snapshot does not yet carry where each body is or the prison's mazes, so a
  replay or LAN peer of an Ascent match would not see them. That is part of the LAN slice.
- The Architect's desk cannot look through an Observer's eyes, and a human Observer has
  no way yet to ask their Architect for help.
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
