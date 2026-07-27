# Phase 112 — Co-op Mode & the Sixteen-Seat Roster

**Status:** `[x]` — landed 2026-07-27.

The simulation was ready for this and the transport was not.

## The cliff was real, not theoretical

`WireFrame.commands` was `[WireHexCommand; 4]` — a fixed four commands, no count.
That single type is what pinned the whole stack to 2v2: the server's `bind`
rejected any other roster with "the LAN wire format requires a 2v2 match
configuration", and it was right to.

The arithmetic at sixteen seats, which the plan predicted and this phase
confirmed:

| | bytes |
|---|---|
| one command (`WireIntent` + actions) | 6 |
| a sixteen-seat frame (tick + count + commands + digest) | 113 |
| `FRAME_WINDOW` (16) of them | **1 808** |
| `MAX_DATAGRAM` | **1 200** |

So the bundle did not degrade at sixteen seats — `encode` returned `Oversized`
and nothing went out at all. The window is now budgeted by bytes
(`frames_per_bundle`) rather than fixed at sixteen frames, so it shrinks as the
roster grows. The test pins it at the boundary, asserting both that the budgeted
bundle fits **and** that one frame more does not, because a slack budget would
keep passing while the cliff crept back.

`LAN_PROTOCOL_VERSION` goes to 2. This matters more than a version bump usually
does: a version-1 client reading a version-2 frame would not fail cleanly, it
would read the count byte as the first byte of a tick and desynchronise silently.

A seat count past the cap is refused at decode rather than allocated for.

## Three copies of the same number

`observed_net::lan::MAX_SEATS`, `observed_match::hex_wfc::MAX_ROSTER` and
`observed_progression::session::lan::LAN_MAX_SEATS` must all be 16. The
simulation's guard was **8** — below what the widened wire can carry — which is a
particularly unpleasant kind of disagreement: a lobby fills to the wire's cap and
the match then refuses to start, so the failure lands at match start rather than
at configuration time.

No crate can see all three (`observed_net` depends on `observed_match`, and
neither depends on `observed_progression`), so the agreement is asserted where
two of them are visible and the third pins itself to the literal with a comment
naming the other two.

## The roster is host configuration now

`LAN_TEAM_COUNT` / `LAN_MEMBERS_PER_TEAM` / `LAN_ROSTER_SIZE` became a `LanRoster
{ teams, members_per_team }` carried on the session. They were used in only two
files, which made this far smaller than it looked.

A bad roster **clamps** rather than refusing to start a server: a host who types
`--team-size 40` gets the nearest seatable roster, not a dead process. The server
gains `--teams`, `--team-size`, `--co-op N` and `--no-guardian`.

## Co-op needed almost nothing

The simulation already had the semantics: team completion and map knowledge are
both keyed by team, so one team means one shared sketch and one shared escape
condition. `LanRoster::co_op(n)` is `teams: 1, members_per_team: n`, and the
tests pin the behaviour rather than adding machinery to produce it.

## Bots off means fewer seats, not idle bodies

Every non-local seat was bot-driven unconditionally, with no way to say
otherwise. `Settings` gains `bot_fill`, `guardian` and `co_op_team_size`.

The important decision is that **bot fill is a roster decision, not a per-tick
one**. Turning it off shrinks the match to the seats a human occupies rather than
leaving seats filled with drivers that do nothing — an unfilled seat would be a
body standing in the facility, which is worse than no body at all.

The Guardian toggle is on `HexMatchConfig` and **travels on the wire**. It has to:
a host running co-op without a Guardian and a client assuming one would diverge
on the first tick the Guardian would have moved. It is still constructed and
still snapshotted when disabled, so the digest keeps its shape.

## Measured

- **Sixteen seats run.** A 4x4 match constructs, steps 240 ticks with all sixteen
  bodies driven, and every body stays in the world. Seventeen is refused.
- **Co-op shares one map.** Four players on one team all read the same sketch.
- **The Guardian toggle works, and the test proves both directions**: disabled it
  never leaves its starting cell, enabled it does — without the second assertion
  the first would pass on a Guardian that simply never moves.
- **The sixteen-seat bundle fits a datagram**, and one frame more does not.

## What this phase did not do

**No hands-on sixteen-seat LAN session.** The plan asked for `lan_lab` at sixteen
seats with a clean reset and a real-UDP packet-loss test at the worst-case frame
size. The codec is pinned at that frame size and the existing real-UDP server
tests still pass, but driving sixteen live clients through the lab is a
hands-on exercise and belongs with the Phase 113 gate, where a human is at the
keyboard anyway.

**No lobby UI for N teams.** The two-team lobby screen still shows two teams. The
session, the wire, the server and the simulation are all N-team; the screen is
the last two-team assumption, and it is a UI task rather than a correctness one —
a host reaches co-op today through `--co-op`.

Both are named in the ROADMAP hand-off rather than left implied.

## Evidence

The gates are the tests named above, in `observed_net` (codec at the datagram
boundary, cap agreement), `observed_match` (sixteen seats, single team, Guardian
toggle) and `observed_progression` (lobby cap).

## Hand-off to Phase 113

- The arc gate is a hands-on playtest. The sixteen-seat co-op session and the
  N-team lobby are the two things it should exercise that no test here covers.
- `server/src/lib.rs` was edited while another session had uncommitted work in
  the same file. Only this phase's hunks were staged, replayed onto a pristine
  copy and verified in an isolated checkout; the other session's change to
  `default_tile_dir` is untouched and still uncommitted.
