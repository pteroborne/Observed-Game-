# `player_input`

The **`player_input`** production crate provides the abstract input boundary for Observed 2.

It decouples character movement systems, AI bots, replay tapes, and network protocol packets from hardware sampling (keyboard, mouse, gamepad).

---

## Core Types

- **`PlayerId(pub u16)`**: Stable identifier for a player slot (`P1`, `P2`, etc.).
- **`PlayerIntent`**: Abstract container capturing normalized 2D movement (`movement`), camera delta (`look`), and semantic action flags (`jump_pressed`, `sprint_held`, `interact_pressed`, `interact_held`, `climb_pressed`).
- **`sanitized()`**: Normalizes movement and look vectors to unit length to prevent diagonal speed exploits.

---

## Testing

Run unit tests:
```bash
cargo test -p player_input
```
