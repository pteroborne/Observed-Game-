# Architect Ascent — premium quality bar

Accepted 2026-10-07. This is the implementation and acceptance ledger for the
full eight-floor game. AAA means repeatable premium player experience within the
chosen solo/bot and LAN scope, not a budget claim. Internet services are excluded.

## Assessment

Engineering is ahead of proof of the complete experience. The player confirms
visual progress and the need for stronger spatial composition. Remaining quality
work: satisfying and understandable decisions in both roles; memorable composed
places; integrated motion/light/spatial sound; frame pacing; complete human and
physical-device release acceptance. The [solo report](../playtests/2026-10-07-solo-quality.md)
preserves the supplied ceiling and Guardian screenshots and distinguishes reports,
source findings and unproven hypotheses. The wider-view benchmark's desktop
walking p95 is 17.857 ms, mutation window 48.197 ms and prison transition 71.483 ms;
see [the matched report](../playtests/2026-10-07-view-distance.md).

## Milestones, in order

- [x] **Record assessment:** preserve reports/screenshots, acceptance criteria,
  decisions and actionable defects. No quality claim closes on a proxy.
- [ ] **Trust threats and geometry:** continuous fixed-tick major movement through
  shared collision/navigation; one pose for support, sight, catch, sound and drawing;
  immediate observation freeze. Preserve directives, anchors, prison sanctuary,
  closed doors and catches. Diagnose ceiling overlap, winding, proxy replacement
  and temporal history in stationary/moving/rewriting views. Give every visible
  fixture a real ceiling/wall attachment or authored suspension; validate it.
- [ ] **Compose all eight floors:** authored spatial beats and mutable routes using
  the existing module/profile pipeline. Each floor has two distinct landmark
  compositions, readable arrival, decision space, committed traversal and reveal.
  Backrooms → Library → Lumen → Zen → Monument interiors → Monument terraces →
  Reactor → Sky. Competing exits express exposure, distance, power and rescue access.
  Initial authoring adds no permanently protected route and leaks no team knowledge.
  Cards, previews, physical geometry and maps share the same module contracts.
- [ ] **Convincing construction:** procedural geometry/original PBR, per-district
  construction dimensions, supports, junctions, mounted fixtures, material scale,
  contact shading and controlled variation. Finish one connected reference per
  district before spreading it to the kit. Review close/far, moving, powered/dark;
  the Legibility Contract remains binding. Keep geometric actors and equipment.
- [ ] **Complete match experience:** tune current rules toward 20–30 minute wins
  for players who understand their roles. Measure waiting, travel, hand stalls,
  recharge, capture/rescue and recovery separately. Contextual first actions;
  reduce persistent instructions after learning; informative placement previews;
  world/spatial feedback for freeze, tool, power, retraction, capture and rescue.
  Map discovered landmarks/rooms/height without leaking truth. Verify summit,
  Rogue win, corruption, replay, rematch, leave and subsequent clean launch.
- [ ] **Performance and release:** saved Desktop/Deck presentation presets,
  bounded streaming/caches, complete structural publication and event profiling.
  Finish controller and fresh-profile journeys, physical LAN and packaged Linux/
  Deck acceptance. Fix the known asserting extended soak before release closure.

## Compatibility and validation

Major movement state must participate in snapshots/digests and recorded replay;
movement semantics bump input compatibility. Authored fixture attachment data
travels through compilation/projection with actionable validation. Corpus/profile/
solver changes regenerate content hashes and deterministic baselines together.
Presentation presets never affect collision, sight or simulation inputs. Preserve
existing ramp/hand-motion changes and serialize Cargo builds in the shared cache.

Focused regressions cover Guardian ramps/doors/atrium/partial visibility/freeze/
anchors/catch/rewrite and both major sources; fixtures across rotations/multilevel
rooms/power/reset; ceiling stationary/turning/streaming/rewrite; LAN/replay equality.
Each code milestone runs `cargo fmt --all`, `cargo dev-clippy`, `cargo dev-test`.
Run changed ignored measurements explicitly; extended asserting failures remain red.
Documentation receives `git diff --check` and link verification.

Survey 24 fixed seeds. Humans must explain location, intended route, competing
exits and return route using landmarks. Obtain at least six complete human sessions
covering solo Observer, solo Architect, cooperative LAN, rescue, corruption and both
outcomes. Structural counts and staged screenshots cannot close these gates.

Benchmark without screenshot readback, including mutation and prison entry.
Desktop reference: p95 ≤16.67 ms, p99 ≤25 ms, no warmed game-caused event frame
above 33.33 ms. Deck 1280×800: p95 ≤33.33 ms, p99 ≤50 ms, no warmed game-caused
event frame above 66.67 ms. Report cold loading separately. Use inspected PNGs and
timestamped H.264 MP4 with seed/build/content identities; human/device acceptance
remains explicitly open until performed. Publish only after packaged gates pass.

## Implementation ledger

Assessment and evidence recorded. Implementation and its measured results are
appended here as each milestone lands; unverified claims remain open.

### Trust milestone engineering, in progress

Major motion now uses the shared flat-bottomed Rapier controller and graph follower.
The solid/visible dimensions fit 3 m doors; majors keep the former 14 m/two-second
cross-cell pace continuously. Wider corner clearance applies to majors only.
Initial placement, release and catch return resolve physical support before drawing.
Observation samples the same pose, including body sides; catches require clear sight
and actual height/proximity. Replay records feet and applies the live model scale.
Input compatibility is **13**; the Observer's v12 ramp semantics remain unchanged.

Fixtures carry geometry-backed attachment data through compilation and rotation.
Unsupported explicit surface sources fail with a location. Posts are decoration,
not new traversal obstacles. The original 151 climb compositions were rechecked
in both directions without stalls after this distinction was enforced.

The source audit identified **282/342** modules with competing overhead samples.
Coplanar structural render faces are now unioned with preserved UVs/normals; collision
hulls are unchanged. Actual mesh publication also resets temporal history. This is
a confirmed source defect, not yet proof of the exact player's flicker cause.

Saved Desktop/Deck visual presets select 180/240 m and 90/120 m detail residency
respectively. Their UI labels state frame-rate targets, not measured guarantees.
Missing saves detect known Deck hardware; manual selection remains available.

Use `python3 tools/check_quality_report.py <timings.json> --preset desktop` (or
`deck`) to compare a screenshot-free 7,200-tick production benchmark with the
budgets. It rejects incomplete/old/wrong-workload evidence. The
[prior desktop comparison](../evidence/solo_quality_2026_10_07/prior-desktop-budget.json)
is intentionally **red**: a budget comparison is not hardware or human acceptance.
Final workspace checks, native captures, performance measurements, extended soak
and the player's recheck are recorded below once performed.

### Verification ledger

The first complete engineering gate passed **2,845 tests / zero failures / 45 ignored**.
The previously failing asserting `hex_full_match_soak` then passed explicitly in
5.86 seconds and was restored to the ordinary gate. Final checks include that
assertion after the final fixture/editor changes. The source fixture supports are
rendered decoration; the physical corpus retains its original climb geometry.

The catalog hash is `6e0aa7c414b47085fc0b2e320a6476fd497407fce8a5512518ad16f7abbbde39`;
the folded simulation hash is
`9bf89cec3bcb62a997bf95e5ba78b0d5df39c4bcc29bfef1eab4a12c1295b7fe`.
The headless climb still finishes on tick **9,788**. Input 13 moves its digest from
`805826a3a1aea01f` to **`d391df1701a52246`**, with two identical runs verified before
re-pinning. Compatibility intentionally rejects earlier peers. No content/profile
fallback or assertion suppression was used.

Human rechecks of the reported situations, spatial-composition acceptance, six
complete sessions, 20–30 minute pacing and physical Deck/LAN release acceptance
remain open. Completing this engineering gate does not claim the programme's
remaining content or human milestones are complete.

### Native evidence and current budgets — 2026-10-08

[Native fixtures, walking movie and timing report](../evidence/solo_quality_2026_10_07/README.md)
are recorded separately. Both supported major fixtures are visible to the local
viewer and frozen by observation. The canonical walking sequence was inspected;
the exact player location/seed remains unknown. The screenshot-free desktop run
measures p95 **18.984 ms**, p99 **23.231 ms** and a **48.527 ms** largest mutation
window, so the desktop budget is still red. No catch occurred: prison transition
performance is not accepted by this run. Physical Deck/LAN and human gates remain open.

### Final engineering gate — 2026-10-08

`cargo fmt --all` and warning-free `cargo dev-clippy` pass. The final `cargo dev-test`
passes **2846 tests, zero failures, 44 ignored**, across 296 targets. The asserting
full-match soak is included in that ordinary gate. The separate affected production
instrument and native captures are linked above. `git diff --check`, new document
links and MP4 decoding pass. The complete extended instrumentation suite was not
run; no extended-suite acceptance is inferred.

Trust milestone: engineering and native shared-major evidence are ready; the
player’s exact-situation recheck remains open. Spatial landmarks/construction,
complete-session tuning and physical-device release milestones remain open.

### Initial spatial reference — 2026-10-08

The [Backrooms arrival/decision reference and matched native views](../evidence/spatial_reference_2026_10_08/README.md)
give the existing spawn a framed departure and body-scale check-in counter. The
existing ground-floor decision room keeps a clear long exit and introduces a
screened side threshold, revealed on approach, with different branch silhouettes
for the return. Every ordered doorway pair is traversable without jumping; a
full-size major clears the centre/departure lanes. Source and production collision
rays agree on the interruption and reveal. Fixtures retain geometry-backed mounts.

Backrooms ceiling troffers now occupy one in twelve acoustic-panel bays rather
than one in three. The construction rhythm is shared style; paths, room features
and equipment remain readable in the four inspected native views. This is the
first initial-room reference, not acceptance of a connected district or all floors.

The same report contains a 24-seed survey of the actual 9–10-room production
policy. All seeds solve, but Library has no anchored gameplay room in 5/24,
Lumen in 8/24, Monument interiors in 7/24 and Monument terraces in 5/24. Reactor
never has two. These are room-anchor counts, not landmark-quality measurements.
Next: guarantee mutable hall compositions around arrivals and choices, then test
card rewrites and human return recognition. Raising the legacy repeated-objective
quota alone would add fixed room footprints rather than meet that requirement.

This slice leaves room placement, ports, protection and knowledge unchanged.
Catalog identity becomes `b1d7b134e50ef3ccc5003ec67101546d8fbd23144f5840a780dd689316357323`;
simulation identity becomes
`675d5f9653e648e9cabc3b65a6f4dcdf09aa7393a67698075006476bb5f8cf03`.
Input stays 13, profile stays 6. The wider eight-floor and human/device gates remain open.

The final slice passes `cargo fmt --all`, warning-free `cargo dev-clippy`, and
`cargo dev-test`: **2,851 passed, zero failed, 44 ignored**, across 297 targets.
The fixed climb seed now finishes on tick **9,786**, digest **`f1a39fdfa7834108`**,
with independent runs verified before updating the baseline. The charge fixture
requires a collision-clear minor body rather than a ray-clear point. The affected
7,200-tick simulation instrument remains steady at p95 **345.752 µs**, excluding
command generation/rendering; it recorded no catch. Final native views retain the
same body-height cameras and final content identity. The extended instrumentation
suite was not run. No new graphics or device acceptance is inferred.

### Mutable Library/Lumen compositions — 2026-10-08

[Native references and implementation record](../evidence/initial_halls_2026_10_08/README.md)
add a connected three-cell canopied gallery and a distinct three-cell tall court
to each target floor. Selection follows the solved floor arrival/departure and
uses compatible ordinary hall interfaces. The court begins at a junction and
is separated from the gallery. All 24 survey seeds contain four compositions,
twelve selected hall cells, and the original room/door topology.

Initial choices are revision-scoped physical content. They add no protected
cells or automatic team discovery, and unrelated cell changes do not reroll them.
A legal card rebuild retires its choice; same-door rebuilding is meaningful once
because it replaces the physical kit, then the ordinary no-op refusal resumes.
Previews, physical/rules revisions and incremental/fresh projection agree.
All module doorway pairs and rotations are traversable by Observers and full-size
majors without jumping. The initial kits retain their authored enclosure rather
than losing it to ordinary outdoor wall trimming.

Forty reserved source maps provide complete flat-hall kits in the two districts,
without entering the ordinary card/relayout lottery. The archive is 478 sources,
382 active. Profile compatibility is **7**, with hashed initial hall composition
enabled; input compatibility stays **13**. Catalog identity is
`2bd68f6ac106342c842c5c4cb67c55385a559b2ab6c37fb9fd2f4715971d0d81`;
simulation identity is `15efcbcef864cfa37fb9ffa23238c526dc8ca7f482b61e2141295332e2094d89`.

Human recognition, remaining districts, complete-match pacing and graphical/device
acceptance remain open. These references do not close the all-eight-floor milestone.

Final engineering gate: formatting and warning-free Clippy pass; workspace tests
pass **2,860 / zero failures / 44 ignored**, across 297 targets. The separate
production simulation measurement reports p95 **349.531 µs** over 7,200 ticks,
excluding command generation/rendering, with no catch. The compact climb baseline
is unchanged. Four supported native reference views were inspected. The complete
extended instrumentation suite and fresh graphics/device acceptance remain unrun.

### Mutable Zen and Monument compositions — 2026-10-08

[Native references and implementation record](../evidence/zen_monument_2026_10_08/README.md)
extend the initial gallery/court system through Zen, Monument interiors and
Monument terraces. Zen introduces wall-backed screen rails and low garden ledges;
interiors use heavier ledges and courses. Separate terrace sources use parapets,
pier-supported canopies and courts without an authored roof cap. Sources and
controllers validate every doorway pairing and rotation for Observers and majors.

The 24 fixed seeds retain their solved topology and receive ten compositions,
thirty mutable hall choices across floors 1–5. Same-door card rebuilding, preview
retirement, revision consistency and incremental/fresh geometry checks pass.
Sixty maps bring the archive to 538 sources, 442 active. Profile compatibility is
**8**; input remains **13**. Simulation identity is
`9922a1750e5c27dd861c3e77035c096a492fa6dd4ef16f5325f71ef16e149adb`.

Ten supported body-height native captures were inspected. They make the lower
canopy, perimeter screens and heavier courses visible, but do not yet establish
strong landmark recognition. The repeated shared surface treatment still dominates
many views. Terrace courts can read as covered because a neighbouring upper-floor
slab remains overhead; the missing authored cap alone does not provide a cleared
skyline. Ordinary-hall exposed undersides remain visible through openings. These
are recorded construction/art gaps, not accepted AAA completion.

Reactor and Sky references remain next. Human match pacing, all-floor recognition,
performance budgets, Deck and physical LAN acceptance remain open.

Final engineering gate: formatting and warning-free Clippy pass; workspace tests
pass **2,860 / zero failures / 44 ignored**, across 297 targets. The separate
7,200-tick simulation probe reports p95 **336.362 µs**, excluding commands/rendering,
with five plays and no catch. The complete extended instrumentation suite and
fresh graphical/device acceptance remain unrun.

### Streaming versus true void — 2026-10-08

[Matched normal/full-world native references](../evidence/visibility_reference_2026_10_08/README.md)
correct the earlier provisional interpretation of the Zen court's apparent openness.
Its missing nearby walls were deferred paper/cedar meshes: presentation hid the
base walls before all replacement finishes were ready. The base shell now stays
opaque until the complete finish is ready. A cold-recipe regression checks the
partial-cache case.

Initial compositions and ordinary flat halls now derive exterior proxy silhouettes
from their authoritative hulls. Door openings, enclosed walls, canopies and terrace
roof gaps no longer become generic exterior bands. Complex-module proxies and
simpler far shading remain separate limits. The same camera poses with all 2,272
geometry owners shown confirm that the large Monument terrace opening remains;
ceiling artifacts also remain, so neither should be classified solely as streaming.

The diagnostic adds per-cell geometry/visibility reports and a capture-only full
residency mode. It changes no collision, observation, topology or compatibility.
The eight-floor source profile remains version 8; simulation identity is unchanged.
Human motion/flicker and device/frame-budget acceptance remain open.

The fresh screenshot-free 7,200-tick Ascent frame run completes without GPU queries:
warm p95 **19.045 ms**, p99 **23.737 ms**, maximum **49.375 ms**, cold view construction
**7.014 s**, with a **365.866 ms** early frame outside the warm sample. Desktop
remains red; more faithful proxy geometry does not close the
frame or cold-start budgets. A separate GPU-instrumented attempt stalled after a
swap-chain timeout and was stopped without a report; per-pass GPU timings are
unavailable for the completed retry. This is not a matched before/after comparison.

Final engineering gate: formatting and warning-free Clippy pass; workspace tests
pass **2,863 / zero failures / 44 ignored**, across 297 targets. The paired
cameras share an identical complete physical-cell census and content identity.
The full extended instrumentation suite and human/device checks remain unrun.
