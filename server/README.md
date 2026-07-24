# `observed_server`

Bevy-free authoritative host for four-seat, two-team LAN matches. The same library
backs the standalone dedicated binary and the game's listen-server option.

```powershell
cargo run -p observed_server -- --bind 0.0.0.0:47624 --name "Workshop"
```

Options: `--bind IP:PORT`, `--name TEXT`, `--min-humans 1..4`, `--seed INTEGER`,
`--tiles PATH`, and `--no-discovery`. The default port is UDP 47624; allow that port
through the host firewall for other machines on the LAN.

The server owns the 60 Hz simulation and emits versioned input frames plus state
digests. Empty/disconnected seats are bot-controlled. Reconnecting and late-joining
clients replay retained authoritative history before regaining control.

