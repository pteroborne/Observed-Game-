# discovery_lab — typed rooms, door reads, and Betrayal-style shifting

A pure-logic feasibility lab for the **room-discovery + gated-objective** mechanic (the
"Part 3" room types in [ROADMAP.md](../../ROADMAP.md)).

## The question it answers

The facility's rooms have a hidden **type** (the core 5, plus three more proven here):

| Glyph | Type | On harvest |
| --- | --- | --- |
| `K` | **Keystone Vault** | +1 keystone (the gate key) |
| `P` | **Power Cache** | +1 power |
| `R` | **Reactor** | +2 power (a richer power node) |
| `C` | **Control** | stabilises the facility (shifting stops) |
| `S` | **Survey** | reveals every room's current type |
| `N` | **Sensor** | reveals only **adjacent** rooms' types (survey at range) |
| `!` | **Decoy** | nothing — but it **lies**: displays as a vault until visited |
| `.` | **Dead-end** | nothing (the bust) |

Phase 39 adds a separate **doorframe read** layer on top of that truth table:
the tile fill is team-local map knowledge, while the top frame/glyph/glow is
the current threshold read from [`observed_style`](../../crates/observed_style).
Most reads match their room type (`K`, `P`, `R`, `C`, `S`, `N`, `.`). An
unvisited Decoy reads as `E` (a false exit signal with exit-coloured sensory
bleed) until direct visitation exposes it as `!`.

You only learn a room's type by **visiting** it, and unobserved rooms **shift** their
types when you look away — so a remembered vault may be a dead end on return. The exit
is **gated**: locked until the team collects `REQUIRED_KEYSTONES` keystones and
`REQUIRED_POWER` power.

Three of the types pose their own behavioural question on top of the gate/solvability
core. **Reactor** makes the power economy *yield-based* (a sum, not a count). **Sensor**
reveals types *at range* — only the 4-neighbour rooms — versus Survey's whole-facility
ping. **Decoy** is the deepest Betrayal turn: it shows as a Keystone Vault when revealed
*remotely* (by a Survey or Sensor), but a direct visit reveals it yields nothing. A decoy
is **never** counted as a real keystone, so deception can mislead the player but can never
strand the run or affect solvability.

For Phase 39, **Sensor** also tags the source of team-map knowledge (`Sensor`) so the
payoff is testable instead of just visual, and **Decoy** also resolves an exit-like
door read (`E`) into the true empty room (`!`) once visited.

The primary question — the analogue of `constraint_lab`'s protected spine — is
**solvability**: one rule (only shift types among *unharvested* rooms) keeps the
objective always completable, because a keystone can never strand on a spent room. With
the constraint off, the same shifting can strand a keystone on an already-harvested room
and make the run impossible. That contrast is what the lab proves.

## Run it

```powershell
cargo run -p discovery_lab
# capture the schematic:
$env:OBSERVED2_CAPTURE = "docs/evidence/discovery_lab.png"; cargo run -p discovery_lab
```

## Controls

- `1`–`9` — visit a room (reveal + harvest its type)
- `Space` — shift the unobserved rooms now
- `X` — try the gated exit
- `A` — toggle auto-explore (sweeps + shifts on a timer)
- `C` — toggle the solvability constraint (watch a keystone strand when off)
- `R` — reset · `F1` — toggle the debug panel

## Success conditions

- The gate starts **LOCKED** and opens only once keystones **and** power meet the
  requirements; `X` escapes only then.
- A room's type reads `?` until visited; visiting reveals and harvests it; harvested
  rooms dim.
- Shifting **conserves** the type multiset (vaults relocate, never vanish) and the
  observed room never shifts.
- With the constraint **ON** the diagnostics' `still solvable` stays `yes` through any
  amount of shifting, and a full sweep escapes. With it **OFF**, some seeds strand a
  keystone and flip it to `NO — run lost`.
- A **Reactor** adds 2 power at once; a **Sensor** lights up only its neighbours; a
  **Decoy** reads as a gold vault when surveyed from afar, then turns out empty on arrival.
- The doorframe read layer is backed by `observed_style::DoorIdentityRole`, has one
  frame/glyph/bleed entity per room, and reset does not leak those entities.
- A **Sensor** feed records adjacent rooms into the team-local map with `Sensor` as the
  source, and an unvisited **Decoy** advertises a false `E` exit read before resolving
  to `!`.
- The seeded bot experiment confirms a scripted **reader** bot beats a random-door bot
  by at least `READER_BOT_TARGET_VISIT_MARGIN` mean visits over `READER_BOT_SEEDS`.
- The `[PASS]` line confirms one tile and one read frame per room plus a single UI root;
  reset restores a fresh facility with no leaked entities.

## What it deliberately does not do

Traversal, competition, equipment, and the real 3D facility are out of scope — this lab
isolates the discovery/gate/solvability logic. Integration reuses `competitive_facility`
+ `equipment_lab` + `incentive_lab` when the gated exit is folded into the teleport
facility. The truth vocabulary is still 8 room types, with a 9-role door-read layer for
the false exit signal; remaining room candidates toward ~10 (Anchor — pin one room; Trap
— scramble memory; Relay — calm the shifting) can be added the same way as this expansion.
