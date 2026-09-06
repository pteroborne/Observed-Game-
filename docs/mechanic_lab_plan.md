# mechanic_lab — a mechanic bench

**Status:** step 1 of 7 landed 2026-09-05 — the simulation, headless, with the
conflict table and every seam under test. Steps 2-3 folded into step 1: the
second implementations were written alongside the first, because the
seam-difference test cannot exist without them. **Opened 2026-09-05.**

## The technical question

*Can game mechanics be swapped at runtime fast enough to find out which ones are
fun?*

This is deliberately not "is rule X fun". Arc T recorded four findings that sat
on *fix landed, awaiting human verification* and then **all four failed at
once** (`docs/arc_t/README.md` §1a). They failed together because there was no
cheap way to try an alternative — every rule change meant editing the lab that
implemented it. This lab is the instrument that removes that cost.

`labs/tactics_lab` is the closest existing lab and answers a different question:
whether the *real* observe-to-freeze loop reads to a player. It drives the real
`HexWfcWorld`. This one deliberately does not — see **Substrate**, below.

## The discipline that keeps the framework from eating the lab

`agents.md` says *"Avoid speculative abstractions"* and *"Only extract a
generalized framework after the prototype functions correctly."* A runtime
mechanic framework is the textbook violation, and it is only defensible here
because the framework **is** the subject.

One rule holds it in check:

> **A trait ships with two implementations or it is not a trait yet.**

One implementor is speculation. Two is a comparison, which is the point. Every
seam below has a real second implementation from day one; none is a stub. A
seam that no test can tell apart is not a seam — see **Gates**.

## Substrate

An abstract, hand-authored lattice. Cells are `Open` or `Sealed`; connectivity
is adjacency between open cells; "rewiring" is flipping cells. No WFC solver, no
compiled tile catalog, no `observed_cutaway`, no `observed_schematic`.

**The trade, stated plainly:** this is a model of the rules, not the rules. A
mode that wins here must be re-proven in `tactics_lab` against the real solver
before it means anything about the shipped game. That second proof is the price
of building the bench in a week instead of a month, and of a WASM bundle small
enough to judge on a phone.

Reused: `observed_hex` (dependency-free coordinate math), `observed_style`
(colours — never invented locally), `observed_ui` (widgets, Bevy-only, no game
state). Bevy with `2d` + `ui` only. The sim carries its own small deterministic
PRNG, so no `rand`/`getrandom` and none of the wasm entropy workaround
`tactics_lab` needs.

`observed_hex` covers facing for free: `HexFace::LATERAL` is six directions
ordered counterclockwise with `opposite == (face + 3) % 6`, so pawn facing is a
`HexFace`, rotation is `±1 mod 6`, and a cone is a contiguous arc of face
indices. Its bounds are **rhombic**, so a radius-3 hexagon is a 7x7 grid plus an
in-board mask (`lateral_distance(centre, c) <= 3`, 37 cells). A mask, not a
crate change.

## Architecture

### A mode is data, not code

`ModeSpec` is a plain serializable struct. Swapping a mechanic at runtime means
**rebuilding `Rules` from an edited `ModeSpec`** — never mutating trait objects
in place. Three things fall out free: a mode is printable and shareable as text,
a determinism test is `ModeSpec + intent log -> digest`, and the runtime panel
is an editor over data rather than a wiring diagram.

```rust
pub struct ModeSpec {
    pub name: String,
    pub board: BoardSpec,        // radius, prison hex, flag hexes, spawns
    pub teams: u8,               // 1 in mode 1; N supported from commit one
    pub pawns_per_team: u8,
    pub turn_limit: u16,
    pub resolution: ResolutionKind,
    pub vision: VisionKind,
    pub cone_timing: ConeTiming,
    pub guardian: GuardianSpec,
    pub setback: SetbackKind,
    pub mutation: MutationKind,
    pub objective: ObjectiveKind,
}
```

### A fixed pipeline with swappable stages

The pipeline never varies. That is what keeps determinism and testing tractable.

```text
collect intents
  -> resolution.resolve(board, intents)      // movement + conflicts -> final pos + facing
  -> vision.locks(board)                     // cone sampled here when ConeTiming::PostMove
  -> threat.act(board, &locks, setback)      // guardians; cone_interaction consults locks
  -> mutation.apply(board, &locks)           // unlocked telegraphed cells reseal
  -> objective.evaluate(board) -> Option<Outcome>
```

`ConeTiming::PreMove` moves the `vision.locks` call *above* `resolution.resolve`
and carries the result down. That single relocation is the entire difference
between "locks are a blind commitment about where I'll be looking" and "locks
are what I set up last turn", which is why it earns an enum rather than a guess.

`vision` precedes `threat` because guardian behaviour consults the lock set.

### The seams

| Seam | Mode 1 uses | Second implementation, day one |
| --- | --- | --- |
| `Resolution` | **Simultaneous** | Sequential |
| `Vision` | **Cone { width, range }** | Radius { range } — `tactics_lab`'s proven `Sight` |
| `Threat` | **Guardians { count, target, cone_interaction }** | None |
| `Setback` | **Prison { hex, release_immunity }** | RespawnAtStart |
| `Mutation` | **TelegraphedReseal { escalation }** | None |
| `Objective` | **PlantFlags { count, require_observation }** | ReachExit |

Six seams, twelve implementations. The second column is mostly ten-line impls,
and each one proves its seam is real.

## Mode 1 — Plant

Radius-3 hexagon, 37 cells, one level, one team.

- **Pawns** — configurable count. Each turn a pawn declares **one action**
  (move / plant / jailbreak) **and a facing**. Facing is free in that it costs
  no action; it is still declared with the intent, so under simultaneous
  resolution it remains a blind commitment. That distinction is load-bearing.
- **Vision cone** — the pawn's own hex is always locked, plus cells within range
  `R` whose direction from the pawn falls in an arc of `W` faces centred on its
  facing. Defaults `W = 1`, `R = 1`. Both configurable.
- **Mutation** — unlocked cells reseal, telegraphed one turn ahead, count
  escalating with turn number. Without this the cone protects nothing and
  locking is decoration.
- **Guardians** — configurable count. **Target** is a policy:
  `NearestPawn` / `FlagBearer` / `CampNearestUnplantedFlag` / `FixedPatrol`.
  Camping guardians play nothing like hunting ones, which is what makes this a
  strategy rather than a number.
- **Cone interaction** — a config flag with three settings, because reasoning
  did not settle it: `Ignores` (cone holds structure only), `Blocked` (a
  guardian cannot enter or act in an observed cell — facing becomes a shield),
  `Slowed` (a guardian entering an observed cell loses its action — facing buys
  time, not safety).
- **Prison** — default the centre hex: visible, central, contested. A free pawn
  ending its turn there releases every jailed pawn onto that hex, **immune for
  the remainder of that resolution**. Without the immunity a camping guardian
  re-jails them instantly and rescue is impossible.
- **Flags** — three authored hexes, known from the start. **Planting requires
  the hex to be inside the planting pawn's cone at resolution**: you observe it
  into stability, then plant. Facing therefore matters at the moment that
  decides the match. Flagged, so it can be switched off.

## Simultaneous resolution — the conflict table

The hard part, and under-specified in a way that bites if code comes first.

| Situation | Resolution |
| --- | --- |
| Two pawns -> same empty hex | **Both refused**, both hold. Symmetric; no hidden priority. |
| A -> B's hex, B moves elsewhere | Allowed (convoy). Iterate to fixpoint. |
| A -> B and B -> A (swap) | **Refused.** Bodies do not pass through each other. |
| Cycle A -> B -> C -> A | Allowed. A full cycle is consistent. |
| Pawn ends on a telegraphed cell | Movement precedes mutation, so the reseal is refused. |
| Guardian end cell == pawn end cell | Pawn taken by `Setback`. |
| Guardian and pawn swap hexes | Pawn taken — it ran through the guardian. |
| Plant and capture in the same turn | Capture wins; the plant is lost. |

Anything surviving all of the above breaks on pawn id, so the sim is
deterministic.

## Build order

Simulation before presentation, per `agents.md`.

1. `sim` skeleton — `Board`, hexagonal mask, `Pawn`, `Intent`, `Outcome`,
   `ModeSpec`, one path through the pipeline. Headless, no Bevy, unit tested.
   `[x]`
2. Mode 1's six implementations. Determinism digest test. `[x]`
3. The six second implementations, plus the seam-difference tests. `[x]`
4. 2D view, input, HUD. No camera — the board is sized to fit, which removes
   `tactics_lab`'s entire pan/zoom/pose layer and makes touch trivial. `[x]`
5. Runtime mode panel: edit `ModeSpec`, rebuild `Rules`, reset. `[x]` Shipped as
   a preset cycler rather than a field editor: `ModeSpec::presets()` is a list of
   named, complete rule sets, and the dock steps through them. Per-field editing
   is deferred until playing says which field wants editing.
6. `scripts/build-mechanic-web.sh` (bash, modelled on `build-studio-web.sh`) and a
   `/mechanic/` path in `deploy/labs` beside `/tactics/` and `/composition/`.
   `[x]` Also wired into `.github/workflows/lab-web.yml`.
7. Metrics. `[ ]`

Estimated **2,500-3,000 lines** including tests. Roughly a third of
`tactics_lab`'s 9,845. A second mode should cost a few hundred.

## Success and failure conditions

Succeeds if:

- mode 1 is playable end to end, and a match is reproducible from a `ModeSpec`
  plus its intent log;
- every seam can be swapped from the running app without a rebuild;
- swapping a single seam visibly changes how a match plays.

Fails if swapping seams produces matches that feel the same — in which case the
seams are in the wrong places, and the pipeline decomposition is where to look
first.

## Gates

`cargo fmt --all`, `cargo dev-clippy` clean at `-D warnings`, `cargo dev-test`.
Reset returns to a clean state with no leaked entities or resources.

Two tests carry the architecture:

1. **Determinism** — `ModeSpec` + intent log -> stable digest, per mode.
2. **Seams differ** — Sequential and Simultaneous produce *different* outcomes
   on a crafted case, and likewise for each other seam pair. A seam no test can
   distinguish is not a seam.

**Neither gate closes this lab.** Per Arc T §1a, no phase closes on a proxy: the
metrics report whether a mode's turns had more than one non-dominated move, and
a person decides whether it was fun.

## Deferred

Race, Collapse and Blind from the first sketch become `ModeSpec`s once the bench
exists. `Contest` (hold the exit rather than touch it) and `Cargo` (a thing that
cannot move itself) likewise. A second **team** is supported by `ModeSpec` from
commit one so a rival drops in without rework; mode 1 ships with `teams = 1`.

---

## As landed — step 1

18 tests, clippy clean at `-D warnings`, ~1,900 lines. Three things the plan did
not predict:

**The mutation had to become a churn, not a decay.** `TelegraphedReseal` as
specified only sealed cells. It ate thirty of thirty-seven by turn eight, after
which nothing could move and two seam tests failed by comparing identical dead
boards. A `budget` now re-opens the oldest seal, so the wall rolls rather than
closes. This is truer to the modelled system — architecture that *rewires* when
unobserved — and it is a correctness rule rather than a difficulty knob.

**The overwatch rule as first written was vacuous.** A pawn holds the cell it
stands in, so "the planter must observe the flag" is satisfied by definition.
`PlantRule::Overwatch` requires a *second* free pawn instead; `StandOnly` keeps
the naive reading and a test pins the gap.

**The conflict table needed snapshot semantics.** Refusing a contested move in
place dissolves the conflict for the pawn refused second, which then sails
through — an invisible priority, and exactly what "both refused" was written to
avoid. Each pass now compares against a snapshot and applies refusals together,
iterating to a fixpoint.

---

## As landed — stink base (mode 2)

Prisoner's Base recency, added on the user's suggestion, and it fits better than
the guardians it now runs beside: a capture rule with no combat, and a second
tempo cost pulling on the same turns as holding ground. It cost one field
(`Pawn::left_base_at`), one `Threat` implementation, per-team prisons, and one
amendment to the conflict table — space contention became team-local, because
the cross-team contact it used to refuse is exactly what recency adjudicates.

Three further things the build found, all recorded in the lab README:

**Guardians needed a cadence.** At one hex per turn against a pawn's one hex per
turn, a guardian never gives back distance and is strictly inescapable on 37
cells. Every pawn was jailed by turn seven, every run.

**The driver had to learn the guardians existed.** It walked into them, and
every trace read as "guardians are unbeatable" when what was unbeatable was a
squad that could not see them. Ranking candidate steps by `(exposed, distance)`
fixed it and turned a three-turn massacre into sixteen turns of play.

**Overwatch plus attrition is a death spiral**, and it closes the objective off:
mode 2 as shipped never plants a flag and always wins on elimination. Left
untuned on purpose — tuning it silently would hide the finding.

---

## As landed — walls, and why they were the accessibility fix

Passability moved from cells to **boundaries**. Asked for a tileset that could
be read without relying on colour, the strongest available answer turned out to
be the one already on the table from the WFC question: draw passability as
geometry. A wall is a bar, a doorway is a gap, and the most important read on
the board needs no hue. No palette could have matched that.

It also made the fourth request coherent. "Know what a tile will mutate to" is
meaningless while a cell can only vanish; with boundaries there is a *state* to
preview, and `MutationPreview` now offers Hidden / Location / Outcome as a
presentation-only setting, pinned by a test that digests the same match under
all three — the same pin `tactics_lab` puts on its whole-map view.

Findings are recorded in the lab README. The sharpest: walls made the board
*more* lethal, not less, because corridors remove the room to evade; and cone
occlusion made the overwatch rule nearly unsatisfiable, since an escort must now
see the flag through a doorway. Both are left untuned, because tuning them
silently would hide them.
