# LAN integration

Observed 2 LAN play is server authoritative and deterministic. Dedicated and listen
hosts configure teams and Observer/body seats, with up to sixteen Observer bodies.
Facility race defaults to two teams of two and retains four-body co-op. Architect
Ascent supports one-to-three Observer bodies per team plus a non-embodied rules
Architect. Humans may request teams with free seats; unoccupied seats are bot-filled
unless the host requires a full human roster, including the independent Architect
desks in Ascent. Teammates share one survivor-map ledger while rival knowledge remains private.

## Run

```powershell
# Dedicated host (add --ascent for Architect Ascent)
cargo run -p observed_server -- --bind 0.0.0.0:47624 --name "Workshop"

# Each player
cargo dev-run -p observed_game
```

The LAN Play screen listens for UDP broadcast replies and accepts a direct `IP:port`
when broadcast is unavailable. `OBSERVED2_LAN_ADDRESS` sets the initial direct address.
The Host LAN button launches the same server library in a stoppable background thread.

## Architect Ascent on LAN

`--ascent` on the dedicated server - or *Architect Ascent* in the Play Hub before Host LAN -
plays the Ascent rules. The launch names its initial human Architect teams and
each client's independent body or desk assignment. The server and every client build the same rules beside the match
from the launch alone (`observed_match::ascent::facility::architect_seats_where`,
`game/src/hex_wfc/ascent.rs` `lan_rules`): a human Architect where one claimed the desk, a
bot everywhere else.

**The desk.** Choose Join as Architect in the browser, or switch roles in the lobby.
Ascent offers one independently connectable desk per team, in addition to its one-to-three
Observer bodies. A three-body co-op team can have three human Observers and one human
Architect. Switching roles never displaces an occupied or reserved slot; the lobby
shows why an unavailable target is disabled. Facility race offers only body connections.

**Seat commands** (introduced in protocol 5; current LAN protocol 17, `observed_net::lan::WireSeatCommand`). Body commands carry Observer/Rogue requests; a separate sorted desk list carries
Architect commands and per-tick human/bot ownership. A command is nothing on most ticks: a card played, a
requisition, an ask for help (T), an answer to one, and from a corrupted player at the
Rogue board a card play, including its directive and sensor cards. The server puts each into the frame,
and every peer maps it to the same existing rules seat: the team's Architect seat
for a desk connection, the body's own for an Observer/Rogue. They apply it on the same tick, so
the digest keeps them honest exactly as it does movement. A resync rebuilds the rules from
the launch with the match. Frames are budgeted for the largest seat command on every seat,
so a bundle never outgrows a datagram.

Not yet over LAN: bot bodies' asks, because which bodies bots drive changes with who is
connected, and every peer must agree.

## Cosmetics and version compatibility

Protocol 17 retains equipped color/trail/badge IDs in Hello and freezes a validated
look per body in Launch. Gimbals, badges and trails use those choices; team/role
irises remain readable. Every client and reconnect sees the same launch metadata;
late joiners receive their chosen look on the next match. These are presentation
facts and never affect input frames or simulation digests. Lobby snapshots also name
the host's rules and bot-fill policy. Update hosts and clients together; protocol
16 packets fail the version check. See the [cosmetic implementation](ux/canonical_cosmetics_roster_implementation.md) and
[independent desk implementation/evidence](ux/independent_lan_architect_implementation.md).

## Session lifecycle

1. The handshake checks the LAN protocol, hex input version, and canonical simulation
   content hash before assigning a stable Observer/body or Architect/team slot.
2. All connected humans must be ready. The server runs a three-second countdown and
   fills the remaining role slots with bots when enabled; full-roster mode requires
   a human in every configured body and desk.
3. Clients send redundant future input bundles. The server selects one command per
   body and desk at 60 Hz, simulates the canonical match, and broadcasts retained command
   frames with deterministic state digests. The command it simulates is the one it puts
   on the wire, decoded: encoding rounds a command, and every client steps the decoded
   frame, so a server stepping its own unrounded commands parted from every client by
   tick 2 (`server` test `replays_in_step`).
4. Connected humans with missing input receive a neutral command. Disconnected or
   synchronizing bodies and desks use bot control. A seat is
   reserved for 30 seconds; reconnecting and late-joining clients replay history from
   tick one before control transfers back. A client behind the live tick is streamed up
   to twelve bundles a tick, consecutive from what it has applied, and replays frames
   for up to 6 ms a tick rather than a fixed window of sixteen, so it catches up at the
   rate it can replay (about fifty ticks of match a second): a joiner 1,500 ticks behind
   comes into step in about 55 ticks, where it took 138. One bundle a tick held it to a
   datagram's worth of frames a tick, three at sixteen seats, which would have taken ten
   minutes to bring a joiner at minute twenty into step.
5. A digest mismatch triggers one complete deterministic history replay. A repeated
   mismatch disconnects the incompatible client. After the match, the server returns
   connected players to the lobby for another ready cycle.

## Verification

```powershell
cargo test -p observed_server
cargo test -p observed_net
cargo dev-test -p observed_game an_ascent_lan_match   # headless LAN soak
cargo test -p observed_progression session::lan
cargo dev-run -p lan_lab
```

The automated server test crosses real loopback UDP, switches teams, readies, launches,
receives authoritative tick one, and requests/replays history. The game's headless LAN soak
(`hex_wfc::net` tests) runs a real `--ascent` server on the production facility with two
game clients stepping through the game's own networked tick, their bodies walked by the
game's bot as a player would, and a third joining mid-match to replay history from tick
one: about 8,000 ticks, every client ending digest for digest with the server. It fails at
tick 3 if the server steps anything but the commands it sends. `lan_lab` exposes the
same production seam with `R` reset. A release check should still include two physical
machines and host-firewall validation.

## Deliberate non-goals

This is trusted-local-network play: there is no Internet matchmaking, NAT traversal,
relay, encryption, account authentication, anti-cheat, server migration, or persistent
match storage.
