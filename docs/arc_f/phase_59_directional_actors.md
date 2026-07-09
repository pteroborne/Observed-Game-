# Phase 59 — Directional Actors (rivals & guardian)

**Objective:** replace the 3-frame stand/walk placeholders with directional actor
sheets — rivals and the guardian select direction from camera-relative angle and
clip from existing presentation state, deterministically and tested. Read
[README.md](README.md) and [../../sprite_roadmap.md](../../sprite_roadmap.md)
first. Depends on Phases 57 (sheets + metadata) and 58 (slot/loading patterns).

## Read first

- `game/src/screens/place/animate.rs` — the current rival sprite animation
  (stand/walk1/walk2 cadence) and yaw-facing billboard path; direction/clip
  selection replaces this.
- `game/src/view/sprites.rs` — sprite slot consumption; this phase moves actor
  slots from single frames to sheet+metadata (Phase-57 loader + `TextureAtlasLayout`).
- Rival presentation state (pacing/operating/exiting) and guardian state
  (dormant, observed/frozen, moving, alert) — find the presentation-side
  components in `game/src/view/components.rs` / `place` systems that already
  encode these; clips key off them, never off sim internals directly.
- `crates/observed_style` — the guardian red light/halo and rival team tinting;
  these signals stay independent of the sprite art.
- Phase-57 metadata: directional counts (4/5/8-way) and semantic clips.

## Design rulings (already decided)

- **Direction from camera-relative angle**, quantized to the sheet's directional
  count; **clip from existing presentation state** — no new state machines.
  Combat-looking source frames map only to the semantic clips
  (operate/alert/disrupted/exit) or are dropped.
- **Deterministic selection:** direction/clip/frame is a pure function of
  (relative angle, state, animation clock already driven by game time) —
  unit-testable without rendering, no wall-clock, no randomness.
- **Signals stay art-independent:** guardian red light/halo, frozen/observed
  treatment, and rival team tint/legibility (Arc C rival legibility work) render
  exactly as today over the new sheets. If sprite palette fights a team tint,
  the tint wins (style is authoritative).
- **Fallback:** absent sheets → the current Kenney single-frame slots → absent
  those, procedural capsules. All three rungs must work.
- Local player/team bodies are out of scope (no player self-view need exists).

## Files you may edit

`game/src/screens/place/animate.rs`, `game/src/view/{sprites.rs, assets.rs,
components.rs}`, `crates/observed_assets/src/lib.rs` + README (actor sheet slots
`RIVAL_ACTOR`, `GUARDIAN_ACTOR` — append-only), `assets/sprites/**`,
`assets/SOURCES.md`, `game/src/tests.rs`. Do NOT edit `sim/`, bot/guardian
behavior, or `observed_style` semantics.

## Implementation

1. **Sheet slots.** Add `RIVAL_ACTOR`/`GUARDIAN_ACTOR` sheet slots (image +
   metadata reference) to `observed_assets`; game-ready sheet PNGs derived from
   the Nmn enforcer and/or knekko guard (rival) and a visually distinct LAB
   sheet (guardian) land in `assets/sprites/` with `SOURCES.md` rows.
2. **Selection function.** Pure `fn actor_frame(sheet_meta, state, rel_angle,
   clock) -> FrameRef` with exhaustive unit tests: direction quantization at
   boundaries, every presentation state maps to a clip, clip wrap determinism.
3. **Renderer wiring.** `animate.rs` computes camera-relative angle per actor
   per frame and swaps atlas indices; billboard yaw behavior preserved for the
   fallback rungs.
4. **Guardian distinctness.** Guardian sheet + treatments verified against the
   Legibility Contract: dormant vs alert vs frozen unmistakable at fog distance
   with the halo doing the semantic work.
5. **Evidence.** `docs/evidence/oga_25d_rivals.png` from a played-match capture
   showing directional rivals + guardian; refresh the bot-POV GIF and visual
   audit.

## Success criteria

- Frame selection is a tested pure function; same inputs → same frames.
- A played capture shows rivals facing correctly as the camera orbits them and
  the guardian reading its states; visual audit zero findings.
- All three fallback rungs verified (sheet → single-frame → capsule) by tests
  or a documented manual check.
- Full verification recipe green.
