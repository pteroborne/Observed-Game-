# Slice 3.6 — UX polish: affordances, feedback, and mode visibility

Written 2026-08-01 from hands-on feedback after Slice 3.5. Arc plan:
`C:\Users\comma\.claude\plans\lets-plan-out-a-swirling-kernighan.md`.

> **Decisions taken 2026-08-01:** docked never-modal panel (B); extract
> `game/src/screens/widgets/` + the chrome half of `view/theme.rs` into a shared
> `observed_ui` crate (E); split delivery into **3.6a feedback** and
> **3.6b controls**.

## The diagnosis

Six separate complaints, one cause: **the tool has no affordances.** It is a
keyboard-driven text panel wearing a 3D viewport. Every interaction is
discoverable only by reading the source or the README.

That splits into four distinct failures, which is also the order to fix them in:

| # | Failure | Symptom reported |
| --- | --- | --- |
| A | No **shading** | "interior geometry doesn't show up" |
| B | No **modal clarity** | "confused by not being able to interact while the menu was open" |
| C | No **discoverability** | "wasn't sure where to click" |
| D | No **mode visibility** | "so many modes — shift click, ctrl click" |
| E | No **affordance or consequence** | "don't know how to interact… don't know what it would do" |

What already works and must not regress: pan, zoom, the schematic legend, the
detent rotation, and the focus/layer scoping.

---

## A. Detail shading — the one rendering fix

Detail hulls currently use `unlit: true`, so every face of every hull is the
same flat colour. Silhouettes read; interiors do not.

**Fix:** lit `StandardMaterial` plus a minimal rig.

- One **directional key light**, aimed from the current view bearing rotated
  ~40 degrees off-axis. Deriving it from `detent_bearing` means contrast stays
  equivalent at all six detents instead of one detent looking flat.
- Low **ambient fill**, so a cut-open interior is shaded rather than black.
- No mesh work needed: `ConvexRenderMesh` already emits crease-preserving
  normals (`DEFAULT_SMOOTH_CREASE_COS = 0.70`), so faceted shading of the
  brush geometry comes free.
- **The schematic lines stay unlit.** They are signal, not surface; lighting
  them would make topology vary with viewing angle.

Legibility Contract: key and ambient intensities must sit inside
`observed_style`'s `ATMOSPHERE_MAX_LUMINANCE` / `SIGNAL_MIN_LUMINANCE` band, so
atmosphere never competes with the schematic overlay it sits under.

---

## B. Modality — stop blocking the viewport

The current "menu owns the keyboard" rule came from `labs/hex_tile_lab`'s
`lab_menu.rs`, which was built for a lab where you fly a character around and a
stray keypress moves you. **In a design tool that is the wrong trade.** The core
loop is *drag a value and watch the layout answer* — which is impossible if
opening the panel freezes the thing you are trying to observe.

**Fix: a docked, never-modal panel.** The viewport stays live at all times.

The rule "a key never means two things at once" survives intact — it is enforced
by **focus scope** instead of by blocking the world:

- Click the panel → arrows, typing, and Enter go to the panel.
- Click the viewport → viewport hotkeys are live.
- The focused region carries a visible outline, so which one has the keyboard is
  never a guess.

`FocusScope` / `UiInputCapture` in `game/src/screens/widgets/focus.rs` already
model exactly this, including gamepad navigation and focus restoration.

---

## C. Discoverability — hover and cursor

**Hover highlight.** Run the existing screen-space `pick()` on cursor motion,
not just on click, and draw the hovered cell's ring in a distinct schematic
role. Roughly twenty lines against machinery that already exists.

**Cursor shape.** Bevy 0.18 exposes `CursorIcon` as a component on the window
entity. Nothing in this repo uses it yet, so this is new ground, but small:

| Context | Cursor |
| --- | --- |
| over a pickable cell, paint mode | crosshair |
| over a pickable cell, inspect (shift held) | pointer |
| dragging with right/middle | grabbing |
| over the panel | default |

Cursor alone is not sufficient — it is a 32-pixel hint at the edge of attention.
It pairs with D.

---

## D. Mode visibility — a persistent action bar

The honest fix for "there are so many modes" is to **stop making the user
remember them**. A fixed block, bottom-left, that always answers *what will my
next click do, right now*:

```
MODE   paint - junction                    [,] [.] change

LMB    paint pin            SHIFT+LMB   inspect only
RMB    pan                  DEL         unpin
WHEEL  zoom                 CTRL+DEL    clear all pins
```

It updates live with held modifiers: hold shift and the `SHIFT+LMB` row
highlights while the `LMB` row dims. That converts invisible modal state into
something continuously visible, and it doubles as the keyboard legend the tool
currently only has in its README.

---

## E. Real controls — and saying what they do

`bevy_ui_widgets 0.18.1` ships headless **`button`, `checkbox`, `radio`,
`slider`, `scrollbar`, `popover`, `menu`**. The `experimental_bevy_ui_widgets`
feature is already enabled in this crate. Everything the panels need exists.

1. **Tunables become real sliders** — a track, a handle, a numeric readout, and
   mouse dragging. A row of text that changes when you press an arrow key is not
   a control; it is a log line.
2. **Every tunable gains a consequence string.** This is the direct answer to
   *"I don't know what it would do."* One line per control, in the panel, at
   rest — not hidden behind a hover:

   > `junction bias` — higher makes branching intersections more common, so
   > routes fork more and dead ends thin out.

   These live beside the existing `TunableField` table so a new field cannot
   ship without one, exactly as `every_profile_scalar_has_a_tunable_entry`
   already forces a field to have UI at all.
3. **Popover tooltips** for the longer "why would I touch this" text.
4. **Scrollbar** on the panel body, so DISTRICTS and COVERAGE stop clipping.
5. **Checkboxes** for the booleans currently hidden behind letter keys
   (cutaway, compare, walls).

---

## F. What we are *not* doing, and why

**egui.** Not on precedent alone this time — on evidence: `bevy_ui_widgets`
covers every widget class this tool needs. Adopting egui would add a second
source of visual truth for panel chrome, selection and focus, which the
Legibility Contract forbids ("presentation asks the style module how to draw a
thing; it never invents"). It has also been formally declined twice
(`docs/refactor_r11_evaluation.md`; `labs/lab_observability_lab/Cargo.toml`).
If the studio later grows docking, curve editors or rich text entry, that is the
moment to re-open it — with an R11-style evaluation, not in passing.

---

---

## Noted: promote the focus view into the in-game tac map

The detail renderer (`tools/composition_studio/src/detail.rs`) is a general
answer to "show me real authored geometry, readable from above", and the game
already has a map surface that wants exactly that: `game/src/view/map/`,
promoted from `iso_observer_lab`, currently a flat schematic.

Why it fits:

- The **cutaway classifier is presentation-only** — it reads hull extents and a
  camera bearing, and touches no simulation state. It can move without dragging
  solver code into `view/`.
- The tac map is **fog-of-war over `MapKnowledge`** (Phase 50 ruling), so it
  would render detail only for cells the player has actually seen. That is both
  the correct game rule *and* a natural performance bound — the drawn set is
  bounded by exploration, not by facility size.
- Both surfaces would then share one visual language, so the map a player reads
  and the map a designer authors against cannot drift.

Not scheduled here; this slice is the studio. Flagged so the renderer is not
buried as a tool-only detail. Whoever picks it up: keep the cutaway in a shared
place rather than copying it, or the two views will diverge the first time a
constant is retuned.

## Verification

Each item has an observable check, because "it feels nicer" is not a gate:

- **A** — capture the same focus view before/after; interior wall faces must be
  distinguishable from floor faces. Look at the PNG.
- **B** — a test that a viewport hotkey still fires while the panel is open, and
  that panel arrow-keys do *not* reach the viewport while the panel has focus.
- **C** — hovering a cell sets the hover ring and the cursor icon; leaving clears
  both.
- **D** — the action bar text changes when a modifier is held; a test asserting
  the shift-held string differs from the resting string.
- **E** — `every_tunable_field_has_a_consequence_string`, mirroring the existing
  `every_profile_scalar_has_a_tunable_entry` ratchet.
- Plus the standing gate: `cargo fmt --all && cargo dev-clippy && cargo dev-test`.

---

## Sequencing

**3.6a — feedback (no structural change).** A shading, C hover + cursor,
D action bar. These are independent of the widget question and deliver most of
the felt improvement. Roughly a third of the work.

**3.6b — controls (structural).** B docked non-modal panel, E real widgets and
consequence strings. This is where the focus-scope work and any shared-widget
extraction land.

Splitting means the viewport feels right before the larger panel refactor
starts, and 3.6a can ship even if 3.6b gets deferred behind Slice 4.
