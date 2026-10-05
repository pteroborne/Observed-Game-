# `observed_server`

Bevy-free authoritative host for configurable N-team LAN matches capped at sixteen
seats. The same library backs the standalone dedicated binary and the game's
listen-server option.

```powershell
cargo run -p observed_server -- --bind 0.0.0.0:47624 --name "Workshop"
```

Options: `--bind IP:PORT`, `--name TEXT`, `--min-humans 1..16`, `--teams 1..16`,
`--team-size 1..16`, `--co-op SIZE`, `--ascent`, `--require-full-roster`,
`--no-guardian`, `--seed INTEGER`, `--tiles PATH`, and `--no-discovery`.
Architect Ascent rejects team sizes outside 1–3 Observer bodies. Its non-embodied
Architect is separate in the rules; a human claiming that desk delegates its
connection's body to a bot. Facility race retains the existing roster limits.
Hosts and clients must use LAN protocol 16 for the frozen cosmetic metadata. The default port is UDP 47624; allow that port
through the host firewall for other machines on the LAN.

The server owns the 60 Hz simulation and emits versioned input frames plus state
digests. Empty/disconnected seats are bot-controlled. Reconnecting and late-joining
clients replay retained authoritative history before regaining control.
