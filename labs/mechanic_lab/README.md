# mechanic_lab

A mechanic bench. A fixed turn pipeline whose every stage is a swappable
strategy, so two rule sets can be compared without editing the lab that
implements them.

## The technical question

*Can game mechanics be swapped at runtime fast enough to find out which ones
are fun?*

Deliberately **not** "is rule X fun". Arc T recorded four findings sitting on
*fix landed, awaiting human verification*, and then all four failed at once
(`docs/arc_t/README.md` §1a). They failed together because trying an
alternative meant editing the lab that implemented the original. This lab
removes that cost.

`labs/tactics_lab` is the nearest neighbour and answers a different question —
whether the *real* observe-to-freeze loop reads to a player — by driving the
real `HexWfcWorld`. This lab does not, and says why under **Substrate**.

## Status

Steps 1-6 of 7 (`docs/mechanic_lab_plan.md`): the simulation, the 2D board, the
runtime mode picker, and the browser build. Two modes plus five variants, 26
tests. Metrics (step 7) are the remainder.

```bash
cargo dev-run -p mechanic_lab                       # play it
cargo test -p mechanic_lab
cargo run -p mechanic_lab --example trace -- base   # one match, turn by turn
cargo run -p mechanic_lab --example sweep           # one match per variant
```

### Browser build

```bash
bash scripts/build-mechanic-web.sh
python3 scripts/serve-tactics-web.py --directory web-dist/mechanic-lab --port 8080
```

The serve script is bundle-agnostic despite the name. Deployed, the lab is
`/mechanic/` on the labs image, beside `/tactics/` and `/composition/`.

## Playing it

Orders are **declared, then resolved together** — simultaneous resolution means
nothing is timed, every order is visible before it counts, and any of them can
be changed. That is also what makes it work with one thumb.

| Do | Get |
| --- | --- |
| tap your pawn | select it; its open neighbours light up |
| tap a neighbour | step there, facing that way |
| `Turn in place` then tap | set facing without moving |
| `Hold` / `Plant` | order the selected pawn to stand, or to plant |
| `Next` | cycle selection; walks a stacked cell before leaving it |
| `Watch` | hand both sides to the driver and let it play on a clock |
| `< Turn` / `Turn >` | rotate the selected pawn one face, keeping its move |
| `Resolve` | your orders and the bot's resolve together |
| `More` | Modes, Step, Vision, Legend, Restart |
| `< Mode` / `Mode >` | swap the whole rule set and deal a fresh match |
| `Legend` | what every mark on the board means |

The facing arrow is drawn on every pawn, not only the selected one: cone vision
is the mechanic, so where a pawn is looking has to be readable at a glance.

### Choosing a facing for after the move

Tapping a neighbour sets the step *and* points the pawn that way, which is the
common case. To arrive facing somewhere else, declare the move and then press
`< Turn` / `Turn >` — rotation keeps the queued action and only changes the
facing, because facing costs no action.

**The cone preview draws from the destination, not from where the pawn is
standing.** Answering it from the cell the pawn is about to leave shows the
consequence of a facing it is no longer going to have, which is worse than
showing nothing. The status line names the projected pose for the same reason.

### The dock

Three rows of full-width controls rather than a wrap of small ones. The first
version packed eleven 62x40 buttons into a 375px screen, which is under every
published minimum for a touch target and felt like it. Common controls are now
54px tall and share the row width evenly; the four that are consulted rather
than used — Modes, Vision, Legend, Restart — live behind `More`.

### Stacking

`Stacking` is a seam, not a decision. Forbidden (the default) makes a narrow
doorway a real bottleneck for your own squad; Allowed lets a squad move as one
body and makes overwatch far easier to satisfy. Rivals were never affected —
cross-team contact is what recency adjudicates.

Two presets in the mode list run with it Allowed. A stacked cell fans its pawns
around the centre and `Next pawn` walks them, so nobody ends up unreachable
under someone else.

## The rule that keeps the framework from eating the lab

`agents.md` forbids speculative abstractions, and a runtime mechanic framework
is the textbook case. It is defensible here only because the framework *is* the
subject — so one rule holds it in check:

> **A trait ships with two implementations or it is not a trait yet.**

One implementor is speculation. Two is a comparison, which is the point.
`every_seam_changes_the_match_it_is_swapped_into` enforces the other half: a
seam whose two implementations produce identical matches fails the build. A
seam no test can distinguish is not a seam.

## The pipeline

```text
open turn        telegraph the change, clear immunity, record prev_at
  -> [locks]     when ConeTiming::PreMove
  -> resolve     facing, movement conflicts, plants
  -> release     a free pawn standing in the prison frees the held
  -> [locks]     when ConeTiming::PostMove
  -> threats     guardians move and take; cone interaction consults locks
  -> mutate      unheld telegraphed cells reseal, oldest seals re-open
  -> evaluate    outcome, or another turn
```

The pipeline never varies; only which strategy fills each stage. `ConeTiming` is
not a branch inside `Vision` — it *relocates* one call, which is the whole
difference between "locks are a blind commitment about where I will be looking"
and "locks are what I set up last turn".

| Seam | Mode 1 | Second implementation |
| --- | --- | --- |
| `Resolution` | Simultaneous | Sequential |
| `Vision` | Cone { width, range } | Radius { range } |
| `Threat` | Guardians { target, cone_interaction } | None |
| `Setback` | Prison { jailbreak, immunity } | RespawnAtStart |
| `Mutation` | TelegraphedReseal { base, cap, budget } | None |
| `Objective` | PlantFlags { rule } | ReachExit |

## A mode is data, not code

Swapping a mechanic means rebuilding `Rules` from an edited `ModeSpec` — never
mutating a trait object in place. So a mode is printable and shareable as text,
a determinism test is `ModeSpec + intent log -> digest`, and the runtime panel
(step 5) is an editor over data rather than a wiring diagram.

## Mode 2 — Base (stink base)

Two squads on the same radius-3 hexagon, laid out symmetrically under the
180-degree rotation `(q, r) -> (6-q, 6-r)` so neither side has a shorter route
to anything. Each team's near flag is two steps from its base and four from the
rival's; the third sits dead centre. `PlantWin::Majority` therefore makes the
middle flag decide the match, which is where the two squads are forced to meet.

Pawns take each other by **recency**, the Prisoner's Base rule: whoever left
their own base most recently outranks whoever left earlier. Fresher takes
staler, staler cannot touch fresher, equal is a standoff, and a pawn standing in
its own base is untouchable. That is a capture rule with no combat — purely
positional, purely stateful — which is what the north star's "players cannot
directly harm opponents" asks for. It also puts a second tempo cost against the
same limited turns as holding ground does: walking home to refresh is distance
you do not travel toward the objective.

Contact is co-location, not adjacency. On 37 cells an adjacency rule would let
three rival pawns threaten roughly eighteen cells at once, and a board that is
mostly lethal has nothing left to decide. Space contention is therefore
team-local: teammates refuse each other a hex, rivals may share one and are then
adjudicated by recency.

Prisons are per team. `prisons[t]` belongs to team `t` and holds the *other*
team's pawns, so a rescue is a trip into enemy ground. With one team it
degenerates to a single neutral cell, which is what mode 1 uses.

Guardians run alongside the rivals as a neutral hazard, rival contact settling
first. Running guardians first would let the facility take a pawn a rival had
already claimed, and the tag would silently never happen.

## Mode 1 — Plant

Radius-3 hexagon, 37 cells (`3r^2 + 3r + 1`), one team of three pawns,
simultaneous resolution, sixteen turns.

Each turn a pawn declares **one action** (move one hex / hold / plant) **and a
facing**. Facing is free of action cost but still declared before resolution, so
under simultaneous play it stays a blind commitment. A pawn holds the cell it
stands in plus its cone; unheld telegraphed cells reseal.

Guardians hunt on a configurable target policy and send pawns to the prison at
the board's centre, where a free pawn standing on it frees everyone held.

## Two ambiguities resolved in code, either of which is worth overruling

**Overwatch.** The obvious reading of "planting requires observation" is
vacuous: a pawn holds the cell it stands in, so a planter standing on a flag
always observes it. `PlantRule::Overwatch` therefore requires a *different* free
pawn to hold the flag in vision — the two-operator station at squad scale, and
the co-op beat the north star asks for. `PlantRule::StandOnly` is the other
reading, and a test pins the difference.

**The seal budget is a correctness rule, not a difficulty knob.** The first
draft only ever sealed cells. It ate thirty of thirty-seven by turn eight, after
which nothing could move and every seam test was comparing two identical
corpses. It was also wrong about the system being modelled, where architecture
*rewires* when unobserved rather than decaying. `TelegraphedReseal` now re-opens
its oldest seal once the budget is full, so the wall rolls around the board
instead of closing on it.

## Substrate

An abstract hand-authored lattice: cells are open or sealed, connectivity is
adjacency, rewiring is flipping cells. No WFC solver, no compiled tile catalog.

**The trade, stated plainly:** this is a model of the rules, not the rules. A
mode that wins here must be re-proven in `tactics_lab` against the real solver
before it says anything about the shipped game. That second proof buys a bench
built in days rather than weeks, and a browser bundle small enough to judge on a
phone.

Reused: `observed_hex` (dependency-free; `HexFace::LATERAL` is pawn facing,
rotation is `±1 mod 6`, and a cone is a contiguous arc of face indices). Its
bounds are rhombic, so the hexagon is a 7x7 grid plus a mask — not a change to a
crate the solver and importer also use. The sim carries its own SplitMix64, so
no `rand`/`getrandom` and none of the wasm entropy workaround `tactics_lab`
needs.

## The architect: a player as the facility

`ModeSpec::architect` hands one team's mutation to a player. That it required no
new subsystem is the point — `Mutation` was always the strategy answering "what
does the facility do when nobody is looking", and the answer here is "whatever
somebody decided". A player *is* a `Mutation` implementation.

The rogue churn keeps running beside it, because the question this slice exists
to answer is not whether a player can lay tiles but **whether the other players
can tell a deliberate play from background noise**. An architect with the board
to itself would never test that.

A tile is a cell's whole port signature — `Sealed`, `DeadEnd`, `Corridor`,
`Bend`, `Junction`, `Hall`, `Room` — with a rotation. That is the WFC unit, and
it is what makes a hand mean something: a corridor and a junction are different
offers. Boundary-level plays would have been simpler and tested nothing, since
every play would be the same play. It also gives the model rooms-versus-
corridors, which a lattice of identical cells cannot express.

**The hand is the cadence throttle, and it is the fix for a problem this lab
already measured.** Unbounded churn produced fifty-odd changes a turn and read
as noise. A player holding four tiles cannot flood the board even wanting to, so
what lands is always few enough to read as intent.

**Held ground refuses architecture** — observe-to-freeze from the other side of
the table. An architect may not rebuild what the operatives are holding,
*including their own team's*: their attention is the price of their safety, and
it is paid to their own architect as much as to the enemy's. In practice the
architect can only build two or more hexes clear of its squad, which mechanically
enforces the division the design wants:

> **The architect builds the future. Observation protects the present.**

Every accepted play is checked for solvability, exhaustively in test: seven
shapes by six rotations by thirty-seven cells, asserting the flags and the squad
stay mutually reachable. An architect who can wall the objective off by accident
is worse than one who cannot play at all.

```powershell
cargo run -p mechanic_lab --example play -- architect "0:H 1:H 2:H A:4,4,corridor,1"
```

Orders are `A:q,r,shape,rotation`. Refusals are printed with their reason,
because refusals are where an architect's skill lives.

### The squad earns the architect's cards

**Nothing refills on a clock.** A card arrives because somebody earned it:

| Operatives do | Architect draws |
| --- | --- |
| plant a flag | 3 |
| free a teammate from prison | 2 |
| hold ground and refuse a rogue rewire | 1 each |

That points the dependency the right way round — operatives generate the
resource, the architect spends it on their behalf — and it makes the loop close
on itself: *observation earns architecture*. Attention spent holding ground
against the facility becomes the material the architect rebuilds it with. A free
trickle would have made the architect self-sufficient and the operatives
optional, which is the failure mode asymmetric designs die of.

A test asserts the end state rather than assuming it: an architect spending
tiles for an idle squad runs dry.

### The look-away trade

Playing the architect alongside operatives surfaces a tension on the first turn
that no amount of reasoning about the rules had predicted:

**To let your architect build, your operatives must look away — and looking away
is exactly what exposes them to the guardians.**

Held ground refuses architecture, and a pawn's cone is three faces wide, so a
squad watching its surroundings closes off most of the ground its own architect
wants. Turning to face elsewhere opens the site *and* drops the teammate shield
that keeps guardians out. The two mechanics were designed independently and pull
against each other through a third: attention is a single currency spent on
safety, on holding structure, and on permitting your own architect to work.

That is the asymmetric design earning its place. Neither seat can act without
costing the other something, which is a far better dependency than a rule
saying two pawns must stand together.

### The economy bites immediately

Two turns of architect play emptied a four-card hand, because the squad had
earned nothing back. That is the intended shape working faster than expected:
an architect cannot simply build, it must build *toward things that pay*, and
the operatives are the only source of payment.

It also changes what a request means. Asking the architect for a route is
asking it to spend a limited resource that only you can replace, so "can you
open me a path" becomes a negotiation rather than an order — which is exactly
the dependency the asymmetric design is for.

### The architect cannot fix the problem in front of you

The sharpest limitation the duel exposed. An operative reached the flag's
doorstep while the rest of the squad was cut off from it by a single wall — and
that wall was the one thing the architect could not touch, because pawns stood
on both sides of it. The request "open the boundary between the two cells my
squad is standing in" is precisely the request the rules forbid.

So the architect is at its most useless exactly where the squad most wants help,
and its usefulness is greatest where nobody is yet looking. Whether that reads
as a satisfying division of labour or as a frustrating one is the thing a human
in the operative seat has to judge; it cannot be settled from the rules.

### The loop closed

Played end to end against a Haiku architect, the full cycle ran in six turns:

1. I asked in prose for a route to the flag at `(0,6)`.
2. The architect spent a card on a junction at `(1,5)` whose SW doorway opened
   onto it — geometry answering a sentence.
3. My squad turned away from the site so the tile could land, then walked the
   route it had made.
4. Planting the flag paid the architect three cards, taking a hand that had run
   down to two unusable tiles back up to four.

Neither seat could have done any part of that alone, and the payment arrived
because the *other* seat succeeded. That is the asymmetric design doing the one
thing it exists to do.

### Not built yet

The architect is **headless only**. The browser build has no tile-laying UI, so
`--example play` is the whole interface. A second architect for the rival team,
and any of the first-person half, are also unbuilt — this slice tests whether
the *rules* work, not whether the seat is fun to sit in.

## The shield is a teammate mechanic

`ConeInteraction::Blocked` protects the cells a pawn is **looking at**, not the
cell it stands in. That distinction is the whole rule.

The first version consulted the full lock set, and a pawn's own cell is always
held — so no guardian could ever enter a cell containing a pawn, every pawn was
permanently invulnerable, and both guardians spent an entire hand-played match
parked next to the squad doing nothing at all. `LockSet` now separates *held*
(by occupancy, which stops the floor rewiring) from *covered* (by sight, which
is what shields), and the guardians read the second.

So **looking after yourself is not a defence — somebody else has to be watching
you.** That is the co-operation within a team the north star asks for, and it
gives facing a job beyond holding structure: face the guardian and your
teammate lives, turn away and they do not.

## Everything unobserved is in play

`Scope::AllUnobserved` telegraphs *every* boundary nobody is holding, rather
than a capped handful. It is the strongest reading of observe-to-freeze: the
only stable architecture is the architecture somebody has their eyes on. A
capped churn lets a player ignore the facility for turns at a time; this one
never does. `Scope::Capped` remains as a preset, and the seam tests use it —
under a full churn the board reshapes faster than a drive can express anything,
so every variant ends the same way and the other seams stop being measurable.

## What walls changed

Boundaries carry passability now: every cell is floor, and what mutates is the
wall between two cells. Three reasons, and only the first was the plan.

**It is the answer to legibility.** A wall is a bar and a doorway is a gap.
The most important information on the board needs no hue at all, which no
palette could have achieved.

**It is truer to the modelled system**, where architecture rewires rather than
decays — and it retires the seal-budget hack, because closing one doorway while
opening another keeps the board alive by construction rather than by a counter.

**It makes rooms and corridors expressible.** `doorway_count` distinguishes
them; the previous model could not represent the difference at all, and the
north star's tension/release rhythm depends on it.

Four things the change surfaced that the plan did not predict:

**Corridors are traps.** Walls made evasion *harder*, not easier — mode 1 now
loses all three pawns in six turns where it used to survive sixteen. Arguably
that is the design working (corridors are meant to be the risky beat), but the
current numbers are too lethal and are left untuned rather than quietly fixed.

**Cone occlusion nearly killed Overwatch.** An escort must now see the flag
*through a doorway*, which the driver almost never manages, so mode 1 plants
nothing at all.

**Walls reduce movement contention.** Resolution order became unobservable in
driver play at exactly the shipped configuration - three pawns, twenty-two walls
- because corridors force single file. It differs at every other pawn and wall
count measured. The seam is real; the board was hiding it.

**The digest was missing the outcome**, so a won match and a lost one could
fingerprint identically. Found by a seam test that should have failed and did
not.

## What a played duel found

Two players — me on team 0, a Haiku agent on team 1 — through
`--example play -- base "<orders>" --duel`, which replays the whole order log
from the seed each invocation and so needs no saved state.

**Team 1 won on turn 7 while losing 1-0 on flags.** My last free pawn was
caught, team 0 had nobody standing, and elimination decided it. The bot sweep
hinted at this; two thinking players confirm it. **While guardians are live the
flag objective is decorative** — attrition settles every match.

**The lone-pawn spiral has no floor.** Since only a *different* pawn's cone
shields you, a team down to one free pawn can never be shielded again: that
pawn is certain to fall, and the only recovery is walking to the prison alone
and unshielded. Both sides hit it, neither escaped.

**Recency never fired once.** Every pawn spent the match at `left_base_at = 0`
because the trip home costs more than it buys, so every contact was a standoff —
two rival pawns shared a hex with nothing happening. Stink base is currently
inert, and the cheap-refresh options from its design note are unspent.

**Both players lost pawns to guardians in the same way**: moving into a hex on
the turn a guardian steps onto it, with nobody looking. The shield works — a
guardian sat beside a covered flag-planter for two turns and could not take it —
but using it means predicting a simultaneous move *and* having a teammate
already facing the right way.

## What the first sweep found

`--example sweep` plays one scripted match per variant. Three results, all from
**bot play** — the driver is a simple player, so these bound what is reachable,
not what is fun:

| Variant | Result |
| --- | --- |
| Mode 2 as shipped | 0/3 flags, won on elimination |
| Mode 2, guardians off | 3/3 flags in 5 turns |
| Mode 2, `StandOnly` instead of `Overwatch` | 3/3 flags |
| Mode 1 as shipped | 1/3 flags, lost on the turn limit |

**Overwatch and attrition compound into a death spiral.** Planting needs two
pawns at the objective at once, so every pawn lost makes the next flag
disproportionately harder, and a team below two free pawns can never plant
again. Guardians alone are survivable and overwatch alone is satisfiable; the
pair is what closes the objective off. Mode 2 as shipped therefore always
resolves by elimination and never by flags, which makes the objective decorative
in exactly the configuration meant to showcase it.

**Mode 1's sixteen turns are not enough for three flags under overwatch**, even
unopposed — the unguarded run reaches 2/3.

Neither is tuned away, because tuning them silently would hide the finding. The
levers are on the table: turn limit, `PlantRule`, `PlantWin::Majority` for a
single team, guardian cadence and count.

## Art

Icons are authored as real SVG under `art/`, `include_str!`d into the binary and
rasterized at startup with `usvg` + `tiny-skia`. No asset server, no runtime
fetches, one file still ships to the browser — but the art is a vector file any
tool can open and any review can diff, which is what makes iterating on it
cheap. `agents.md` prefers code-as-art over authored assets; SVG text in the
repository is the reading of that which keeps the art editable without
introducing a binary asset pipeline.

The direction is Chip's Challenge: chunky, pictographic, hard black outlines,
flat fills, one unmistakable silhouette per thing. That direction was chosen for
legibility rather than nostalgia — a heavy-outlined silhouette survives any
colour-vision deficiency, and reads at the size a 37-cell board leaves you on a
phone.

**The vision simulation reaches the pixels.** Each icon is rasterized once per
`ColorVisionMode` at startup, with `simulate_color_vision` applied per pixel, so
the `Vision` control shows a simulated board with simulated art on it. Tinting
the sprite instead would have left the control lying about the artwork, which is
worse than not offering it.

## Motion, the channel colour cannot take

A telegraphed boundary **breathes**, and the rhythm carries the outcome:

| About to | Rhythm |
| --- | --- |
| wall up | fast, hard flash — it takes a route away |
| open | slow, soft breath — it is an offer, not a loss |
| change, outcome hidden | a neutral middle pulse |

Both cells the boundary joins pulse with it, because a bar on an edge is a small
thing to notice on a phone. Strip every colour out and closing is still
distinguishable from opening, which is the point: motion reads identically under
every colour-vision deficiency, at any palette, on any screen. It is the
strongest signal available for the thing a player most needs to notice.

The animation is a separate system from the board redraw. The redraw only runs
when the match changes; an animation that forced a full respawn every frame
would be paying entity churn for a sine wave.

## Watching two drivers play

`Watch` hands both sides to the scripted driver and advances a turn every 1.25
seconds — comfortably longer than the movement glide, so each turn resolves,
settles, and sits readable for a moment before the next begins. `Step` under
`More` advances exactly one turn.

The point is not a demo. A match you *watch* is the cheapest way to see whether
a mode has a shape at all: whether turns differ from one another, whether
anything ever gets held, whether the objective is reachable. `--example sweep`
answers that as a number; this answers it as something you can look at, which is
the only kind of answer that closes a question about fun.

Its clock is a separate resource rather than a field on `Session`, and that is
load-bearing: the board only redraws when `Session` changes, so a timer ticking
inside it would mark the session changed every frame, respawn every entity, and
restart each glide from zero — nothing would ever appear to move.

## Movement is animated, because simultaneous resolution hides itself

Everything moves at once, and everything moving at once is exactly what a
teleport conceals: three pawns, two guardians and a rewired wall all changing
between one frame and the next is not a turn anyone can read. Actors slide from
where they were to where they are over about a third of a second — long enough
to see who went where, short enough never to become a wait.

## Reading the board without colour

Every distinction is carried by **shape first**, colour second. That is not a
courtesy: `observed_style::outline` already puts width in the semantic
treatment "so color is never the only channel carrying meaning", and the
Legibility Contract forbids an unlabelled coloured marker.

| Thing | Shape |
| --- | --- |
| wall | brick bar across the boundary |
| doorway | the absence of one |
| boundary about to change | dashed ghost bar, breathing |
| your pawn | round helm |
| rival pawn | diamond |
| pawn in prison | broken ring, crossed out |
| guardian | triangle with an eye |
| flag | pennant, hollow until planted |
| prison | barred box |
| base | hexagon ring with a tick |
| observed cell | inner ring on quiet floor |
| isolated cell | hatched |
| facing | arrow on the faced edge |

The **Vision** control cycles all five `ColorVisionMode` simulations over the
live board, not a swatch page — every colour is routed through
`simulate_color_vision` before it reaches a material. The default palette is
tuned against deuteranopia.

## Known simplifications

- Guardians may share a cell; only pawns contend for space. Two guardians on the
  same target policy therefore move as one, which looks odd and is on the list.
- Movement is one hex per turn. The conflict table assumes it.
- Guardian cadence is the reason guardians are escapable at all. At cadence 1 a
  guardian closes one hex per turn and so does a pawn, so it never gives
  distance back and corners you against the rim; the first draft jailed every
  pawn by turn seven, every time.
- The scripted driver plans one squad at one flag. It evades guardians, rescues
  and refreshes, but it does not contest, feint, or split — so its verdicts are
  a floor on what the rules permit, never a ceiling.

## Success and failure conditions

Succeeds if mode 1 is playable end to end, every seam swaps from the running app
without a rebuild, and swapping one seam visibly changes how a match plays.

Fails if swapped seams produce matches that feel the same — in which case the
seams are in the wrong places and the pipeline decomposition is where to look.

## Gates

`cargo fmt --all`, `cargo clippy -p mechanic_lab --all-targets -- -D warnings`,
`cargo test -p mechanic_lab`.

**Neither gate closes this lab.** Per Arc T §1a no phase closes on a proxy: the
tests prove the seams are real and the matches reproducible; a person decides
whether any of it is fun.
