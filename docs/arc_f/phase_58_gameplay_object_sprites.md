# Phase 58 — Gameplay Object Sprites

**Objective:** OGA objects enter the *played game* before actor animation does —
keystones, access cards, the anchor torch body, route cells, and equipment become
sprite-backed semantic slots driven by existing match state. Read
[README.md](README.md) and [../../sprite_roadmap.md](../../sprite_roadmap.md)
first. Depends on Phase 57 (derived frames + metadata exist).

## Read first

- `game/src/screens/place/item_visuals.rs` — how keystones/items/devices are
  presented today (procedural meshes + the `CONTROL_DEVICE` sprite slot); this
  is the main integration site.
- `game/src/view/sprites.rs` + `game/src/view/assets.rs` — sprite slot loading,
  loaded-image gating, fallback path; extend, don't fork.
- `crates/observed_assets/src/lib.rs` — current sprite slots; the new object
  slots land here per the `sprite_roadmap.md` slot plan: `KEYSTONE_CARD`,
  `KEYSTONE_CORE`, `EXIT_ACCESS_CARD`, `ANCHOR_TORCH`, `ROUTE_CELL`,
  `RELAY_DEVICE`, `BATTERY_CHARGE`, `REPAIR_TOKEN`.
- `game/src/screens/place/factory.rs` — place build/reset lifecycle the sprites
  must respect (despawn on place reset and match exit).
- `crates/observed_style` — halo/emissive treatments for keystones and
  interactables that must persist on top of sprite art.

## Design rulings (already decided)

- **State-driven, presentation-only:** object visuals key off existing
  `keystones`/`items`/match state. No new sim state, no new gameplay.
- **Style survives the sprite:** gameplay-critical objects keep their
  `observed_style` emissive/halo treatment layered over the art — the sprite is
  the body, the style treatment is the signal. A keystone must read as a
  keystone from across a fogged room exactly as it does today.
- **Exit-unlock read stays in-world** (frame/glyph/light per the door-identity
  system) — card/core iconography may reinforce it at the object, but no HUD
  element appears (immersion ruling).
- **Fallbacks intact:** absent files → today's procedural meshes, per slot.
- Game-ready frames are copied/derived into `assets/sprites/` (single PNGs or an
  atlas) with `SOURCES.md` rows; the game does not load from `assets/oga_25d/`.

## Files you may edit

`crates/observed_assets/src/lib.rs` + README (new slots — keep additions
append-only; Track A may be editing this file in a parallel worktree),
`game/src/view/{sprites.rs, assets.rs, components.rs}`,
`game/src/screens/place/{item_visuals.rs, factory.rs, animate.rs}`,
`assets/sprites/**`, `assets/SOURCES.md`, `game/src/tests.rs`. Do NOT edit
`sim/`, `observed_style` semantics, or door/keystone rules.

## Implementation

1. **Slots.** Add the eight object slots to `observed_assets` with tests
   (manifest alignment pattern from the Phase-49 tests); document in the README.
2. **Game-ready frames.** Select frames from Phase-57 derived output (items +
   keycards sheets), place under `assets/sprites/`, ledger in `SOURCES.md`.
3. **Presentation mapping** in `item_visuals.rs`: keystone rooms show card/core
   sprites; dropped equipment uses relay/cell/device sprites; the anchor torch
   swaps its body sprite while keeping halo/light semantics; exit unlock state
   may add card/core iconography *at the exit object*.
4. **Lifecycle.** Sprites spawn/despawn with the place render signature exactly
   like current item visuals; verify the no-leak test still enumerates
   everything (no new Match resources expected).
5. **Tests + evidence:** slot alignment, fallback-when-absent, despawn-on-reset;
   refresh visual audit and capture one in-game object shot
   (`docs/evidence/oga_25d_objects.png`).

## Success criteria

- All eight slots resolve when files exist and fall back procedurally when
  deleted; both paths covered by tests.
- Keystone/interactable reads pass the visual audit unchanged (zero findings) —
  style treatment visibly layered over sprite bodies in evidence.
- Object visuals despawn on place reset and `OnExit(Match)`; no-leak test green.
- Full verification recipe green.

## As landed (2026-07-10; note written during Arc H Phase 61)

- All eight object slots (`KEYSTONE_CARD`, `KEYSTONE_CORE`, `EXIT_ACCESS_CARD`,
  `ANCHOR_TORCH`, `ROUTE_CELL`, `RELAY_DEVICE`, `BATTERY_CHARGE`, `REPAIR_TOKEN`)
  in `observed_assets` + game-ready PNGs under `assets/sprites/`, mapped in
  `place/item_visuals.rs` from existing match state, style halos layered on top.
- Tests: `test_gameplay_object_sprites_slots_and_fallbacks` covers resolve +
  procedural fallback; despawn on reset/exit under the no-leak discipline.
- Deviation: the in-game capture (`docs/evidence/oga_25d_objects.png`) exposed the
  texture/style regression (bright albedo, palette absent) — tracked as Arc H
  Phase 62; the object sprites themselves render as designed.
