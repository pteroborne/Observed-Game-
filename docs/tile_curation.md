# WFC tile curation — first pass

The current production budget is **331 active authored sources** after the ascent pass below.
The initial pruning pass selected 332 active sources, down from
428 (96 retired, 22%). This is a conservative working budget, not a claim that
332 is a mathematical minimum or that every retained tile meets the art bar.
Keep all ten architectural districts.

## Count connections separately from designs

| Part | Active sources | Reason |
| --- | ---: | --- |
| Stair-tower kit | 171 | One structural design expanded into required door/vertical signatures; not 171 art briefs. |
| Whole gameplay rooms | 11 | Preserve room roles, footprints and socket contracts. |
| Other cells | 150 | District architecture, horizontal connection coverage, galleries and ramps. |
| Total | 332 | Rotations and register expansion are additional runtime entries. |

The runtime also supplies generated compatibility geometry. These 332 count
**authored sources**, not every fallback prototype the game can draw. Reducing
that fallback vocabulary would be a separate topology change.

## Retire redundant art first

The production exclusion list is `assets/tiles/.tileignore`, an existing
compiler feature. Retired maps and forge builders remain available as source
references. `tilec gen-tiles` cannot silently put them back in the compiled
catalog; `tilec build` always applies the exclusion list.

| Retired group | Sources | Reason |
| --- | ---: | --- |
| Refectory, lockers, waiting, classroom, office, plant | 60 | Six furnishing programs multiplied across ten districts; all use the straight-hall connection role. Prefer the district-specific architectural suites. |
| Plain open halls | 18 | Repeated perimeter boxes already covered by retained district connections. |
| Overlit pocket B | 1 | First full-height partition blocks its advertised straight route; production seed 1 stalls with no local navigation guide. |
| Liminal layout 1 | 17 | Second decoration pass over the same topology; retain layout 0's complete connection vocabulary. |

The Liminal Grid open three-way and four-way halls stay: their exact unrotated
patterns are not covered by the retained candidates. The regression gate checks
each retired source's district, archetype, levels, footprint, rotation policy and
full port pattern against a retained source. Room modules and all 171 towers stay.
The production spectator regression exposed the pocket B obstruction; the
original catalog passed, the pruned catalog with either old or rebuilt generic
straight failed, and retiring pocket B restored both pinned seeds.
This proves vocabulary preservation, **not identical seeded layouts**: removing
weighted alternatives deliberately changes the selection lottery.

## Quality order and acceptance bar

This is a first-pass editorial assessment based on the generators and inspected
corridor renders, not a completed visual ranking of 428 modules.

1. **Generic straight hall — first replacement.** High weight, repeated across
   nine districts; oversized plain flanks and tiny decorative columns provide
   little spatial identity. The Lantern Passage replaces its geometry at the
   existing ID and port signature: three grounded portal ribs, chamfered heads,
   a lower split ceiling and a recessed axial light well. The clear lane is
   3.75 m at the ribs; the 4.5 m seam apertures remain unchanged.
2. **Generic cap and turns.** Review next for hidden decoration, weak destinations,
   repeated silhouettes and lack of a readable bend. Do not add more variants
   until one convincing design serves each route role.
3. **Retained liminal fillers.** Bring the necessary topology patterns up to the
   spatial standard of the Third Light suite, without multiplying layouts again.
4. **Tower art as one family.** Improve a shared structural recipe only after
   floor openings, stair clearances, landing paths and vertical seams are proven.

A successful replacement needs a recognizable silhouette and spatial idea,
credible support, a clear route at eye level, useful light/shadow structure,
clean joins, and physical traversal in both directions. More hulls alone are
not an improvement. “Masterpiece” remains a visual judgment; the first passage
is a review candidate, not a declaration that the library is finished.

## Evidence and preview correctness

Evidence is in [tile_curation](evidence/tile_curation/). Before and after section
captures use the same camera and Monolith register. The tile lab now gives later
explicit authored geometry precedence over earlier compatibility geometry at a
shared key. Previously it could show a generated fallback while naming the
authored tile. Scripted freelook yaw now points toward the requested target.

The catalog and LAN content hash change; the composition profile is unchanged.

## Verification outcome

Formatting and workspace Clippy pass. All workspace package tests passed across
the authoring run (216 tests) and the final remaining-workspace run; authoring
doctests also completed. Both production spectator regression seeds escape
without recovery. Placement counts and tower selection digests remain unchanged;
the horizontal selection digests intentionally changed with the candidate pool.
See [the evidence and exact commands](evidence/tile_curation/README.md).


## Ascent-first replacement pass

The new scope makes ascent a deliberate room event. Prioritize ascending modules
before more horizontal decoration. The initial ascent inventory was **eight
`hall_ramp` source designs** and **171 tower connection files representing one
structural design**. Removing tower signatures would remove legal routes; their
file count is not a measure of how often a floor should climb.

**First replacement: Processional Ascent**, at the existing `authored/hall_ramp`
ID and variant 0. A five-metre-wide, half-metre-thick flight replaces the
full-footprint wedge. Four grounded piers support it; low continuous cheeks frame
two side wells; three rising, chamfered portal frames carry practical lighting.
The door apertures, eight-metre rise, climb gradient, rotation policy, and original
five-node traversal spine remain unchanged. This is physical authored geometry,
not a screenshot-only model or a texture substitution.

Retire `hall_ramp_gallery` from production: the replacement now supplies the
legible well-and-flight idea more deliberately. Its source stays in the forge
archive. The active budget becomes **331** (97 retired), with **seven ascent
sources**. Keep the perimeter ascent, five district studies, and all 171 tower
connection files pending their own visual and traversal reviews.

### Rarity belongs to composition

The `architect_ascent_rare_circulation` composition profile lowers ramp/head/shaft
bias to 0.25 and raises the four lateral hall families together to 4.0. Raising
lateral alternatives is necessary because tower weights already hit the positive
integer floor. No legal variant is removed, and required route pins still win.
This is a relative draw bias, not an exact placement quota or an ascent-card deal
rate. The Rogue lab's logical five-card hand still has no ascent-room card.

The reproducible `ascent_curation` example generates the profile and its digest
through the authoring API. Compared with the neutral baseline:

| Deterministic sample | Ramp starts before / after | Tower cells before / after | Occupied cells before / after |
| --- | ---: | ---: | ---: |
| 16 compact facilities, 12×9×4, seeds 0–15 | 391 / 79 | 749 / 372 | 4,698 / 4,465 |
| 4 production facilities, 24×17×8, seeds 0–3 | 899 / 106 | 1,523 / 605 | 9,788 / 9,914 |

All sampled facilities solved and retained a spawn-to-exit route. The production
sample has 88% fewer ramp starts and 60% fewer tower cells. These are measured
sample results, not a guarantee for every seed; protected routes and topology
can force additional climbs. Both catalog and profile content hashes change.

The [before/after views and verification](evidence/ascent_curation/README.md)
record the first replacement. Next: review the tower family as one architectural
brief, retaining its floor apertures, landing ring, and column alignment.
