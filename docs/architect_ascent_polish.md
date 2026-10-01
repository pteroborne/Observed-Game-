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

- [In the maze, having asked the Architect for rescue and been answered](evidence/ascent-prison/prison-maze-1280x800.png)
- [At its way out](evidence/ascent-prison/prison-way-out-1280x800.png)
- [The lobby's gate, from the hall next door](evidence/ascent-prison/prison-lobby-1280x800.png)

Measured at production scale: a tick is about 0.1 ms of rules and a bot Architect's beat
about 3 ms. A Guardian's catch used to cost 35-40 ms, because the new maze's geometry and
colliders (about 13 ms each) were built on the tick of the catch. A team's `n`th maze is now
fixed by the match seed, the team and `n`, so it is carved ahead on a thread of its own -
the first when the prison opens, each next one as soon as the last is used - and the catch
only takes it (`hex_wfc/model/prison/carving.rs`). Measured 2026-09-26 the catch itself is
under a millisecond and its whole tick about 3-6 ms, the higher only when it lands on a bot
Architect's decision beat. The result is the same maze whichever thread carved it and
whenever it finished; a catch before its maze is ready waits for the rest, and one whose
carving failed, or on a target without threads, carves it on the spot.

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
- **A player's body asks too** (`hex_wfc/ask.rs`): T (rebindable as "Ask the Architect")
  or the controller's D-pad left asks. The player does not pick what for - the rules name
  it from where the body is (`AscentSession::ask_for_help`): rescue when jailed, power on a
  dark floor, and otherwise a route on from the cell the body faces if the team has found
  it, or from where it stands. A small panel under the objective says what was asked,
  whether the Architect has answered ("ON IT"), and how long the ask has left, and names
  the key; a refusal shows for four seconds, and an answer is heard.
- **Through an Observer's eyes** (`architect/eyes.rs`): V (the controller's View button,
  or LOOK THROUGH THEIR EYES) steps off the desk into the first active Observer's eyes, Q
  / E (LB / RB) go to the next, and V, Escape or B come back. Nothing is simulated for it:
  the world view follows one body, the runtime's viewed body (`HexWfcRuntime::viewed`,
  normally the local one), and while looking that is the Observer's - the camera rides its
  eye with the gaze easing after the bot's head, the facility streams in around it, the
  light and district are its, and a jailed Observer is seen in its maze. The board, the
  climb and the desk step aside, their cameras resting, and a bar over the view says whose
  eyes these are and how to go back; the desk hears nothing else, and the desk has first
  claim on Escape and East while looking, so neither pauses.
- **Stair cards** (`CardKind::Stair`; three to a district in an Architect's deck on the
  real facility): the one play that builds the way up. Played on a cell the team has
  found and turned to the direction of the climb, it lays the corpus's ramp pair
  (`observed_facility::hex_wfc::authored_ramp`) - a `RampUp` at the foot, entered from the
  walkway behind it, and its `RampHead` on the floor above, which may be unexplored - as
  one play and one cooldown. Both cells must be free as any play's target must, neither
  may be fixed structure, and neither may be open sky - building in the air re-derives the
  whole open-air region, whose edges re-project wherever they are, watched or not. Once
  built, the stair is fixed itself. Every register builds one
  (three authored ramp tiles are scoped to all). At the desk it is a card whose miniature is
  the ramp, an amber ghost of both cells on the board, and a build the Architect believes
  on both floors. A stair laid over a hall with other doorways leaves those doorways
  meeting walls: contradictions, which the rules treat as play and a loyal Architect
  repairs. The desk does not yet say so before the play.
- **A live hand** (`AscentSession::keep_hands_live`): the design's refill rule - a hand
  must keep a card with a legal target in a district its team can play - is now kept once a
  beat: a loyal hand holding no tile for any floor its team stands on draws one in from its
  own deck. Stair cards made the dead hand likelier (two stairs, the other district's tiles
  and a door, with nothing ever played to change it), and a bot holding one could never
  repair a contradiction on its floor.
- **Why the bot Architects hold their cards** (measured 2026-09-26 on a production
  facility): a loyal Architect may only play on ground its team has found, and a solved
  facility is consistent - every doorway meets a doorway - so with nobody disturbing it,
  no play near the team shortens its way up or opens a new route (an unexplored cell with a
  doorway facing the team already meets one). Stairs do not change that: the production
  facility climbs by authored stair towers, so a new ramp's head is exactly as far from the
  summit as its foot. The bot now judges stairs as readily as halls, and plays when it can
  repair or answer; what the design leaves it to answer is the **Rogue**, whose plays make
  the contradictions and the Guardian pressure a loyal Architect exists to meet. Neither the
  game nor LAN seats a Rogue yet, and the Rogue bot judges every play by simulating it on a
  copy of the whole rules - unaffordable at production scale until it is made local, as the
  loyal bot was.
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
- [Through a teammate's eyes](evidence/ascent-architect/architect-eyes-1280x800.png)

### The kinetic tool and floor power

Every body in an Ascent match carries the kinetic Lance (`game/src/hex_wfc/kinetic.rs`) in
its main hand, the torch moving to the off hand. Left click pushes the minor in the
crosshair away along the look, right click pulls it back (`hex_wfc::model::kinetic`); a
shot that lands costs `KINETIC_SHOT_COST` from the Observer's charge, which the rules own,
and a miss is free. What kills a minor is still the facility: it has to go over an edge.

The charge comes back only at a powered recharge station, and a floor's power is its
generator's (design sections 5 and 6), so both now stand in the facility
(`ascent::facility::power`, `game/src/hex_wfc/power.rs`):

- **Generators are sited once, before tick zero, where a body can stand.** A first-person
  cell is fourteen metres across, so a fixture is not a cell but a point on its cell's
  floor (`HexWfcMatch::standing_point`: the centre if a body fits there, else the nearest
  clear, supported spot within four metres). Every floor gets a generator on a cell a body
  can reach from the floor's spine - never a stair or ramp cell, never the prison - in a
  room wherever the floor has one that qualifies. A generator's cell is fixed structure:
  refused to every card as `FixedStructure` and never retracted, as rooms and stairs are.
  The lab's teleport pads are not sited: the first-person match has plates of its own.
- **Stations are played, from the Architect's mixed hand** (design section 6). A
  first-person deck holds four station cards (`CardKind::Station`). One is played on any
  built cell with a standing point - a room's included, never a generator's, a stair's or
  the prison's (`AscentRules::station_points`, refreshed whenever a tile changes) - under
  the same knowledge, sight, occupancy and cooldown rules as any card. A station is
  equipment on its tile, not structure: rewrite or retract the tile and the station goes
  with it. A hand short of a tile for its floor gives up another floor's tile, a door or a
  stair before it gives up a station. A bot Architect, before improving any route, plays a
  station within reach of its team on a floor they stand on that has none. At the desk the
  card's miniature is the cradle itself.
- **Worked in person, within 2.2 m.** Interact at the generator switches the floor's power
  through the same Observer command a lab Observer uses, so the power policy and its
  refusals are the rules'. A body standing at a powered station draws `RECHARGE_PER_BEAT`
  each beat; a dark station gives nothing, and neither does the rest of its cell. The rules
  no longer charge an embodied Observer for standing anywhere in the station's cell.
- **What the game draws.** The generator is a squat hexagonal turbine: bronze rotor rings
  that turn while the floor has power round a core in the powered violet, under a column
  of its light; cut, the rotor stops and the core burns a low collapse red, dark but still
  self-lit, because it is what a dark floor sends you looking for. The station is a cradle
  of three bronze posts round a charge cell in the tool's own push colour, under a violet
  ring; dead, both go the unpowered grey. A floor without power keeps an eighth of its
  practicals' light and its diffusers go out; the district key over the runner stays,
  because darkness costs observation range in the rules and never legibility here.
- **What the player is told.** The prompt at the generator says what interact will do
  (cut or restore the floor's power, and what that costs); at the station it says the tool
  is recharging and fills its bar with the charge, with no keycap, since a station is
  stood at rather than pressed. The equipment line says NO POWER on a dark floor. The power
  going off or on is heard at the generator and said when it is the local body's floor, and
  charge ticks in as the tool fills.

Evidence (`OBSERVED2_CAPTURE_HEX_WFC_POWER=<dir> cargo dev-run -p observed_game`, a
production facility, the local body stood before the spawn floor's generator and then its
station and walked up through the match's own input; the capture advances exactly 1/30 s a
rendered frame, so `<dir>/frames` makes a real-time 30 fps video however slowly it renders,
and `<dir>/stills.txt` names the frame that is each still):

- [The generator, powered](evidence/ascent-power/power-1-generator-1280x800.png)
- [In reach: the prompt to cut it](evidence/ascent-power/power-2-generator-prompt-1280x800.png)
- [Cut: the core burns red, the floor says NO POWER](evidence/ascent-power/power-3-generator-cut-1280x800.png)
- [Restored](evidence/ascent-power/power-4-generator-restored-1280x800.png)
- [The station, the tool nearly empty](evidence/ascent-power/power-5-station-1280x800.png)
- [Recharging](evidence/ascent-power/power-6-station-recharging-1280x800.png)
- [Charged](evidence/ascent-power/power-7-station-charged-1280x800.png)
- [A dead station on a floor without power](evidence/ascent-power/power-8-station-dead-1280x800.png)
- [The whole walkthrough, 16 s](evidence/ascent-power/power-walkthrough-1280x800.mp4)

Over LAN nothing new travels - the generator is a body's interact bit - but the same frames
now step to a different match, so `LAN_PROTOCOL_VERSION` is 7.

Bot bodies now take a physical route to their floor's generator when its power is out,
press interact within reach to restore it, and take a route to a powered station when
their Lance has less charge than one shot. At the station they wait through the recharge
beats until full. Both local and authoritative LAN bots make these choices through the
same body command path, and match tests walk a bot to each fixture and check the result.
An errand counts only if its route stays on the body's floor: the errand is judged by the
floor the body stands on, so a route that climbed away through a stair was dropped the
moment the body arrived on a powered floor, and the bot stalled there
(`HexBotDriver::floor_route_len_to`).

The power capture deploys a station near the body (`AscentRules::stage_station`) if the
team's bot Architect has not yet played one on the spawn floor.

**What a dark floor stops** (design section 5). Its doors freeze: the rules refuse to
operate a door on a floor without power, and the physical panel follows the rules
(`a_floor_without_power_freezes_its_doors`). Its teleport plates are inert: every tick,
before the bodies move, the rules tell the physical match which floors are dark
(`HexWfcMatch::set_dark_floors`), and a plate on a dark floor, or linked to one on a dark
floor, carries nobody until the power comes back. The game draws such a plate as a lone
one, unlit and still, the same reading as a plate with no partner.

**The way up stays open in the dark** (decided 2026-10-01). The design's "ascent rooms are
inert" was written for powered lifts. The real facility climbs by walked stair towers and
stair-card ramps, so a dark floor's stairs stay walkable, and the rules' own routes
(`ArchitectLab::exits`) keep them on a first-person facility so the rules agree with the
bodies; a lab board's ascent rooms still stop on a dark floor. Barring the stairs was
weighed and set aside: parts of a floor reach their own generator only through another
floor, and a body there with its floor dark and its stairs barred could never restore it.
Darkness still costs a floor its doors, plates, recharge and sight range.

### Where a minor can die

A minor is destroyed only by the architecture, and a push is how an Observer hands it
over. `hex_wfc::model::kinetic::edges` measures where that can happen on the facility the
game plays: from every standing point on every walkable, non-stair cell of three
production facilities, twelve level pushes are walked through the colliders a minor's
capsule meets, and every cell the probe calls lethal is replayed as up to three real
shoves. Only a shove that ends in `GuardianLost` counts; the probe alone overcounts
several times, because a railing often stands just past a slab's edge and a fall carries
forward onto roofs a straight-down ray misses.

| | seed 1 | seed 2 | seed 3 |
|---|---|---|---|
| walkable non-stair cells | 956 | 859 | 825 |
| cells a push kills from, lost only out of the facility | 3 (0.3%) | 3 (0.3%) | 7 (0.8%) |
| ...and with `MINOR_BREAKING_DROP` | 101 (10.6%) | 95 (11.1%) | 78 (9.5%) |
| ...of those on floors 5-7 | 94 | 92 | 69 |

Two findings drove the change:

- **The upper floors' open edges never killed.** They hang over the lower wings' roofs,
  thirty metres down, and a minor landed there and lived.
- **A retracted hall above the ground floors is not a pit.** What is under it is the
  ceiling of the cell below, half a metre down: a minor drops onto that roof and can step
  back out. On floors 0-3 a retraction still opens onto nothing, and every sampled push
  through one killed.

So a minor now also breaks on landing from a fall of more than `MINOR_BREAKING_DROP` (5 m,
over half a storey): the design's "off a ledge or unrailed balcony". A stair, a ramp or a
step down never comes near it.

Then the railings came off. Open edges were railed below the top three storeys
(`RAILED_BELOW_LEVEL` was 5); now only the ground floor, the Institutional district, is
railed, and every floor of the Liminal Grid above it stands open. The same measurement:

| | seed 1 | seed 2 | seed 3 |
|---|---|---|---|
| cells a push kills from, railed below floor 6 | 101 (10.6%) | 95 (11.1%) | 78 (9.5%) |
| ...railed on the ground floor only | 295 (30.9%) | 254 (29.6%) | 230 (27.9%) |

The new ground is floors 2-5 (floor 2 alone gained 73, 41 and 46 cells). The price is
paid by bodies as well: an unrailed edge takes a body into true void, which in Ascent is
corruption. In the ten-minute bot soak no bot body fell, and minors walked off edges on
their own a little more often (5 lost where 2 were).

Waves grow with height and minors leave only by falling, so a first-person floor holds at
most `MINORS_PER_FLOOR` (8). A wave that would pass it releases only up to it, and its
disturbance is spent all the same. A lab board, whose shove commits a minor outright, keeps
no ceiling; its soak depends on the pressure. `production_minor_crowd` (ten minutes, two
teams of two, every seat a bot, and bots never shove) filled floors 0 and 1 to the ceiling
on seed 1 - 18 released where the waves asked for 30 - and that crowd won the Rogue the
match in four minutes. Floor 0's waves are one minor each, but came fourteen times.

Evidence (`OBSERVED2_CAPTURE_HEX_WFC_MINORS=<dir> cargo dev-run -p observed_game`): the
capture finds a push that kills on a production facility, the highest floor first and
proven by playing it on a copy of the match (`HexWfcMatch::killing_push`), stages three
minors there and the body a few metres behind the first, and plays the rest through the
match's own input at one tick a frame. On this seed it found the top floor's open edge; a
second minor was staged ahead, and the third found no room in view.

- [Minors closing in](evidence/ascent-minors/minors-1-closing-1280x800.png)
- [The push, charge 90](evidence/ascent-minors/minors-2-pushed-1280x800.png)
- [Over the edge and broken: "Minor sent into the void"](evidence/ascent-minors/minors-3-broken-1280x800.png)
- [The whole push, 3 s](evidence/ascent-minors/minors-push-1280x800.mp4) and
  [at quarter speed](evidence/ascent-minors/minors-push-quarter-speed-1280x800.mp4)

### Doors

A door card has always been playable at the desk; until now the door it deployed was the
rules' alone, and nothing in the world stood in the doorway. It stands there now
(`hex_wfc::model::doors`, `ascent::facility::doors`, `game/src/hex_wfc/doors.rs`):

- **The rules own every door** (design section 4) and hand the whole set to the physical
  match after every step, after the tick's rewrites, so a door whose cell was rewritten or
  retracted is gone in the same tick. A door card deploys a door closed.
- **A closed door is a panel across the doorway**: a collider the size of the corpus's
  doorway (4.5 m wide, 4 m to the lintel) in a collider id range of its own. It stops a
  body, a minor and a kinetic push. A minor does not plan a way through it or catch across
  it; the major Guardian does not step, see or catch across it either, and waits at it as
  it waits at the prison lobby. An open door is its frame alone.
- **A body works a door with interact** within 2.5 m of the doorway's middle, through the
  rules' own Observer command: any loyal Observer, any team's door, and never on a floor
  without power, which freezes its doors. A body at its floor's generator works the
  generator, not a door beside it.
- **Bot bodies open a closed door they walk into**, rather than stand against it.
- **No door between two cells of one room**: they share open floor rather than a doorway,
  and a door there would be a panel standing in the middle of the room. The rules refuse it
  on the real facility as `InvalidThreshold`.
- **What the game draws**: a bronze frame (two posts and a lintel) and a dark shutter that
  rolls down to close and up to open, deployed rolled up so that a door deployed closed
  rolls down into place. A strip along the lintel says the state from either side: the
  exit green open, the collapse red closed, the unpowered grey when its floor is dark and
  the door is frozen. The door is heard where it stands when it is deployed, opened or
  closed, and the prompt at it says what interact will do, or why it will not.

Evidence (`OBSERVED2_CAPTURE_HEX_WFC_DOORS=<dir> cargo dev-run -p observed_game`): a door
deployed closed in front of the body (staged into the rules, `AscentRules::stage_door`),
the body walking up and opening it, a minor waiting beyond it (staged) coming through, the
body pushing it back through the doorway and closing the door on it.

- [Deployed, rolled down](evidence/ascent-doors/doors-1-deployed-1280x800.png)
- [At the door: "Open the door"](evidence/ascent-doors/doors-2-prompt-1280x800.png)
- [Opened, the minor coming through](evidence/ascent-doors/doors-3-opened-1280x800.png)
- [Pushed back through](evidence/ascent-doors/doors-4-pushed-1280x800.png)
- [Closed on it again](evidence/ascent-doors/doors-5-closed-1280x800.png)
- [The whole scene, 5 s](evidence/ascent-doors/doors-1280x800.mp4) and
  [at half speed](evidence/ascent-doors/doors-half-speed-1280x800.mp4)

Sight: the rules' cell sight already stopped at a closed door, and the physical match's
Guardian now does too - a body does not freeze the major Guardian, or get caught by it,
across a closed door. Not yet: a door shut on a minor only separates it; the design's
"through a threshold an Observer then closes" still needs the minor to go over something.
Anchors and torches are still rule state and physical state apart.

### The Rogue board

A body that falls into true void has corrupted (design section 7): the rules already made
its seat a Rogue seat. Its player had nowhere to go; now they take a seat at the Rogue board
(`ascent::join_rogue_board`, `ArchitectDesk::rogue`), the same desk a team's Architect
uses, reading the Rogue's side of the rules:

- **The Rogue's own deck** (`Deck::rogue`, design section 7): tiles and doors, four
  Guardian directive cards and four sensor cards, and no stairs - the Rogue builds no way
  up. The directive and sensor cards are played like any card - picked up, aimed,
  confirmed - start the seat's cooldown, and build and disturb nothing. A team's desk is
  refused one (`CommandRefusal::RogueOnly`); a team's deck never deals one.
- **A Rogue hand of their own** (`AscentSession::rogue_hands`, `ArchitectDesk::hand`): a
  player who joins the Rogue is dealt a Rogue deck and a cooldown of their own (a user
  decision, 2026-09-28: the design's one shared hand is amended). The rules' own
  Rogue hand stays the seated operators' - the bot Rogue's - so a player at the board
  never waits on the bot's clock, nor loses the card under the cursor to its play. Each
  corruption adds an operator to the Rogue, as the design's asymmetric faction change
  reads. Once a beat, the seated Rogue and every joined player whose hand has no tile
  for a district with a mutable target draw one in from their own deck. Losing one
  upper floor does not retire Liminal Grid tiles while another upper floor remains.
- **The facility's truth** (`ArchitectLab::rogue_view`): every cell as it stands, every
  Guardian, and of the loyal Observers only those the Rogue may know - jailed, corrupted,
  or detected by a Guardian (design section 10). The board, the climb and the side panel
  all read it through `ArchitectDesk::knowledge` and `ArchitectDesk::shows`, so nothing
  there reads a team's memory.
- **Its own seat**: a play goes out from the player's own seat, which the rules now know as
  a Rogue seat. Over LAN that needs nothing new: a body's seat commands already map to its
  own seat, and a corrupted peer takes the board the same way.
- **What a team has and the Rogue does not** is taken out: nobody asks the Rogue for help,
  it looks through nobody's eyes, and a requisition is a team's. The side panel is headed
  DETECTED (by a Guardian or a sensor), the title says ROGUE AI, and the control strip names neither
  answering nor requisitioning.
- **Directives** (`CardKind::Directive`, `ascent::sim::directive`): a directive card sends
  the major Guardians to the cell it is played on. It is refused on void, a collapsed
  floor or the prison. The rules keep it (`ArchitectLab::directed`) until a major Guardian stands on
  the cell or `DIRECTIVE_TICKS` (30 s) pass; the facility hands it to the bodies each tick
  (`HexWfcMatch::direct_guardians`), and a directed major walks there instead of hunting
  the leading body - unless a body shares its cell, which it catches first, or the cell
  cannot be reached, where it hunts as ever. Minors never take one (design section 4).
  The Rogue's board shows it as a breathing red beacon and a line with the time left;
  a team's board is not told.
- **The result**: a corrupted player plays for the Rogue, and wins with it
  (`ascent::result_for`).

Evidence (`OBSERVED2_CAPTURE_HEX_WFC_ROGUE=<dir> cargo dev-run -p observed_game`: the local
body walks until it is dropped into true void - staged, `HexWfcMatch::drop_into_void` - and
the rest is the Architect capture's, from the Rogue board):

- [The Rogue board](evidence/ascent-rogue/rogue-board-1280x800.png)
- [A card picked up](evidence/ascent-rogue/rogue-play-1280x800.png)
- [The play building in](evidence/ascent-rogue/rogue-building-in-1280x800.png)
- [Built, the Rogue's hand recharging](evidence/ascent-rogue/rogue-built-1280x800.png)
- [Guardians directed](evidence/ascent-rogue/rogue-directed-1280x800.png): the beacon,
  and the directive with its time
- [Seven seconds on](evidence/ascent-rogue/rogue-directed-walked-1280x800.png): the
  Guardian two cells from the beacon, walking in

Over LAN a directive or sensor card travels as any card play does (protocol 14 dropped
the separate commands of 11 and 12).

**The bot Rogue plays its orders too** (`ascent::sim::rogue`): after closing the hunt with
a tile, it sends the Guardians - a directive card on the detected Observer nearest a
major Guardian on its floor - before undermining; and with nobody detected it watches the
way up with a sensor card (below). It is markedly stronger for it: on the two-floor test
facility with two bot bodies it now wins at tick 1616 (27 s), where before it had not
won in 3,000 ticks.

**Instability surge** (`CardKind::Surge`, `ascent::sim::instability`): three cards in
the seeded Rogue deck raise disturbance by 50 on the chosen floor. When an exposed
contradiction on that floor is already warning, the surge shortens its pending
retraction to at most 90 ticks from the play. It cannot be played on observed,
occupied, anchored, void, collapsed, or prison cells, and it never retracts a
protected tile directly. The warning event identifies where the surge happened;
the existing disturbance wave and collapse systems carry the result into the
facility. The board shows the current floor's pressure and any exposed
retraction countdown. The bot Rogue uses a surge near a detected Observer when neither a
route-closing play nor a fresh contradiction is available.

### Sensors

Design section 7: the Rogue sees a loyal Observer "only while that Observer is detected by
a Guardian or an explicit Rogue-controlled sensor". The sensor is now in the rules
(`ascent::sim::sensor`), in the facility, and at the board:

- **Installed by a sensor card** (`CardKind::Sensor`) from a Rogue hand, played on a cell
  like any card. Like any Rogue play it cannot be made where an Observer is looking
  (`Observed`), nor on void, a collapsed floor, the prison, or a cell that already has
  one.
- **What it sees**: its own cell and `SENSOR_RANGE` (4) cells along each open lateral line
  from it - a wall or a closed door ends the line, as it ends a Guardian's six. It sees
  only while its floor has power: a dark floor blinds its sensors, which gives cutting a
  generator a second edge.
- **What that is for**: an Observer a live sensor sees joins the Rogue's knowledge
  (`ArchitectLab::rogue_detected`), so the Rogue board shows it and the bot Rogue hunts
  it. It never feeds the Guardians' own pursuit: sensors inform the Rogue, the Rogue
  directs. When nobody is detected, the bot's beat refill offers a sensor if its
  seeded hand lacks one, keeping its required district tile in hand.
- **At most four** (`MAX_SENSORS`): a fifth retires the oldest. A sensor goes with its
  cell when the cell is emptied, retracted or its floor collapses.
- **Counterplay** (`ObserverAction::Dismantle`, `AscentRules::operate_sensors`): a sensor
  hangs 2.6 m over the floor at its cell's middle (`HexWfcMatch::set_sensors`, found once
  against the colliders, again when the facility is rebuilt), and a loyal body under it
  takes it down with interact, after a generator and a door in the interact order. The
  prompt says so.
- **In first person** (`game/src/hex_wfc/sensors.rs`): a polyhedral eye in the director's
  colour, turning inside a ring, with a small light of its own. It burns brighter, turns
  faster and its ring breathes while it sees someone, and it is the unpowered grey,
  unlit, while blind. It is heard where it hangs when installed and when taken down.
- **At the Rogue board**: each sensor on the floor in view is a red eye - the Rogue's,
  against the Observers' cyan - over its cell, breathing while it sees someone, grey while
  blind, and a thin red ring on every cell a live one watches. The orders line counts them
  ("Sensors 2 / 4, 1 seeing someone."). A team's board is not told.
- **The bot Rogue watches the way up**: with nobody detected it has nobody to play
  against, so it plays a sensor card at the foot of a stair or ramp no sensor watches yet,
  on the floor it watches least - every climb passes one.

Over LAN a sensor card is a card play and a body takes one down with its ordinary
interact.

Evidence (`OBSERVED2_CAPTURE_HEX_WFC_SENSORS=<dir> cargo dev-run -p observed_game`: a sensor
staged one open step from a cell near the body, which is stood a few metres off facing
it; the walk and the interact are the match's own):

- [Watching the body](evidence/ascent-sensors/sensors-1-watching-1280x800.png)
- [Under it, the prompt](evidence/ascent-sensors/sensors-2-prompt-1280x800.png)
- [Taken down](evidence/ascent-sensors/sensors-3-taken-down-1280x800.png)
- [The walk up and the take-down](evidence/ascent-sensors/sensors.mp4)
- [At the Rogue board](evidence/ascent-rogue/rogue-sensor-1280x800.png): the sensor just
  installed through the desk, the bot Rogue's, and what each watches

Not yet: a sensor seen from across the facility is not on a team's map; the Rogue cannot
move one, only install another.

### Not yet joined

- The rules' own Guardians (the lab's cell-level hunters) are not placed; the physical
  Guardian is the one that catches.
- A team's map knowledge exists twice: the rules' (what the Architect targets) and the
  physical match's (what the in-play map shows). They are fed by different sight models.
- Anchors are one thing now: a lantern anchoring a room's doorway anchors, in the rules,
  both cells either side of it (`HexWfcMatch::anchored_cells`), so no card rewrites or
  retracts them. Before, the rules' anchor set stayed empty on the real facility, and a
  card could rewrite the corridor a lantern was holding. Doors are one thing too
  ([Doors](#doors)).
- Ascent plays over LAN, a human at a team's Architect desk where one claims it in the
  lobby and a bot everywhere else; plays, requisitions, asks and answers travel as seat
  commands in the frames (`docs/lan_integration.md`). A match
  snapshot still does not carry where each body is or the prison's mazes, so a late
  joiner replays history from tick one, which rebuilds both.
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
