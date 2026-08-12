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

The lab opens on a **match setup screen**. Pick a preset — Scout, Standard or
Collapse — or change any row, then start. `R` returns to setup at any time.

| Input | Does |
| --- | --- |
| hover a cell | preview the route, AP cost and this turn's stopping cell |
| click a cell | commit the previewed route; the unit advances one visible step at a time |
| click a unit | select it |
| `Tab` / Next unit | select the next unit that can still act |
| `1`–`6` | select a unit directly |
| `Space` / End turn | end the turn: rivals move, the Guardian hunts, the facility shifts |
| `V` / Deck / overview | switch between the authored active deck and stacked schematic |
| `[` `]` / `-` `+` | browse the active authored deck |
| scroll, right-drag | zoom and pan |
| `Esc` | pause, resume or open the help/legend panel |

Every command has an on-screen control as well as a key. That is deliberate: the
prototype is pointed at a possible touch build, and a lab that can only be driven
from a keyboard would not survive the move.

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
- `observed_cutaway` — cached authored hulls and the shared ceiling/near-wall cutaway.
- `observed_ui` — the setup screen's widgets and focus.

Three things could not be borrowed, and each says so where it lives: the
Guardian's cadence is per-tick in the shipped game and per-turn here
(`sim/adversary.rs`), fog recording is per-hop rather than a fixed one hop
(`sim/knowledge.rs`), and an anchor pins a **cell** rather than a named doorway,
because a unit with no facing has no doorway to choose (`sim/mod.rs`).

## What it drops

Bodies, physics, continuous position, and the sub-room placement of objectives.
A unit is a cell. Everything else follows from that.

## The telegraph

The mechanic worth watching. At the start of a turn the pocket that will
re-collapse is selected, solved, and **drawn**. You spend the turn deciding
whether to hold it — standing in it, or anchoring it — or to spend that time
covering ground instead. At the end of the turn the commit is attempted against
wherever the squad actually ended up, and `commit_relayout_delta` refuses it if
you held it.

This is the shipped `MUTATION_WARNING_TICKS` contract at a cadence a human can
read. Nothing about the rules changed; only the rate.

## Evidence

```powershell
$env:OBSERVED2_CAPTURE="docs/evidence/tactics_lab"; cargo run -p tactics_lab
```

Plays a scripted match per seed and writes a frame per turn plus a
`manifest.json` recording, for every turn, how many cells the facility changed,
how many the squad held, and how much of the map it knew. Two settings
configurations are compared by diffing manifests, not by squinting at images.

## Success and failure conditions

The lab succeeds if:

- holding a telegraphed pocket visibly refuses the shift, and the HUD says so;
- turning `Facility shift` off produces a match that feels *worse*;
- the authored deck and stacked overview never disagree about what a cell is.

## Presentation direction

The detailed deck uses the committed authored WFC catalogue rather than an
approximation. Neutral slate hulls and orange construction lines create the
greybox/dev-grid read; cyan, amber and red are reserved for named interaction
states. The 3D viewport ends where the right-side command dock begins, so UI
never covers a selectable cell. No render or pointer state participates in the
match digest or replay log.

It fails if spending observation on holding ground is never worth more than
spending it on distance — in which case the freeze is decoration, and the
settings are where to look first.
