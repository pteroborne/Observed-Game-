# Independent LAN Architects

A three-Observer Ascent team can now connect **one human Architect and three human
Observers**. Previously the Architect connection consumed an Observer slot and a
bot walked that body. The desk now has its own connection and no physical body.

## Ownership and capacity

`observed_core::lan::LanSeatId` distinguishes an Observer's `PlayerId` from an
Architect's `TeamId`. The pure lobby keeps body and desk slots separately. Ascent
adds one desk per team while retaining one-to-three Observer bodies per team and
the sixteen-body total cap. The largest supported lobby therefore has sixteen
bodies and sixteen desks, or thirty-two connections. Facility race retains its
sixteen-body/connection cap and offers no Architect role.

Ready, reservations, team changes, reconnect, expiration and launch preparation
apply to both kinds of connection. `--min-humans` counts both kinds; Ascent accepts
up to its configured capacity, at most 32. `--require-full-roster` requires humans
in every configured body and desk slot before launch. With bot fill enabled, empty
bodies and desks remain bot-controlled. Recovery after an in-match disconnect can
use bots under either startup policy.

Switching from Observer to Architect in the lobby atomically frees the body slot.
Switching back requires a free body on that team. Occupied or reserved targets are
never displaced. Role changes reset readiness and cancel countdown; role/team
changes are locked during a match. A reserved desk reconnects to the same team
without adding a body. Expiration retires its token, and disconnected/retired
connections cannot inject queued commands into a later occupant's slot.

## Protocol and deterministic control

LAN protocol **17** carries a requested join role, a typed personal assignment and
an assignment revision. Delayed Welcome packets cannot roll back a role change.
Launch repeats the personal assignment so a lost role-change acknowledgement
cannot prepare the wrong perspective. Assignment acknowledgements preserve frame
progress, so a delayed pre-launch Welcome cannot rewind a running replica.
Hosts and clients must be updated together;
protocol 16 datagrams fail the version check.

Authoritative frames retain one movement command per physical body and carry a
separate, bounded, sorted list of human-controlled desks and their commands. The
server routes input by validated token/address/assignment, ignores a desk's
movement, and maps desk commands to the existing rules seat for that team. Body
commands still carry Observer requests and corrupted-player Rogue commands.

The desk list also defines human/bot control for that tick. Server and replicas
apply it before advancing the shared rules. A disconnected or synchronizing desk
uses the existing deterministic Architect tree; a caught-up human takes over the
same seat, hand, cooldowns and knowledge. Late joins and reconstruction replay the
same handoffs from authoritative history. Worst-case sixteen-body/sixteen-desk
frames and budgeted bundles stay within the 1,200-byte datagram ceiling.

## Frontend and recorded perspective

The LAN browser explicitly selects **Join as Observer** or **Join as Architect**.
The Play Hub seeds that choice; browser/lobby choices survive returning from the
lobby. Hosting a race disables an incompatible Architect choice. Direct joining
an incompatible host gives a concrete role error.

Lobby rows distinguish the Architect from each body, including readiness and
reservation/preparation state. Role/team actions disable occupied targets. The
roster and team controls paginate together, sixteen role/body rows per page; the
initial page includes the local assignment. Stable team widget IDs survive page
changes, and Leave remains visible at the maximum size.

A network desk sends neutral movement and uses a teammate only as a viewing anchor.
It does not acquire that body's identity, role iris or cosmetic look. Its replay
contains named team members rather than a fictional embodied "You", and defaults
to a real team member. Observer cosmetics remain frozen per body; reconstruction
also preserves the accepted appearance map. No persisted profile/setup schema
changed.

## Verification and evidence

Pure/session and codec regressions cover separate capacity, atomic role changes,
reserved targets, readiness, reconnect, assignment ordering, a lost Welcome,
duplicate desk commands and worst-case datagram budgets. A real UDP regression
launches four connections over three bodies, holds tick zero until the Architect
prepares, rejects desk movement as body control, and verifies bot/human takeover
and token retirement. This also fixes full-roster startup cancelling a pending
launch when no loader had timed out; both successful preparation and actual
timeout cancellation are covered. The game's own four-peer adapter test plays an Architect
card and an Observer request while every replica stays in step. Material/replay
checks verify that a desk has teammate irises and no owned recorded body.

[Native evidence](../evidence/ux/independent_desks_1280x800/README.md) records two
separate graphical processes on one loopback server. Both use semantic browser
Join and lobby Ready actions, the real asynchronous Loading worker, and ordinary
networked runtime entry. The other two Observer connections are neutral-input
transport fixtures that acknowledge preparation/history; they are not graphical
clients. Guardian pressure is disabled and returning-player help completion is
staged. Each graphical client advances hundreds of authoritative ticks with zero
resyncs and three physical bodies. Their recorded digests belong to their own
capture ticks; the pictures are not frame-synchronized.

A separate capture connects one graphical client and thirty-one transport fixtures
to the largest lobby, then visits both roster/team pages. It proves layout and
connection capacity, not thirty-two graphical clients or a full match at that size.
All nine native images include text/control bounds, metadata, checks and hashes.

Human keyboard/pointer/controller play, physical-machine/firewall acceptance and
the known ignored asserting `hex_full_match_soak` remain open. The extended suite
is not claimed green.

To reproduce, build the native game once with `bevy/dynamic_linking`, then launch
the compiled binary in separate temporary working/config directories; do not run
two Cargo builds against the shared cache. Each process sets `OBSERVED2_ASSET_ROOT`,
`OBSERVED2_CONFIG_DIR`, `OBSERVED2_CAPTURE_FRONTEND` (its image directory),
`OBSERVED2_CAPTURE_FRONTEND_DESKS=1`, and `OBSERVED2_CAPTURE_DESK_COORD` (shared
coordination directory). Start `OBSERVED2_CAPTURE_DESK_PEER=architect`, wait for
`address` in that directory, then start `observer`. Use distinct
`OBSERVED2_CAPTURE_DESK_ACCOUNT` values, such as 930 and 931. The `roster` mode runs
alone with its own coordination directory. On Linux, use the dynamic-library path
from the [previous reproduction instructions](cosmetics_loading_implementation.md#reproduce-screenshots).
