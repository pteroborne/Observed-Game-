# LAN Lab

This resettable lab starts the Bevy-free authoritative server on an ephemeral localhost
UDP port and joins it through the production `LanClient`. It proves the real socket,
compatibility handshake, four stable bot/human seats, ready countdown, server-thread
lifecycle, and reset cleanup before the assembled game consumes them.

```powershell
cargo run -p lan_lab
```

Press `R` to stop the old server thread, close both sockets, and start a clean session.
The monitor reaches `[PASS]` once the authoritative four-seat lobby snapshot arrives.
