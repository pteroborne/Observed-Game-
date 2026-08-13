# tactics_lab

A turn-based, whole-board variant of the Observed facility. A squad of units
moves cell to cell across a real solved hex facility, spending action points;
when the turn ends, unobserved structure re-collapses through the same
observation-safe relayout the shipped game uses.

## The technical question

*Can the observe-to-freeze loop be read and reasoned about by a player, if you
give them the whole board and a turn to think?*

The shipped game answers the first half of this well and the second half badly.
Arc T's playtest reported that choices felt shallow, that the facility's changes
were imperceptible, and that the map could not orient anyone — three symptoms of
the same thing, which is that a first-person player at 60 Hz never sees the
system they are supposed to be playing against.

This lab removes the camera problem so the rules can be judged on their own.

## Running it

```powershell
cargo dev-run -p tactics_lab
```

The lab opens on a **match setup screen**. Pick Guided, Scout, Standard, or
Collapse, then adjust it: ranges with three or more choices are discrete
sliders, while two-state rules are ordinary toggle buttons. `R` returns to
setup at any time.

**Guided** is the teaching mode: one 80-tile floor, the complete facility and
exit visible from turn one, six AP, slow telegraphed shifts, no Guardian, and
both anchors and teleport plates available. `Tiles per floor` and `Floors per
map` are independent rows, so difficulty can be increased along either axis.
`Observation radius` is an explicit 0/1/2-tile slider; Standard starts at zero
so occupied ground is held without freezing a large halo. `See full map`
reveals the drawing and exit marker but does not increase that radius.

| Input | Does |
| --- | --- |
| hover a cell | preview the route, AP cost and this turn's stopping cell |
| click a cell | commit the previewed route; the unit advances one visible step at a time |
| click a unit | select it |
| `Tab` / Next unit | select the next unit that can still act |
| `1`–`6` | select a unit directly |
| `Space` / End turn | end the turn: rivals move, the Guardian hunts, the facility shifts |
| `V` / Deck / map | switch between the authored one-third-wall deck and active-deck operations map |
| `[` `]` / Deck - / Deck + | browse decks in either view |
| wheel or `-` `+` / Zoom | zoom under the map cursor; dock buttons zoom around centre |
| right/middle-drag or arrows / Pan | pan only when the gesture starts inside the map viewport |
| `Home` / Recenter | restore the current view's fitted framing |
| `B` / Bot run / pause | let the deterministic objective bot drive the squad, or return control |
| `.` / Bot step | pause and advance exactly one bot decision for debugging |
| `Esc` | pause, resume or open the help/legend panel |

Every command has an on-screen control as well as a key. That is deliberate: the
prototype is pointed at a possible touch build, and a lab that can only be driven
from a keyboard would not survive the move.

The map and command dock have exclusive pointer ownership. Scrolling or dragging
over the dock never moves the board; on a short display the fixed dock scrolls
internally instead of reflowing its controls. Deck and map retain independent
pan/zoom poses, so switching views does not destroy either composition.

## Bot spectate

Bot spectate issues one normal `TacticsAction` every 0.45 seconds. Its floating
runner projections interpolate between the authoritative cells selected by that
same navigation action and idle with a small hover animation. It collects
required keystones, coordinates the squad at a two-operator station, uses an
anchor when standing in a telegraphed pocket, and then routes everyone to the
exit. The bot has no alternate mutation path: its decisions pass through the
same legality, AP, action log, observation, Guardian, and relayout rules as
manual play. `Bot step` is the useful debugging mode—each press exposes one
decision and its resulting digest-sized simulation change while the camera and
deck controls remain available.

## What it reuses

Almost everything. The point of the lab is that a conclusion drawn here is a
conclusion about the game, not about a model of it.

- `observed_facility::hex_wfc::HexWfcWorld` — the real solver, and the real
  `begin_frontier_relayout` → `advance_relayout` → `commit_relayout_delta` cycle.
- `observed_match::hex_wfc` — `HexPlayerMapKnowledge` (fog and staleness),
  `HexLanternState` (anchors), `HexPadState` (teleport plates),
  `HexGuardianStatus`.
- `observed_style` — every colour on screen.
- `observed_schematic` — line and band meshes, shared with `iso_observer_lab`.
- `observed_cutaway` — cached authored hulls plus shared cutaway and configurable low-wall projection.
- `observed_ui` — the setup screen's widgets and focus.

Three things could not be borrowed, and each says so where it lives: the
Guardian's cadence is per-tick in the shipped game and per-turn here
(`sim/adversary.rs`), fog recording is per-hop rather than a fixed one hop
(`sim/knowledge.rs`), and an anchor pins a **cell** rather than a named doorway,
because a unit with no facing has no doorway to choose (`sim/mod.rs`).

## What it drops

Physical bodies, collision, continuous simulation position, and the sub-room
placement of objectives. A unit is logically a cell; the floating production
runner silhouette and its interpolation are presentation only.

## The telegraph

The mechanic worth watching. At the start of a turn the pocket that will
re-collapse is selected, solved, and **drawn**. You spend the turn deciding
whether to hold it — standing in it, or anchoring it — or to spend that time
covering ground instead. At the end of the turn the commit is attempted against
wherever the squad actually ended up, and `commit_relayout_delta` refuses it if
you held it.

This is the shipped `MUTATION_WARNING_TICKS` contract at a cadence a human can
read. Because a tactics turn permits several whole-cell moves while the real-time
warning does not, pocket selection reserves one extra protected ring. That keeps
routine movement from accidentally holding most warnings. Pocket size also
scales from eight cells on the smallest map toward the production 32-cell target;
the production-sized pocket had occupied 40% of an 80-tile teaching floor.
Both adjustments preserve the
same commit rule: current observation and anchors always win. A zero-delta solve
is reported separately from a hold, while a real committed pocket remains
magenta for the following turn, including in full-map mode.

Initial solves use the production composition-profile boundary with a
lab-authored readability profile. It increases true WFC void space, suppresses
repeated straight runs and full-height shafts, and favours turns, junctions,
ramps, and expanses. Global and per-district void biases stack inside the
profile's existing safe bounds, leaving neutral production seeds and their
pinned traversal evidence unchanged. No route or room is removed after generation: normal WFC adjacency,
connectivity, room, and objective validation still decide every accepted map.

## Evidence

```powershell
$env:OBSERVED2_CAPTURE="docs/evidence/tactics_lab"; cargo run -p tactics_lab
```

Set `OBSERVED2_CAPTURE_VIEW=map` to capture the operations map instead of the
one-third-wall deck. Set `OBSERVED2_CAPTURE_PRESET=guided` to capture the teaching
configuration, or set `OBSERVED2_CAPTURE_FULL_MAP=1` to reveal every cell while
capturing another preset.

Lets the deterministic spectator bot play one match per seed and writes a frame per turn plus a
`manifest.json` recording, for every turn, how many cells the facility changed,
how many the squad held, how many cells are truly blank, how many nonblank
archetypes appear, and how much of the map it knew. Two settings
configurations are compared by diffing manifests, not by squinting at images.

## Success and failure conditions

The lab succeeds if:

- holding a telegraphed pocket visibly refuses the shift, and the HUD says so;
- turning `Facility shift` off produces a match that feels *worse*;
- the authored one-third-wall deck and active-deck operations map never disagree about what a cell is;
- a bot spectate run can finish an objective match through ordinary logged actions.

## Presentation direction

The detailed deck uses the committed authored WFC catalogue rather than an
approximation. Ceilings are removed and every perimeter hull is capped at one
third of the canonical deck height; floors, ramps, stairs, columns, and other
interior structure keep their authored shape. Shaft connections use compact
plan-view up/down arrows instead of the large staircase profile. Ordinary hulls,
map floors, and construction lines retain their style-owned architecture
register accent, with one local practical-light pool per visible district.
Hover names the register and the legend explains the structural hue; observation,
anchors, warnings, routes, and selection still replace atmosphere with their
named signal treatment. Route, selection, and relayout warnings have translucent
floor area as well as line work. The 3D
viewport ends where the fixed right-side command dock begins, so UI never covers
a selectable cell. The second view is a top-down schematic of one deck with real
doorway gaps and explicit vertical glyphs, not an occluded stack of every floor.
No render or pointer state participates in the match digest or replay log.

It fails if spending observation on holding ground is never worth more than
spending it on distance — in which case the freeze is decoration, and the
settings are where to look first.
