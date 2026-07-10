# Phase 62 — Style Reconciliation (textures back under the Contract)

**Objective:** the imported albedo textures visually overpowered the style layer —
`docs/evidence/oga_25d_objects.png` shows a bright beige room with quartz-noise
walls and a busy triangulated ceiling where the neon-noir district palette should
be. Restore the rule that **`observed_style` owns what the player sees**: albedo is
an input the palette tints, UVs scale in world units, the ceiling-tile geometry
goes, and the visual audit learns to catch this class of regression forever.
Closes bug backlog #2. Read [README.md](README.md) first.

## Read first

- `docs/evidence/oga_25d_objects.png` — the regression, seen. Compare against the
  pre-Arc-F look in `docs/evidence/visual_audit/02_lighting.png` history (git show
  the pre-arc version) and the comfort-design intent: wonder → liminal dread,
  district palettes carrying mood.
- `game/src/screens/place/` shell/material build — where wall/floor/ceiling
  meshes get UVs and materials; `game/src/view/assets.rs` — how albedo textures
  and `WALL_ALBEDO_LAB` enter materials; `game/src/view/theme.rs` +
  `crates/observed_style` — the district palettes and `drained`/`klaxon` states
  that must visibly modulate every surface again.
- `game/src/evidence/audit.rs` — the visual audit that passed while the palette
  disappeared; it gains the style-presence check.
- Playtest reports (bug backlog #2): textures stretched oddly; "ceiling tiles"
  geometry doesn't work.

## Design rulings (already decided)

- **Palette over albedo, always:** every textured surface's final color =
  district palette treatment applied to the albedo (multiply/tint via the
  existing material pipeline), strong enough that two districts are
  unmistakably different in a capture and `drained`/`klaxon` still read.
  If a texture cannot sit under the tint legibly, it doesn't ship on that
  surface — the procedural material returns.
- **World-unit UVs:** wall/floor/ceiling UVs derive from world dimensions
  (texels per metre constant), so a long wall repeats instead of smearing.
- **The triangulated ceiling-tile geometry is removed** — replace with a flat
  treated ceiling (textured under tint or procedural). If any ceiling relief is
  kept, it must be subtle and palette-tinted; the playtest verdict was clear.
- **Style-presence audit check:** the visual audit gains an automated check that
  the district palette measurably dominates a captured room (e.g. mean hue/sat of
  audit captures within tolerance of the district palette, or a material-level
  assertion that every `SurfaceRole` material carries the palette tint). Design
  the cheapest check that would have failed on `oga_25d_objects.png`.

## Files you may edit

`game/src/screens/place/` (shell/material/UV build), `game/src/view/{assets.rs,
theme.rs}`, `crates/observed_style` (only if a treatment helper is missing — no
semantic changes), `game/src/evidence/audit.rs`, `game/src/tests.rs`,
`assets/textures/` (re-derive/remove offending files), `assets/SOURCES.md`.
Do NOT touch `sim/`, geometry/nav, or gameplay.

## Success criteria

- A fresh capture set shows two different districts clearly carrying their
  palettes over textured surfaces, and a drained room visibly drained. Viewed,
  per the falsifiable-evidence rule.
- No smeared textures on long walls (capture a long hallway wall).
- The ceiling reads clean in captures; the triangulated tile geometry is gone.
- The new style-presence audit check **fails against the old broken state**
  (demonstrate: run it on the pre-fix build or the archived capture — the check's
  test includes a fixture proving it would have caught the regression) and passes
  after the fix.
- Full verification recipe green; full visual audit zero findings.
