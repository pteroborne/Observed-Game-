# Phase 69 — Light Staging in the Game + the Luminance Gate

**Objective:** promote the lab-proven rigs into the game's place spawners — one
shadow-casting key light per place, per-district temperature pairs, optional
pools-rhythm spacing — and land the luminance-corridor check in the visual
audit. Closes backlog #6. Read [README.md](README.md) first.

## Read first

- `docs/arc_i/phase_68_lighting_lab.md` as-landed note — the rig parameters,
  shadow settings, and volumetrics verdict this phase implements. **Do not start
  before Phase 68 has landed.**
- `game/src/screens/place/lighting.rs` — the current rig (`spawn_place_lighting`:
  unshadowed overhead fill + flicker fixtures). This is the file being grown.
- `game/src/screens/match_runtime/ambience.rs` — palette_for_match, drained/
  klaxon transforms, `FlickerLight` handling, DistanceFog.
- `crates/observed_style` — where per-district light data (key/fill temperature,
  register parameters) belongs; the style crate owns player-facing decisions.
- `game/src/evidence/audit.rs` — the visual audit the luminance check joins;
  the style-presence check (Phase 62) is the pattern to follow.
- `docs/bug_backlog.md` #6 — the defect this phase closes.

## Design rulings (already decided)

- **One shadow-casting key `SpotLight` per place** (lab-tuned settings), plus the
  existing fill/fixtures dimmed to supporting roles. Preview places must be lit
  identically to the place they preview (the existing preview-parity rule).
- **Key/fill temperature pairs are style data**, per district, in
  `observed_style` — not hardcoded in the spawner.
- **The flickering fixture may be the shadow caster** only within comfort limits
  (slow breathing, no strobe); decoherence stutter stays diegetic.
- **Pools rhythm** is a fixture-spacing mode (`fixture_positions` grows a
  rhythmic variant with genuine dark gaps), used where the district register
  calls for it — signals are emissive and must read in the gaps.
- **The luminance-corridor check joins the visual audit** exactly as prototyped:
  it must fail on the archived `phase_62_long_hallway.png` (floor) and an
  all-white fixture (ceiling), and pass on the refreshed captures. `cargo test`
  carries the fixture tests; the audit carries the live gate.
- **Determinism:** light placement stays a pure function of place geometry +
  seed; headless == interactive.
- **Volumetrics land only if Phase 68's verdict was clean** with bloom on; if
  the matrix showed artifacts, volumetric shafts stay lab-only this arc and the
  as-landed note says so.

## Files you may edit

`game/src/screens/place/lighting.rs`, `game/src/screens/place/shell.rs` (light
mount points only), `game/src/screens/match_runtime/ambience.rs`,
`crates/observed_style` (light data + treatments), `game/src/evidence/audit.rs`
(luminance check), `game/src/tests.rs`. Do NOT touch `sim/`, geometry/nav, or
match rules.

## Success criteria

- Captures: the Phase-62 evidence set re-captured (match, long hallway, hallway
  doorway, drained room, flat ceiling) — **every capture passes the luminance
  corridor and is viewed**; the long-hallway capture visibly shows walls, floor,
  and threshold. Backlog #6's "world captures render nearly black" is
  demonstrably false afterward.
- The visual audit runs green with the new check; the check provably fails on
  the archived dark capture (pinned fixture test).
- Drained and klaxon states still read: one capture each, viewed.
- Frame-time note: match scene with shadows on, this machine, Vulkan.
- Full verification recipe green.
