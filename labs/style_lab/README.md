# style_lab — the neon-noir visual language

The keystone of the **Legibility & visual-language arc** (see
[ROADMAP.md](../../ROADMAP.md) and the North Star in [agents.md](../../agents.md)).

**Primary technical question:** *can a code-only visual language (no authored
textures or meshes — just primitives + colour / emission / light / fog) render
every gameplay role distinctly, and keep gameplay-critical signals legible through
neon-noir fog and bloom?*

## What it is

A pure production crate, [`crates/observed_style`](../../crates/observed_style)
(promoted out of this lab in refactor R1), maps **semantic state → visual
treatment**:

- `SurfaceRole` (plain / spine / safe bypass / trap armed / trap idle / wall /
  ceiling)
- `MarkerRole` (next-room beacon / exit / control / collapse / you / teammate /
  rival / director) — every marker is **signal tier**
- `DoorIdentityRole` (keystone / power / reactor / control / survey / sensor /
  false exit / decoy / dead-end door reads) — every read is **signal tier** and
  glyph-backed
- `ObservedState` (frozen / unobserved / rerouting) — a modifier on a surface

A `Treatment` is data (`base_color`, HDR `emissive`, a `signal` flag, an optional
neon `edge`), not rendering. Consumers — the labs and the assembled `game` — turn
it into a `StandardMaterial` (+ edge gizmo / light). Presentation never invents
ad-hoc colours; it asks this module.

The **Legibility Contract** is encoded and tested here: every signal-tier treatment
meets a minimum emissive luminance so it punches through fog/bloom; atmosphere
surfaces stay dark; an armed trap stays legible even when unobserved; and
`legend()` enumerates every role uniquely (no unexplained marker).

## Run

```powershell
cargo run -p style_lab
```

Renders four rows in neon-noir — surfaces (middle), signal markers (front),
doorframe identity reads (front-most), and the spine surface in each observed state
(back) — with an on-screen legend.

- `R` — reset (rebuilds the scene without restarting; no leaked entities)
- `F1` — toggle the overlay panels

Capture the reference screenshot:

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/style_lab.png"; cargo run -p style_lab
```

## Success conditions

- Every `SurfaceRole`, `MarkerRole`, and `DoorIdentityRole` reads as a **distinct**
  swatch.
- Signal markers and the armed trap glow clearly through the fog; plain
  floors/walls/ceilings recede into dark atmosphere.
- The overlay shows `[PASS]` (one camera, one UI root) and the full legend.
- `R` rebuilds an identical scene; entity counts are stable across resets.

## Tests

`cargo test -p observed_style` covers the pure rules (distinctness, door-read
glyph coverage, the signal luminance floor, dark atmosphere, observed modulation,
legend completeness).
`cargo test -p style_lab` covers the Bevy lifecycle (boots with one camera +
overlay + every role rendered; reset rebuilds without leaking).
