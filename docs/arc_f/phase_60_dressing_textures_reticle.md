# Phase 60 — Room Dressing, Textures & Interaction Read

**Objective:** the atmosphere payoff — role-driven room props, optional LAB
texture variants, and a decision on the interaction reticle — landing the sprite
layer's tone without ever costing a read. This closes Track B and Arc F. Read
[README.md](README.md) and [../../sprite_roadmap.md](../../sprite_roadmap.md)
(presentation rules) first. Depends on Phases 57–59.

## Read first

- `game/src/screens/place/factory.rs` — how rooms/halls are dressed today
  (models: crates, consoles, hazard props; fixtures); sprite dressing joins this
  path.
- `game/src/view/theme.rs` + `crates/observed_style` — district palettes, drained/
  klaxon states; dressing must recolor with the room, not fight it.
- Room roles (`RoomRole`) and the dressing rules table in `sprite_roadmap.md`
  (monitor/control/reactor/archive/corridor mappings; the three hard rules).
- `assets/oga_25d/derived/` decoration sheets (Nmn decorations, knekko oldschool
  set) and LAB textures from Phase 57.
- `docs/evidence/visual_audit/` — the contrast/legibility audit is the gate for
  every change in this phase.

## Design rulings (already decided)

- **Role-driven, seeded, deterministic:** prop selection/placement per room is a
  pure function of room identity + seed (`observed_core::SplitMix`), so the same
  room dresses the same way across peers/replays and never shifts on re-render.
- **The three hard rules:** no prop blocks or visually covers a threshold; no
  prop shares a gameplay signal color unless it is gameplay-critical; decorative
  sprites render dimmer than interactables and rivals.
- **Textures are albedo only,** behind optional `observed_assets` slots, with
  `observed_style` tint/emission on top; semantics never come from the texture.
  Start with one or two room roles (LAB-flavored monitor/archive), not global.
- **Reticle decision (make it, don't assume it):** normal play is HUD-free. If
  the game's interaction read needs help, prefer a *diegetic* cue (object glow /
  cursor-light) over a screen-space crosshair. A crosshair asset may be used
  only if it is minimal, only-when-interactable, and ruled consistent with the
  immersion ruling — document the ruling taken in this doc's "As landed" note.
- Tone stays liminal wonder→dread ([[comfort-design-language]]): sparse beats
  cluttered; corpse-type decorations are off-tone — skip them.

## Files you may edit

`game/src/screens/place/{factory.rs, item_visuals.rs}`, `game/src/view/{sprites.rs,
assets.rs, theme.rs, components.rs}`, `crates/observed_assets/src/lib.rs` + README
(dressing/texture slots — append-only), `assets/sprites/**`, `assets/textures/**`,
`assets/SOURCES.md`, `game/src/tests.rs`, `sprite_roadmap.md` (status ledger
tick). Do NOT edit `sim/`, room generation, or nav/collision behavior — dressing
is strictly non-blocking visuals placed clear of walk paths.

## Implementation

1. **Dressing slots + placement.** Decoration sprite slots per the slot plan;
   a seeded placement function per room role honoring the hard rules (threshold
   clearance from the room's `DoorGap` data; dim material treatment).
2. **Role passes.** Monitor: consoles/screens; guardian control: desks/warning
   devices; reactor/foundry: barrels/hazard lights; archive/hollow: sparse;
   corridors: minimal, never near thresholds or traversal lines.
3. **Texture variants.** Optional albedo slots for one/two roles; verify drained/
   klaxon palette transitions still read (palette drives mood even over albedo).
4. **Interaction read.** Evaluate current interact affordance; implement the
   diegetic cue or minimal reticle per the ruling; legend-back any new signal in
   the debug HUD legend.
5. **Evidence + audit.** Refresh the full visual audit set and bot-POV GIF; one
   before/after dressing comparison shot per dressed role
   (`docs/evidence/oga_25d_dressing_*.png`).

## Success criteria

- Visual audit passes with zero findings with dressing and textures live —
  doorway, threshold, keystone, rival, and guardian reads unambiguous in every
  audit shot.
- Dressing is deterministic across two runs of the same seed (test on the
  placement function) and despawns cleanly on reset/exit.
- No prop intersects a threshold clearance zone (unit test on placement output).
- Reticle/interaction ruling documented in the "As landed" note.
- `sprite_roadmap.md` status ledger fully ticked; full verification recipe
  green; Arc F evidence set complete under `docs/evidence/`.
