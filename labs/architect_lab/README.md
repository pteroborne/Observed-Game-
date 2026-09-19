# Rogue Architect

A resettable Rogue pressure prototype on the real deterministic `HexWfcWorld`.
Reconnect and rewrite a damaged facility so autonomous Guardians can discover,
pursue, and jail both Observers. Start with **Pocket Pursuit**, then try the two
larger, two-floor hunts.

## Play in a browser

```bash
scripts/build-architect-web.sh
python3 -m http.server 8774 --bind 0.0.0.0 --directory web-dist/architect-lab
```

Open `http://localhost:8774` on the host, or the host's LAN IP and port 8774 on a
phone connected to the same network. A phone on the host’s Tailscale network can
use its Tailscale IP with the same port; the current preview is
`http://100.71.157.43:8774`. Keep the preview server running on the host. The build requires the Rust
`wasm32-unknown-unknown` target and lockfile-matched `wasm-bindgen-cli` (0.2.125).
The build script honors Cargo's configured target directory.

The browser runs the **same Rust simulation compiled to WASM**, with a DOM/SVG
presentation. It does not download Bevy's desktop renderer, require WebGPU, fetch
fonts, or send gameplay to a server. The output directory is a standalone static
bundle. Its JS, stylesheet, WASM, and illustrated asset requests share a content-derived cache key.
Use ordinary HTTP compression when deploying; the build also emits a gzip copy
of the WASM for servers that support precompressed responses.

### Illustrated tactical tiles

The browser board and hand use five ink-and-watercolor tile studies over the
actual WFC connectivity. **Tiles** opens a larger viewer with all six clockwise
rotations; opening it pauses the hunt. Rust supplies the face order, neighbor
vectors, and card masks to the browser. Artwork never decides an opening.

See [art direction and topology contract](web/art/README.md) for original prompts,
source PNGs, optimized assets, symmetry handling, and rotation verification.

### The interaction loop

1. Read the short introduction. Time begins paused.
2. Select one of the five cards, then tap a tile marked by a glowing dot.
3. Inspect its legal orientation and immediate route/instability preview.
4. Rotate if desired, then explicitly **Play this card**.
5. **Begin hunt** or **Resume hunt** to watch the consequence. **Plan** pauses time
   whenever you want to think. The five-second recharge uses simulation time.

Tap selects; one-finger drag pans; pinch zooms. A drag or pinch cannot commit a
card or accidentally become a tap. **Fit map** recenters. Both floors have named
buttons, and scenario/reset/demo controls work without a keyboard. In the browser,
`1`–`5` select cards, `R` rotates, and Space toggles planning when focus is on the
board. Returning from a background tab leaves the hunt paused.

HTML buttons, selects, live feedback, labeled keyboard-selectable SVG tiles,
visible focus, safe-area padding, and reduced-motion support are intentional parts
of the interface. Portrait supports 320px widths without horizontal scrolling;
landscape uses a scrollable command rail with sticky play controls.

## Rules now proven in the lab

- Seeded five-card deck, district matching, rotations, local attachment, refill,
  shared 300-tick cooldown, and atomic command rejection. Pocket deals only its
  own district. A no-op cannot waste a card.
- Emergency requisition refills a depleted or dead hand to exactly five instantly
  and publicly for the price of releasing exactly one major Guardian on the floor
  active Observers occupy, leaving the placement cooldown untouched. Released major
  and spawn cell selection are fully deterministic from seed and command history.
- Scenario damage is applied to a solved facility. The first repair card is offered
  from the ordinary finite deck. Missing known cells can be rebuilt through the
  same command boundary as replacements.
- Unmatched lateral **or vertical** ports warn before one exposed tile retracts
  every 180 ticks. Exposed connections into retracted cells continue the instability.
  A compatible card cancels pending retractions; removed cells require rebuilding.
- Observation, occupancy, anchors, open doors, and the prison protect against
  retraction. A completely consumed non-prison floor closes permanently and its
  district cards recycle out of the deck.
- Closed doors block movement and sight. Observers can reopen doors; open doors
  protect their connection. A rewrite/retraction consumes hosted doors.
- Matching sealed vertical faces are **not** a passage; cross-floor movement needs
  matching open ports.
- Traced deterministic Observer/Guardian/Architect selectors. Guardians detect
  along connected sightlines, investigate card targets, and patrol by visit history
  rather than oscillating between the first two neighbors. The bot evaluates known
  connectivity and detected targets, then submits ordinary legal card commands.
- Captures produce simulation events and the all-jailed outcome. Browser prey
  markers show detected Observers and explicitly marked stale sightings, never
  undiscovered current positions. Previews describe immediate topology, not promised
  future captures.
- The prison core is a vertical, non-collapsing multi-level maze at the horizontal
  center of the facility, immune to retraction and floor closure. Guardian capture
  deposits an Observer at the lowest holding cell (level 0).
- Jailed Observers remain embodied and loyal (no roster removal, no faction change).
  A jailed Observer navigates the difficult internal maze route via the bot tree's
  top-priority "escape jail" behavior, transitioning back to `Active` upon stepping
  across the threshold into the facility.
- All-jailed simultaneously yields immediate Rogue victory; while any Observer is
  active, a jailed teammate can complete their multi-step self-escape traversal.
- Unsafe falls and true-void corruption: Observers whose supporting tile is retracted
  fall downward through the vertical hex stack. If surviving structure exists on any
  lower level, the Observer lands safely. Only a fall through the entire surviving
  stack into true void corrupts the Observer into the Rogue AI faction. Corruption is
  immediate, irreversible, and public (logged to the event stream and exposed in web
  snapshots), removing the former Observer from first-person play. When every loyal
  Observer is eliminated (all are jailed or corrupted), Rogue achieves victory.

## Deep Stack and the 5-Floor Proof

`ArchitectMode::DeepStack` scales the facility to 8×6×5 (240 cells across 5 full levels,
0 to 4), damaged across five route handoffs. Running across 149 simulation beats (the
longest deterministic run in the suite), it proves several critical architectural
properties that smaller modes cannot:

1. **True 5-Floor Vertical Scale**: Verifies that the hex WFC solver, lateral/vertical
   port connectivity, and district generation scale smoothly without combinatorial blowup
   or unsolvable dead ends.
2. **Multi-Floor Vertical Prison Maze**: In 1- and 2-floor modes, the central prison core
   is largely confined to floor 0. Deep Stack exercises a multi-level vertical prison maze
   spanning floors 0 and 1, verifying that jailed Observers can solve complex 3D escape routes
   spanning multiple floor transitions to re-enter active play.
3. **Multi-Floor Routing and Asymmetric Pressure**: With 5 stacked floors, retraction
   waves and disturbance thresholds test vertical isolation, floor closure, and power
   station distribution across an expansive facility.

## Fall Reachability: Rare by Design vs. Unreachable by Construction

Backlog item #42 hypothesized that falls never land safely because facilities lacked
lower floors to catch falling Observers, proposing a deeper facility as the remedy.
Deep Stack's empirical results disproved this: in baseline runs, Deep Stack reported
**0 fall landings and 0 corruptions** — not falls that corrupted for lack of lower floors,
but a complete absence of falls.

To determine whether falls were **rare by design** or **unreachable by construction**,
we instrumented multi-seed simulation across 40 seeds (10 seeds each across Pocket,
QuickClimb, FullAscent, and DeepStack):

| Metric | Measured Total across 40 Seeds |
| :--- | :--- |
| Retractions committed | 1,087 |
| Retractions on occupied cells | **0 (0.0%)** |
| Retractions on observed cells | **0 (0.0%)** |
| Actor beats on raw candidate cells | 871 beats |
| Actor beats on telegraphed contradictions | 3,767 beats |
| Natural fall landings | 1 (blind prison exit step) |
| Corruptions | 0 |

### The Diagnosis: Unreachable by Construction

The empirical data proved that survivable falls were **unreachable by construction**
under the original rules:
1. `sim::stability::retraction_protected(&self, cell: HexCoord)` explicitly protected
   `self.occupied().contains(&cell)`.
2. Any cell containing an actor was unconditionally filtered out by `next_retraction()`.
3. Simultaneously, direct card plays onto occupied cells are rejected with
   `CommandRefusal::Occupied` (`sim.rs:398`).
4. Consequently, neither direct Architect actions nor autonomous retractions could ever
   remove the floor from beneath an actor's feet during ordinary facility play.

## The Smallest Honest Change Proposal

To make falls reachable without compromising game balance or the Legibility Contract,
we propose the following minimal, honest rule refinement:

- **Do NOT remove `CommandRefusal::Occupied`**: This refusal is load-bearing. An Architect
  must never be permitted to drop a card directly onto an actor to delete them.
- **Remove `self.occupied().contains(&cell)` from `retraction_protected`**: When an
  unmatched connection creates a contradiction adjacent to or beneath an actor, the tile
  begins telegraphing for 180 ticks (3 beats) with visible/audible hazard warning.
- **Why this does not break the Occupied rule**: The Architect does not target the actor
  directly. The actor receives 3 full behavior-tree beats (180 ticks) of advance warning.
  Under the `evade immediate danger` tree priority, the actor has ample opportunity to
  step off the compromised tile. Only if the actor is trapped, cornered, or chooses not
  to evade will the scheduled retraction commit, collapsing the floor beneath them and
  invoking `resolve_falls()`.

## 5-Floor Legibility Contract & Debug Overlay

Five vertically stacked floors on a single board present an acute legibility challenge.
To satisfy the hard **Legibility Contract** (every state visually identifiable with zero
invented colors):

1. **Distinct Architectural Registers**: Each level is bound to a canonical register
   from `observed_content::ArchitectureRegister` and styled using exact colors from
   `observed_style::architecture_tactical`:
   - **Floor 1**: `Institutional` (Slate cyan-grey) — *FOUNDATION*
   - **Floor 2**: `LiminalGrid` (Olive/gold) — *CONCOURSE*
   - **Floor 3**: `Wellshaft` (Industrial teal/amber) — *INTERIOR*
   - **Floor 4**: `FacetMonument` (Jade/emerald) — *GALLERY*
   - **Floor 5**: `Megastructure` (Deep rust/crimson) — *SUMMIT*
2. **Floor Ascent Axis & Selected Highlight**: An unbroken axis line spans continuously
   from Floor 1 to Floor 5. When a cell is targeted, its floor title plate dynamically
   highlights in amber.
3. **Actor Floor Badges & Traces**: Every Observer and Guardian carries an explicit
   `F1`..`F5` badge above their glyph. The sidebar actor roster displays exact floor
   locations for all actors (`OBS 00 [F02] ...`).
4. **Debug Overlay (`KeyO` / `OVERLAY [O]`)**:
   - **Retraction Countdown**: Displays remaining time and ticks (`RETRACT 02s (085t)`)
     with a hazard halo on the next retraction target.
   - **Fall Safety Prognosis**: Every cell on levels ≥ 1 evaluates
     `find_lower_surviving_structure` to indicate safe lower landing (`v F#`) versus fatal
     void drop (`X VOID`).
   - **Per-Floor Power**: Floor headers and generator stations display live power status
     (`[PWR:ON]` / `[PWR:OFF]` and `GEN:ON` / `GEN:OFF`).
5. **Comprehensive In-Game Legend**: An expanded sidebar key documents every floor register,
   actor glyph, tactical cell color, and debug overlay indicator.

The Pocket opening stays unresolved without a card. The deterministic bot repairs
it and wins through the ordinary command path. Larger scenarios exercise multiple
repairs and floor transitions. A paused human can inspect the same choices.

## Desktop lab

```bash
cargo dev-run -p architect_lab
```

The original Bevy diagnostic interface remains available: card keys `1`–`5`,
`Q`/`E` rotate, Space submits, `B` bot, `P` pause, `N` one behavior beat, `R` reset,
`[`/`]` scenario, `F`/Home recenter, mouse wheel zoom, and right/middle drag pan.
The browser is the primary touch interface; the desktop rail remains a debugging
view rather than the phone layout.

## Verification

```bash
cargo fmt --all
cargo dev-clippy
cargo dev-test
cargo clippy -p architect_lab --all-targets --features web -- -D warnings
cargo test -p architect_lab --no-default-features --features web --lib
cargo run -p architect_lab --no-default-features --example pressure_probe
scripts/build-architect-web.sh
# With Playwright available in Node's module search path and Chromium installed:
node scripts/verify-architect-web.cjs
node scripts/verify-architect-rotations.cjs
```

Set `ARCHITECT_URL` for a server other than `http://127.0.0.1:8774`. The browser
verification uses real touch events for pan and pinch and runs the complete
preview → play → capture → result → reset flow against the built WASM. It captures
evidence in `docs/evidence/architect_lab/mobile/`.

Rust coverage includes retraction cadence, every protection class, rebuild/repair,
floor closure, atomic rejection, vertical movement, deterministic play, and
renderer-free Bevy selection/reset lifecycle checks.

## Scope and human gate

This remains a cell-level Rogue lab. First-person movement, authored 3D hull
projection, loyal construction, continuous physical ragdoll falls, teammate rescue,
and production LAN integration belong to subsequent proofs. The native diagnostic
view can expose full actor state; the browser filters undetected prey.

Automated completion establishes that the loop works, not that it is fun. The
remaining gate is a person on a real phone predicting a useful card play and
causing an understandable capture. Mobile Chromium emulation does not substitute
for physical iOS Safari or Android testing.

![Phone move preview](../../docs/evidence/architect_lab/mobile/phone-preview.png)
