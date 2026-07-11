# Phase 70 — District Light Identities (incl. the Overlit District)

**Objective:** give each district a light identity — a coordinate on the lab's
four axes (brightness, scale, repetition, directionality) — and add the
user-endorsed **overlit district** (the Backrooms/Severance register). Two
districts must be unmistakable in captures *by light alone*. Read
[README.md](README.md) first.

## Read first

- `docs/light_and_line_arc_plan.md` — the reference table and the four axes;
  districts are coordinates, not copies.
- Phase 68/69 as-landed notes — the proven registers and the game-side rig.
- `crates/observed_style` — district palettes and the new light data from
  Phase 69.
- `game/src/screens/match_runtime/ambience.rs` — `district_for_place`, how a
  place learns its district, drained/klaxon transforms.
- The comfort design language (docs/decisions or memory): district palettes were
  already a comfort deliverable; light identity extends that, it does not
  replace it.

## Design rulings (already decided)

- **Register assignments are proposed by the implementing agent from the lab
  captures and confirmed by the parent session before landing** — with one
  fixed: exactly one district is the **overlit** register (flat, shadowless,
  even, slightly wrong; high ambient + emissive panels; deliberately no shadow
  caster). The user chose this contrast deliberately: it makes the shadowed
  districts darker by comparison.
- **Every register keeps the Contract:** the signal kit reads in every district;
  the luminance corridor (Phase 69's gate) passes for every district capture —
  the overlit district exercises the ceiling side of the corridor.
- **Drained and klaxon must read under every identity** — including overlit,
  where "drained" cannot rely on darkening alone (the lab's scene-3 findings
  guide what draining an overlit space looks like — e.g. panels dying to
  sparse).
- **Identity is data:** the register parameters live in `observed_style`
  district data; the spawner reads them. No per-district code branches in the
  spawner beyond reading the data.

## Files you may edit

`crates/observed_style` (district light data), `game/src/screens/place/lighting.rs`
(reading new data only), `game/src/screens/match_runtime/ambience.rs`,
`game/src/tests.rs`. Do NOT touch geometry, `sim/`, or the audit machinery
(Phase 69 finished it).

## Success criteria

- One capture per district (same room archetype, same camera) — a contact sheet
  where every district is identifiable by light alone; **viewed**, differences
  described in the as-landed note.
- The overlit district's capture passes the luminance ceiling; the darkest
  district's capture passes the floor.
- Drained + klaxon captures for the overlit district and one dark district,
  viewed.
- Signal-kit legibility spot-check capture in the overlit and darkest districts.
- Full verification recipe green; visual audit (incl. luminance corridor) green.
