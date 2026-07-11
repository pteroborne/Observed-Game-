# Arc I — Light & Line (atmosphere and legibility)

Planned 2026-07-11, the same day Arc H shipped the MVP. The next horizon was
chosen with the user: before either big Post-MVP feature (LAN, world
interaction), make the game *look and sound like itself* — the liminal feel the
comfort language promised, delivered through light, shadow, and geometry. The
trigger facts: the game currently renders **zero shadows** (every place light is
an unshadowed point fill), the world-space evidence captures read nearly black
(backlog #6), the audio mix is unbalanced (#7), and the shipped style relies
entirely on palette tint + neon trim with no directional light structure at all.

## The thesis

**Atmosphere and legibility are the same problem.** A space with no directional
light is simultaneously flat (no mood) and unreadable (no structure for the eye).
One shadow-casting light per place buys both at once — and the teleport model
makes this cheap, because only one place exists at a time, so the light budget is
per-room, not per-map. The arc turns lighting from an accident of fill values
into a designed, per-district language, gated by an automated luminance check so
the "too dark to read" class of defect can never ship silently again.

## The reference set (user-chosen, 2026-07-11)

Nine media touchstones, each isolating one liminal element, each becoming one
lab diorama:

| # | Reference | Liminal element | Register |
|---|-----------|-----------------|----------|
| 1 | Japanese architecture | shadow as material — low warm light through slats, blades on the floor | dark, directional |
| 2 | Brutalist architecture | mass and weight — one hard shaft landing on monolithic concrete | dark, directional |
| 3 | Backrooms | the overlit nowhere — flat shadowless mono-yellow evenness | overlit, shadowless |
| 4 | Severance (Lumon) | the pristine institution — cold symmetric corridors someone maintains | overlit, shadowless |
| 5 | Halo (CE Forerunner) | monumental indifference — obtuse angles, glowing seams, one shaft into a huge dark volume | dark, monumental |
| 6 | BLAME! | the megastructure void — silhouettes against one faint distant light | dark, monumental |
| 7 | Silo | warm pools in buried dark — caged practicals descending into gloom | dark, rhythmic |
| 8 | Library of Babel | infinity by repetition — identical rooms visible through every threshold | repetition |
| 9 | Rudon's Plane | decaying variety — detail and color thinning with distance toward featurelessness | repetition, gradient |

The set spans four axes — brightness, scale, repetition, directionality —
so districts later become *coordinates in that space*, not copies of the
references.

## Design principles

1. **The Legibility Contract is tested against every register.** The signal kit
   (keystone, anchor torch, exit door, rival sprite) is staged in all nine
   dioramas; a register that hides a signal doesn't ship.
2. **Every aesthetic lands with a gate.** The lab's luminance-corridor check
   (captures must clear a darkness floor AND stay under a blowout ceiling) joins
   the visual audit and must provably fail on the archived near-black Phase-62
   captures.
3. **Lab before game.** All nine registers are proven as static dioramas with
   freestanding geometry (user ruling 2026-07-11) before any game code changes.
   Findings transfer as parameters (light rigs, temperatures, densities), not
   copied code.
4. **Falsifiable evidence continues.** The Arc H standing rule carries over
   unchanged: every phase ends with captures the implementing agent viewed and
   the parent session can reject.
5. **Comfort language constraints hold.** Diegetic decohere (no strobe/flash),
   accessibility toggles keep working, drained/klaxon states must still read
   under every new register.
6. **Known engine gotchas are test subjects, not surprises.** Volumetrics ×
   bloom/HDR interplay gets an explicit on/off matrix in the lab; shadow-map
   aliasing on thin slats gets tuned in the lab; dx12-vs-Vulkan capture behavior
   is already documented from Arc H.

## Stages

- **Phase 67 — Hygiene Opener: Audio Mix & Bot-POV Stall.** Backlog #7 (effects
  too loud, ambience beds too quiet — gain staging in the `AudioDirector` cue
  table) and backlog #5 (the bot-POV walkthrough freezes in Room 11 from shot
  ~55). Small, file-disjoint from the lab work.
- **Phase 68 — `lighting_lab`: Nine Liminal Dioramas.** A new lab staging the
  nine reference scenes as static dioramas (keys 1–9), freestanding geometry,
  one screenshot per scene through the standard capture path. The signal kit in
  every scene; the volumetrics × bloom matrix; slat shadow tuning; the
  luminance-corridor check prototyped against these captures; frame-time notes
  per scene on this machine (Vulkan).
- **Phase 69 — Light Staging in the Game + the Luminance Gate.** Promote the
  lab's rigs into the place spawners: one shadow-casting key light per place,
  per-district key/fill temperature pairs in `observed_style`, optional
  pools-rhythm fixture spacing. The luminance-corridor check joins the visual
  audit (proven to fail on the old dark captures). Re-capture the Phase-62-class
  evidence legibly. **Closes backlog #6.**
- **Phase 70 — District Light Identities (incl. the Overlit District).** Assign
  registers to the existing districts and add the overlit district (user-endorsed).
  Two districts must be unmistakable in captures *by light alone*; drained and
  klaxon still read under every identity.
- **Phase 71 — District Geometry Language.** The Post-MVP "Forerunner" idea,
  scoped: a geometric grammar per district (obtuse-angle wall breaks, slat/screen
  occluders, verticality accents) applied to room shells and hallway dressing,
  within the existing solvability/threshold/nav invariants — the Phase-64
  threshold-integrity gate and routing tests stay green corpus-wide.
- **Phase 72 — Arc Gate: Evidence Refresh & Playtest.** The Arc H ship-gate
  pattern, rerun: full evidence refresh from one build (all viewed), then a user
  playtest checklist — does traversal *feel* liminal, do signals read in every
  district, is the audio mix fixed. Findings land in the backlog; the arc closes
  when the checklist passes.

**Waves:** [67 ∥ 68] → [69] → [70 ∥ 71] → [72]. 67 owns audio + bot-POV capture,
68 owns the new lab crate — disjoint. 70 owns style/ambience light data, 71 owns
shell/hallway geometry — disjoint.

## Horizon

The Post-MVP Backlog retains true LAN multiplayer with dedicated servers, world
interaction, and the Arc C/D follow-ups. `bevy_mod_outline` (queued since the
asset arc "when legibility resumes") is a candidate add-on for Phase 69 or 70 if
the lab shows silhouette reinforcement is needed; otherwise it stays queued.
