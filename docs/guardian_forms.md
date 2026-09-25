# Guardian forms: candidates for the major Guardian

Three candidate forms for the major Guardian, modelled in `labs/guardian_form_lab`.
Each acts out the four things a major does in a match: hunting, frozen because someone
sees it, frozen by an anchor lantern, and the catch. Each stands beside the existing
minor Guardian, a 1.8 m person and the facility's real doorway (4.5 m wide, 4 m clear).
One is to be chosen for the major; the others may become new kinds of minor.

![The three candidates, the minor Guardian and a person, hunting](evidence/guardian_forms/40_lineup_hunting.png)

## The brief

- **Modron is a mood, not a design.** `agents.md` and the Architect Ascent design both
  say so, and ask for original pyramidal Guardians. The mood taken here is clockwork
  order, rank, and a single dispassionate gaze. No name, part or silhouette is copied.
- **Major and minor differ in silhouette first.** Only a major answers to being looked
  at. The existing minor is small, pale and three-eyed, with legs. Every candidate is
  large, dark and one-eyed, with none.
- **Every state is said in shape and motion, not colour alone.** Frozen must *look*
  frozen with the colour turned off. Colour supports it: the seams go dark and the
  anchor's purple clamps on.

All colours come from `observed_style::guardian`: an oxidised-bronze shell, brass trim,
and the threat red (`MarkerRole::Collapse`) in the eye, the seams and the search beam.
Its tests hold the rules. The shell is darker than a minor's ivory and never emits. The
eye is signal-tier. The beam is haze under the signal floor. A frozen seam is at most a
tenth as bright as a live one.

## The candidates

### Tumbler: a stepped pyramid that locks when seen

Hexagonal tiers, each turning at its own rate like a lock's tumblers, with the eye in a
crown that tracks and scans. **Seen, the tiers snap into line with a latch's overshoot,
it settles onto the floor, the eye fixes on you and the seams go dark.** Being frozen
looks like the facility's own language for observation: a lock closing. Rank is its
tier count: three, four or five.

| Hunting | Frozen by sight | Frozen by an anchor | Catch |
| --- | --- | --- | --- |
| ![](evidence/guardian_forms/11_tumbler_4_hunting.png) | ![](evidence/guardian_forms/12_tumbler_4_frozen_by_sight.png) | ![](evidence/guardian_forms/13_tumbler_4_frozen_by_anchor.png) | ![](evidence/guardian_forms/14_tumbler_4_catch.png) |

Films: [seen](evidence/guardian_forms/film_tumbler_4_seen.mp4) ·
[catch](evidence/guardian_forms/film_tumbler_4_catch.mp4) ·
[encounter](evidence/guardian_forms/15_tumbler_4_encounter.png) ·
[ranks](evidence/guardian_forms/41_tumbler_ranks.png)

### Plumb: an inverted pyramid ringed by orbits

It hangs point-down, turning slowly, so its eye and beam sweep the room like a
lighthouse, while three brass rings orbit it. **Seen, the rings fall flat into a seal,
it turns its eye on you, and it sets its point on the floor.** It is the most uncanny of
the three up close. The catch opens its six faces like petals around a red core.

| Hunting | Frozen by sight | Frozen by an anchor | Catch |
| --- | --- | --- | --- |
| ![](evidence/guardian_forms/21_plumb_hunting.png) | ![](evidence/guardian_forms/22_plumb_frozen_by_sight.png) | ![](evidence/guardian_forms/23_plumb_frozen_by_anchor.png) | ![](evidence/guardian_forms/24_plumb_catch.png) |

Films: [seen](evidence/guardian_forms/film_plumb_seen.mp4) ·
[catch](evidence/guardian_forms/film_plumb_catch.mp4) ·
[encounter](evidence/guardian_forms/25_plumb_encounter.png)

### Roller: an octahedral cage that walks by tumbling

It rolls face to face: a pause, a slow tip, a heavy fall onto the next face. The eye
stays level inside the cage, scanning through its windows. **Seen, it tips up onto a
single point and stays there, impossibly balanced.** No unfrozen body could hold that
pose, so it reads as frozen from any distance. The catch splits it at the equator.

| Hunting | Frozen by sight | Frozen by an anchor | Catch |
| --- | --- | --- | --- |
| ![](evidence/guardian_forms/31_roller_hunting.png) | ![](evidence/guardian_forms/32_roller_frozen_by_sight.png) | ![](evidence/guardian_forms/33_roller_frozen_by_anchor.png) | ![](evidence/guardian_forms/34_roller_catch.png) |

Films: [seen](evidence/guardian_forms/film_roller_seen.mp4) ·
[catch](evidence/guardian_forms/film_roller_catch.mp4) ·
[encounter](evidence/guardian_forms/35_roller_encounter.png)

## Comparing them

| | Tumbler | Plumb | Roller |
| --- | --- | --- | --- |
| Height | 3.2 m (4 tiers); 2.6 to 3.8 m by rank | 3.25 m hovering, 2.9 m grounded | 1.4 m on a face, 2.4 m on a point |
| Frozen reads by | tiers in line, settling, dark seams | rings flat, point grounded, stare | balanced on one point |
| Frozen from a distance | good: the moving stripes stop | good: the rings stop orbiting | best: an impossible pose |
| Headroom under a 4 m lintel | 0.8 m (4 tiers), 0.2 m (5) | 0.75 m | 1.6 m |
| Up close | a watchful tower | most unsettling | a cage around an eye |
| Motion | turning in place, gliding | turning, swaying, gliding | walks, in a heavy rhythm |
| Fits the sim's gliding position | yes | yes | needs the roll timed to its speed |
| Rank variants | natural (tier count) | ring count, perhaps | size |
| Silhouette against the minor | clearly different | clearly different | clearly different |

## Beyond the major

Minor Guardians are never frozen by sight. They are answered by the kinetic tool,
which shoves them into void, off an unrailed edge, or into a retracting tile. Whichever
forms are not chosen could become minors whose *shape* says how to beat them:

- **A small Plumb hovers.** A shove off a ledge does nothing, because it floats. It has
  to be driven into a retracting tile, or through a door that is then closed.
- **A small Roller rolls.** Push it and it keeps tumbling, so it is the easiest to put
  off an unrailed edge, and the hardest to stop coming.
- **The existing minor walks.** It sits between the two.

These are ideas for the minor roster, not decisions.

## Running it

```powershell
cargo dev-run -p guardian_form_lab
$env:OBSERVED2_CAPTURE = "docs/evidence/guardian_forms"; cargo dev-run -p guardian_form_lab
```

Keys: `1`-`3` the Tumbler at 3, 4 or 5 tiers; `4` the Plumb; `5` the Roller; `L` the
line-up; `K` the ranks. States: `H` hunting, `S` frozen by sight, `A` frozen by an
anchor, `C` catch. `Tab` changes the camera, `P` pauses, `R` resets.

The capture writes the stills and a frame folder per film. Films are compiled to MP4
(frames stay out of git):

```bash
ffmpeg -framerate 30 -i film_tumbler_4_seen/f_%04d.png -c:v libx264 -pix_fmt yuv420p -crf 20 -movflags +faststart film_tumbler_4_seen.mp4
```

## What is tested

The poses are pure functions of state and time (`form.rs`), so the lab tests the
properties the choice rests on:

- a frozen Guardian does not move;
- a hunting one always does, including the Roller between rolls;
- the seams go dark exactly when frozen;
- every part's origin stays inside the doorway while hunting or frozen (origins, not
  full extents: the heights above are computed from the dimensions);
- the Roller never sinks or lifts off mid-roll, lands flat, returns to its start, and
  balances on exactly one point when frozen.

## Still thin

- **No sound.** The Tumbler's latch, the rings' settle and the Roller's fall are all
  written to be heard, and none of them is yet.
- **Not in the facility.** The stage is a lab deck, with the facility's light, doorway
  and floor-cell size. The chosen form still has to be promoted into the game's
  Guardian and seen in play.
- **The Roller's roll is not tied to a speed.** It walks a fixed out-and-back. In a
  match its roll would have to keep pace with the simulation's 2.5 m/s glide.
