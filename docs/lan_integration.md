# LAN integration

Observed 2 LAN play is server authoritative and deterministic. A headless dedicated
server or an in-game listen host owns one four-seat match: Team 1 is P0/P1 and Team 2
is P2/P3. Humans may request either team while a bot seat is free; otherwise joins are
balanced. Bots fill every unoccupied seat. A team finishes only after both members
escape, and teammates share one survivor-map ledger while rival knowledge remains
private.

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
plays the Ascent rules. The launch says so (LAN protocol 4), and the server and every
client build the same rules beside the match from the launch alone
(`observed_match::ascent::facility::architect_seats`, `game/src/hex_wfc/ascent.rs`
`lan_rules`): a bot Architect for every team, every human a body. They step the same
frames through those rules, so the digest keeps them honest exactly as it does a race;
a resync rebuilds the rules from the launch with the match.

Not yet over LAN: a human Architect, and a body's ask (T), because seat commands do not
travel in frames yet; and bot bodies' asks, because which bodies bots drive changes with
who is connected, and every peer must agree. The Play Hub's *as the Architect* seat plays
as a body on LAN.

## Session lifecycle

1. The handshake checks the LAN protocol, hex input version, and canonical simulation
   content hash before assigning a stable `PlayerId` and `TeamId`.
2. All connected humans must be ready. The server runs a three-second countdown and
   fills the remaining seats with bots.
3. Clients send redundant future input bundles. The server selects one command per
   player at 60 Hz, simulates the canonical match, and broadcasts retained command
   frames with deterministic state digests. The command it simulates is the one it puts
   on the wire, decoded: encoding rounds a command, and every client steps the decoded
   frame, so a server stepping its own unrounded commands parted from every client by
   tick 2 (`server` test `replays_in_step`).
4. Missing/disconnected human commands immediately fall back to bot control. A seat is
   reserved for 30 seconds; reconnecting and late-joining clients replay history from
   tick one before control transfers back.
5. A digest mismatch triggers one complete deterministic history replay. A repeated
   mismatch disconnects the incompatible client. After the match, the server returns
   connected players to the lobby for another ready cycle.

## Verification

```powershell
cargo test -p observed_server
cargo test -p observed_net
cargo test -p observed_progression session::lan
cargo dev-run -p lan_lab
```

The automated server test crosses real loopback UDP, switches teams, readies, launches,
receives authoritative tick one, and requests/replays history. `lan_lab` exposes the
same production seam with `R` reset. A release check should still include two physical
machines and host-firewall validation.

## Deliberate non-goals

This is trusted-local-network play: there is no Internet matchmaking, NAT traversal,
relay, encryption, account authentication, anti-cheat, server migration, or persistent
match storage.
