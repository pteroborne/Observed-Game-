# `composition_studio`

Author the WFC composition profile and watch the facility answer.

Tiles say what the solver *may* build; the composition profile says what it
*tends* to build. Arc R Slice 0 made that profile authorable content and folded
it into the simulation content hash. This is where a person turns the knobs.

## Usage

```bash
cargo dev-run -p composition_studio
```

Headless, for evidence — describe a view in JSON and get a deterministic PNG:

```bash
cargo dev-run -p composition_studio -- --script scratch/studio/fit.json
```

```json
{
  "seed_index": 0,
  "layer": 0,
  "zoom": 1.0,
  "compare": true,
  "hide_menu": true,
  "archetype_bias": { "junction": 4.0, "corner": 0.25 },
  "output_image": "docs/evidence/composition_studio/junction_heavy.png"
}
```

Every field is optional. An unparseable script is a hard error rather than a
fallback: a capture that silently photographed the wrong view is worse than one
that did not run. (Write the file without a BOM — PowerShell 5.1's
`Out-File -Encoding utf8` adds one. The loader strips a leading BOM anyway.)

## What the viewport shows

| Colour | Meaning |
| --- | --- |
| dim green outline | the cell floor plan — the lattice itself |
| green wall band | a face you cannot pass through |
| **red** wall band | in compare mode (`B`), a cell whose placement differs from the same seed solved at the *baseline* profile — the direct answer to "what did my tuning change?" |
| amber ring | the selected cell |

Colours come from `observed_style`; a ratchet test fails the build if this crate
invents one. A second ratchet fails on non-ASCII in a rendered string, because
the tool ships no font asset and Bevy's default subset is all it can draw.

## Keys

| Key | Action |
| --- | --- |
| `F2` | open/close the tool menu. While open it **owns the keyboard**, so a key never means two things at once. |
| `Tab` / `Shift+Tab` | menu open: cycle tabs. Menu closed: cycle the drawn layer. |
| `PageUp` / `PageDown` | step the drawn layer |
| `[` / `]` | previous / next preset seed |
| `Up` / `Down` | select a tunable (TUNING tab) |
| `Left` / `Right`, `+` / `-` | move the selected tunable |
| `R` | re-solve |
| `Ctrl+R` | reload the profile from disk and re-solve, keeping zoom/pan/layer |
| `G` | show/hide wall bands (plan-only view) |
| `B` | toggle the baseline compare overlay |
| `A` | run the whole-catalog seam audit (slow; recompiles every `.map`) |
| `F` | detail: focus (selected cell + what it connects to) |
| `Shift+F` | detail: whole current layer (refused on "all layers") |
| `C` | cutaway on/off |
| `Q` / `E` | rotate the view one 60-degree detent |
| `Home` / `0` | reset zoom and pan |
| `Ctrl+S` | save to the working path (`scratch/composition/`) |
| `Ctrl+Shift+S` | promote to the corpus — asks for confirmation first |
| `Ctrl+Z` | revert to the last saved profile |
| `,` / `.` | cycle the pin brush |
| `Delete` | unpin the selected cell |
| `Ctrl+Delete` | clear every painted pin |
| right / middle drag | pan |
| scroll | zoom |
| **left drag** | **paint the active brush** |
| shift + left drag | inspect without painting |

## Two rules this tool does not soften

**It never displays a hash it did not compute.** The status line shows the
folded simulation content hash because editing a profile locks out LAN peers who
have not taken the same edit. When the compiled catalog's sidecar cannot be
read, it prints `unavailable` — a placeholder digest folds into a
plausible-looking wrong answer, which is worse than a blank.

**It never silently falls back to the baseline profile.** Slice 0 made a missing
profile a hard error precisely because a quiet fallback is what the content hash
exists to catch. If the corpus profile cannot be read the tool says so, keeps
saying so, and refuses to promote over it.

## Saving vs shipping

`Ctrl+S` writes to `scratch/composition/`, which nothing ships from. Putting a
profile into `assets/tiles/` is a separate confirmed action that names the
simulation hash before and after, because every LAN peer must take the same
change to stay joinable.

## Detail view — seeing the actual tiles

The schematic answers *topology*. It cannot answer *craft*: do these two tiles
meet, is the doorway where the solver thinks it is, does this seam line up.
`F` renders the real authored hulls instead.

A tile drawn whole from a fixed isometric shows you its ceiling and the three
walls between you and its interior, so three tests decide what survives:

1. **Floor always stays** — anything topping out at or below the slab. Cull it
   and cells appear to float.
2. **Ceiling goes** when a hull's *lowest* point is above head clearance.
   Deliberately min-Y and not the centroid: a pillar spanning floor to ceiling
   has a high centroid, and culling it would delete structure while calling it
   roofing.
3. **Near walls go** — a perimeter hull whose plan azimuth is within 90 degrees
   of the camera. At any bearing that is three of six, which falls out of hex
   geometry rather than being tuned.

`Q`/`E` rotate in **six 60-degree detents**, matching the six faces, so each
stop has one unambiguous set of near walls instead of geometry popping at
arbitrary angles. Detent 0 is the historical 45-degree camera, so every capture
taken before this feature is still framed identically.

**Two scopes.** *Focus* (`F`) draws the selected cell plus everything it shares
an open port with — the connection set, bounded at eight cells, so it costs the
same on a 5,600-cell facility as on a hundred-cell one. *Layer* (`Shift+F`)
draws the whole current layer and is refused on "all layers": one production
layer is around 18,000 hulls, which is heavy but drawable, while ten layers is
roughly 120,000, which is not a diagram. Two scopes mean no LOD system is needed.

Solid massing takes each cell's **district accent**, because this view shows the
geometry that will really be built and should look like it; the cell under
inspection takes the selection amber. The schematic keeps drawing on top, lifted
just clear of the floor slab — those lines are the *solver's* topology and the
solid is the *authored* geometry, and a disagreement between them is a real bug
worth seeing rather than hiding behind whichever drew last.

## Pins — saying "here", not "please"

Sliders bottom out at `PROFILE_MIN` (0.25), never zero, and are labelled *bias*.
A zero weight would silently degrade solvability with nothing checking. A pin is
the checked form: left-drag paints one, and the solver is **required** to honour
it rather than merely made likely to.

Pins live in the profile, so painting one is an ordinary profile edit — it marks
the profile unsaved, moves the simulation hash, and re-solves. Clearing them all
returns the profile to byte-identical baseline, so a stray empty pin set never
moves the hash for nothing.

Three things the PINS tab will tell you:

- **unsatisfiable** — a pin that can never work on this lattice (a door facing
  off the edge, a room painted onto fabric). Reported *before* the first solve
  attempt, so it names the mistake instead of surfacing as
  `RetryBudgetExhausted` a hundred attempts later.
- **conflict** — which pin, added in order, first made the facility unsolvable.
  Costs a run of solves, so it happens at authoring time only, but it answers
  the question an author actually has: *which pin do I change?* A failed solve
  with pins present leads with this instead of the retry count.
- **blueprint collision** — a stamped room landed on your pin. Room footprints
  are a frozen contract, so the blueprint wins and the pin is ignored there.
  That override is *reported* rather than silent.

One consequence worth knowing: a pin filters a cell's initial domain, which
changes the min-entropy tie-break, which changes the RNG walk. So unlike every
other part of a profile, **a pin set produces a different layout for the same
seed**. That is intended — pins are hashed content, so a layout is still
reproducible from `(seed, profile)`.

Relayout is taught about pins too: the solved world carries them and refuses a
pinned cell as a mutation site. Without that, runtime rewiring would quietly
erase authored intent mid-match.

## The COVERAGE tab

Answers "does the authored catalog cover what the solver can ask for?" *before*
a match dies at load with `MissingTile`.

It works by mirroring the projector's own selection rule — register from
`world.architecture` (except `stair_tower`, which resolves at level 0 so a whole
column agrees), then exact `(archetype, register, signature)`, then the
`generic` kit, then failure. A coverage report that reasoned differently would
confidently contradict the thing it predicts, so the panel also shows what the
*real* projector returned, and `coverage_agrees_with_the_projector` fails the
build if the two ever disagree.

What it reports:

- **projector verdict** — the authoritative "would this layout load?"
- **uncovered demands** — archetype, register, signature, cell count, and an
  example coordinate to go look at
- **generic fallbacks, heaviest first** — legal, but those districts have no
  geometry of their own there; this is the authoring to-do list
- **room variety** — authored modules per role. Note the distinction:
  `runtime_catalog` expands one module with `register_scope: ["all"]` into ten
  prototypes, so the prototype count looks like variety and is not. Every role
  in the committed corpus sits on exactly **one** authored module, which is
  `bug_backlog.md` #25 stated as a number.
- **never placed** — weight-zero prototypes (unselectable by construction,
  almost always an authoring bug) and how much of the catalog this layout skips
- **seams** (`A`) — the real `tilec audit-seams` elevation audit

## Deliberately not here

- **`search.candidates > 1`** — Slice 4. The field is shown read-only.
- **Geometry authoring** — Slice 5, and a separate binary in this crate.
- **`observed_match::hex_wfc::trim`** — written, tested, and uncalled.
  Activating it needs per-face `PortClass` on `HexStructurePiece`, which changes
  the shape of `HexWfcGeometrySnapshot`, a determinism surface with ~1100 lines
  of tests. It is left dead **on purpose**, not forgotten.
