# Hex WFC Lab

**Hex WFC Lab** is the retained diagnostic surface for the canonical hexagonal
WFC facility. It combines the compact Phase 90 solve replay with a production
facility atlas for inspecting what the full game actually generated.

## Core features

- **Step-by-step solve replay**: Animates WFC constraint propagation, forced
  cells, domain pruning, and collapse on the compact 12 x 9 x 4 fixture.
- **3D and plan modes**: Switches between level-sliced 2D and navigable 3D.
- **Production corpus atlas**: Uses `HexWfcConfig::arc_default`, production room
  quotas, and the runtime authored catalogue. It searches nearby deterministic
  seeds until all 10 played room roles and all 8 exact hall archetypes occur in
  one solve.
- **Faithful whole-room projection**: Retains authored whole-room prototypes; it
  no longer substitutes every room footprint with isolated cell geometry.
- **Whole-map plus exact local detail**: A two-mesh room/hall overview shows the
  complete solved lattice. Exact authored hulls stream around the free-fly
  camera, avoiding a facility-wide mesh/entity and Rapier cost.
- **Concept index**: Stable previous/next jumps visit every played room and hall
  concept at a useful inspection vantage.
- **Relayout demo**: Demonstrates observation-safe WFC relayout during play.

## Controls

- `P`: Toggle compact replay / production corpus
- `F3`: Toggle 2D plan / 3D facility
- `Space`: Play / pause the compact solve replay
- `N`: Advance the compact replay one step
- `+` / `-`: Change replay speed
- `PgUp` / `PgDn`: Slice the plan-view level
- `1`-`9`: Select a preset seed
- `R`: Solve the next seed in the active generation mode
- `V`: Toggle walk / free-fly in compact mode; production is free-fly only
- `WASD` + mouse: Fly horizontally and look
- `E` / `Q`: Fly up / down
- `Shift`: Fly quickly
- `Home`: Return to the whole-map overview
- `[` / `]`: Previous / next room or hall concept in 3D

## Run

```powershell
cargo run -p hex_wfc_lab
```

Launch directly into the production free-fly atlas:

```powershell
$env:OBSERVED2_HEX_PRODUCTION='1'
cargo run -p hex_wfc_lab
```
