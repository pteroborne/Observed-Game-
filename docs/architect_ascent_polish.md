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

### Not yet joined

- The rules' own Guardians (the lab's cell-level hunters) are not placed; the physical
  Guardian is the one that catches.
- A team's map knowledge exists twice: the rules' (what the Architect targets) and the
  physical match's (what the in-play map shows). They are fed by different sight models.
- Doors, anchors and torches are rule state and physical state respectively, not one
  thing.
- A match snapshot does not yet carry where each body is or the prison's mazes, so a
  replay or LAN peer of an Ascent match would not see them. That is part of the LAN slice.
- Nothing draws the maze yet: the game does not run an Ascent match.

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
