# OpenGameArt 2.5D Sprite Roadmap (rewritten 2026-07-09)

Source collection: <https://opengameart.org/content/25d-fps-cc0>

Goal: replace the current narrow 2.5D dev placeholders with a complete,
manifest-driven 2.5D presentation layer — actors, keystones, equipment, room
objects, decorations, and selected textures — while preserving the
simulation/presentation split and the Legibility Contract.

**This document is the sprite design reference.** Execution is staged as
Arc F Phases 57–60 with one sub-agent hand-off doc per phase in
[docs/arc_f/](docs/arc_f/README.md). Read this file for *what and why*; read the
phase docs for *who edits which files and when*.

## Status ledger

| Original phase | State | Where it landed / lives now |
| --- | --- | --- |
| 1 — Raw intake & provenance | **DONE** (2026-07-09) | `assets/oga_25d/raw/` + rows in `assets/SOURCES.md`; CC-BY sound libraries flagged reference-only |
| — Dev placeholder pass | **DONE** (2026-07-09) | Kenney sprite slots (`RUNNER_*`, `RIVAL_*`, `GUARDIAN_STAND`, `CONTROL_DEVICE`) in `observed_assets`, consumed by `game/src/view/sprites.rs` + `place/item_visuals.rs`, proven in `sprite3d_placeholder_lab` |
| 2 — Metadata & atlas pipeline | **DONE** | [Arc F Phase 57](docs/arc_f/phase_57_sprite_pipeline_lab.md) |
| 3 — Expanded lab | **DONE** | [Arc F Phase 57](docs/arc_f/phase_57_sprite_pipeline_lab.md) |
| 4 — Gameplay objects | **DONE** | [Arc F Phase 58](docs/arc_f/phase_58_gameplay_object_sprites.md) |
| 5 — Directional actors | **DONE** | [Arc F Phase 59](docs/arc_f/phase_59_directional_actors.md) |
| 6 — Room dressing | **DONE** (2026-07-10) | [Arc F Phase 60](docs/arc_f/phase_60_dressing_textures_reticle.md) |
| 7 — Textures / HUD / reticle / audio | **DONE** (2026-07-10) | Textures + reticle landed in Phase 60; HUD iconography cut; Audio moved to Phase 56 |
| 8 — Evidence, tests, docs | **DONE** | Folded into each phase's criteria |

## Core constraints (binding)

- **Simulation stays asset-free.** No `sim/` module may depend on sprite names,
  Bevy handles, atlases, materials, or presentation components. The
  `game/src/arch_check.rs` ratchets enforce the direction; never fix the test.
- **`observed_assets` is the single source of truth for drop-in slots.** New
  sprite/texture slots are named manifest entries there — present → used,
  absent → procedural fallback. Game code never hard-codes sprite paths.
- **`observed_style` owns semantics.** Color, emission, contrast, and every
  gameplay-critical signal treatment come from the style module. Imported art
  never replaces the Legibility Contract: doors, keystones, rivals, guardian
  states, and hazards must still punch through fog/bloom regardless of sprite.
- **HUD-free play is a standing ruling (Phase 50, 2026-07-09).** Normal play has
  no HUD; status readouts are debug-only (`OBSERVED2_DEBUG_HUD`). Therefore this
  roadmap ships **no HUD iconography for normal play**. Icon use is limited to:
  the debug HUD, the post-match summary, and the fog-of-war tac-map sketch —
  and only where those surfaces already exist.
- **Raw stays raw.** `assets/oga_25d/raw/` is untouched downloads;
  `assets/oga_25d/derived/` holds cropped frames, atlases, and generated
  metadata; only game-ready files enter `assets/sprites/` / `assets/textures/`.
- **Derivation is deterministic.** Any crop/pack script is rerunnable and
  byte-stable for the same inputs, so derived assets can be regenerated and
  diffed instead of trusted.
- **CC0 only in game-ready paths.** The two Little Robot Sound Factory
  libraries (CC-BY 3.0) stay reference-only; the audio track replaces them with
  CC0 sources rather than adding attribution machinery.

## Source set (all downloaded, see `assets/SOURCES.md` for provenance)

| Source | Intended use |
| --- | --- |
| `LAB sprites` (mutantleg) | Broad object/actor test sheet for the atlas pipeline; guardian-candidate monsters. |
| `LAB textures` (mutantleg) | Optional wall/floor/ceiling albedo variants for lab-like rooms. |
| `Items - armor, health, ammo` (Nmn) | Repurpose as batteries, charges, route cells, and equipment tokens. |
| `Key/Credit Cards` (XCVG) | Keystones, access credentials, exit unlock tokens. |
| `Decorations - torches, trees, some corpses` (Nmn) | Room dressing and animated props; use selectively for tone. |
| `Oldschool FPS decoration sprites` (knekko) | Additional non-blocking props. |
| `Human guard for Sprite Based FPS` (Nmn) | Directional rival/team actor. |
| `The Guard` (knekko) | Alternate actor sheet with walk/hit/death frames; remap non-combat states as operate, alert, disrupted, exit. |
| `64 crosshairs pack` (para) + Cursor Pack | Interaction reticle / menu cursor, only if a reticle read is warranted (Phase 60 decides). |
| `FPS Weapons Overlay` (knekko) | Only if repurposed as non-combat first-person tools (scanner, relay placer, observation device). Optional. |

## Semantic slot plan

Object slots (Phase 58): `KEYSTONE_CARD`, `KEYSTONE_CORE`, `EXIT_ACCESS_CARD`,
`ANCHOR_TORCH`, `ROUTE_CELL`, `RELAY_DEVICE`, `BATTERY_CHARGE`, `REPAIR_TOKEN`.

Actor sheets (Phase 59): `RIVAL_ACTOR`, `GUARDIAN_ACTOR` — directional sheets
with semantic clips, replacing the current 3-frame stand/walk slots.

Dressing/texture slots (Phase 60): per-`RoomRole` prop sets and optional
`WALL_ALBEDO_*` / `FLOOR_ALBEDO_*` variants under style tint.

## Frame metadata spec (Phase 57 deliverable)

One checked-in metadata file per derived sheet in `assets/oga_25d/derived/`,
machine-readable (RON or JSON), declaring:

- frame rectangles (validated in-bounds by unit test)
- pivot point and pixels-per-metre
- directional count: billboard, 4-way, 5-way, 8-way, 16-way
- semantic clips: `idle`, `walk`, `operate`, `alert`, `disrupted`, `exit`,
  `pickup` — combat frames in source sheets are remapped to these or dropped
- default material treatment role (which `observed_style` treatment applies)

Prefer Bevy `TextureAtlasLayout` for regular grids; standalone derived PNGs
only for irregular sheets or one-off icons.

## Presentation rules

- **Objects** are driven by existing `keystones`/`items`/match state, despawn on
  place reset and match exit, and keep their `observed_style` emissive/halo
  treatment on top of the sprite art.
- **Actors** select direction from camera-relative angle and clip from existing
  presentation state (idle, pacing, operating, exiting; guardian dormant,
  observed/frozen, moving, alert). Frame selection is deterministic and tested.
  The guardian's red light/halo signal stays independent of the sprite art.
- **Dressing** is role-driven, never random-global: monitor rooms get consoles
  and screens; guardian control gets desks and warning devices; reactor/foundry
  get barrels and hazard lights; archive/hollow stay sparse; corridors get only
  low-risk dressing. No prop may block or visually cover a threshold; no prop
  may share a gameplay signal color unless it *is* gameplay-critical; decorative
  sprites render dimmer than interactables and rivals.
- **Textures** are optional albedo inputs; `observed_style` tint/emission apply
  on top and semantics never come from the texture.
- **Fallbacks always work.** Every slot preserves today's procedural fallback;
  deleting `assets/oga_25d/` must leave a fully playable, legible game.

## Non-goals

- No combat mechanics, however shooter-themed the source art is.
- No texture-heavy clutter that trades neon-noir readability for realism.
- No bulk import: only sheets with a mapped semantic role enter derived/game paths.
- No generalized asset framework beyond what these sheets and mappings prove.
- No HUD iconography for normal play (immersion ruling above).
- No audio work in this roadmap — the audio track (Arc F Phases 55–56) owns it.
