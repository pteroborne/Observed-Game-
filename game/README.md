# Observed 2 — the assembled game

The `game` package builds the final playable binary. It strings the proven simulation crates and lab prototypes into a single cohesive player loop:

```
Splash → Main Menu → Solo/LAN Browser → LAN Lobby → HexWfc → Results → LAN Lobby …
```

---

## Running & Commands

- **Run Playable Game**:
  ```powershell
  cargo dev-run -p observed_game
  ```
- **Join a direct LAN address by default**:
  ```powershell
  $env:OBSERVED2_LAN_ADDRESS = "192.168.1.20:47624"
  cargo dev-run -p observed_game
  ```
- **Run Tests & Architectural Ratchet Checks**:
  ```powershell
  cargo dev-test -p game
  ```

---

## Architectural Boundaries

The game layer enforces strict architectural separation:
1. **Simulation vs Presentation**: `view/` and UI screen modules may import `sim/`, but `sim/` must **NEVER** import `view/` or `screens/`.
2. **Explicit Imports**: No glob re-exports (`pub use x::*`) between modules, and no `use super::*` outside `#[cfg(test)]`.
3. **Ratchet Enforcement**: [`game/src/arch_check.rs`](src/arch_check.rs) automatically fails cargo tests if a glob re-export, forbidden dependency direction, or oversized file creeps back into `game/src/`.

---

## Core Submodules

- **[`main.rs`](src/main.rs) & [`lib.rs`](src/lib.rs)**: Main binary entrypoint and Bevy app plugin composition (`GameState` state machine).
- **[`lan.rs`](src/lan.rs)**: Production LAN client, discovery browser, and optional listen-server lifetime.
- **[`hex_wfc/`](src/hex_wfc/)**: Canonical local/authoritative-LAN play adapter over the 2v2 `observed_match::hex_wfc` simulation.
- **[`full_wfc/`](src/full_wfc/)**: Demoted square lattice regression adapter (`OBSERVED2_MAP=square`).
- **[`sim/`](src/sim/)**: Pure simulation resources (`MatchDirector`, `SpectatorBot`, `LobbyRuntime`).
- **[`view/`](src/view/)**: Presentation building blocks (`theme.rs`, `assets.rs`, `environment.rs`, `components.rs`).
- **[`screens/`](src/screens/)**: Bevy UI screens, including LAN discovery/direct-IP and server-owned team/readiness lobby state.
- **[`evidence/`](src/evidence/)**: Screenshot drivers, tour capture loops, and visual-audit collectors (`OBSERVED2_CAPTURE`).
- **[`arch_check.rs`](src/arch_check.rs)**: Source-scanning architectural ratchet test suite.
