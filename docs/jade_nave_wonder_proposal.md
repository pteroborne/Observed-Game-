# The Jade Nave — Monument wonder

A single finite Architect card places three joined bays on **Monument**, displayed
floors **5 and 6** / simulation levels **4 and 5** of the eight-floor ascent.
Six opaque jade piers frame a raised circuit, with a protected watching landing
and a real access ramp in each bay. Pale stone, bronze reveals and warm ceiling
coffers make this a heavy, inhabited monument with useful differences in height.

## Research and inspirational references

| Primary reference and photographs | Observation | Original adaptation |
| --- | --- | --- |
| [Brion Memorial — FAI](https://fondoambiente.it/luoghi/memoriale-brion), Carlo Scarpa | A monumental entrance leads into a geometric garden; an arch bridge and distinct pavilions organize the sequence. The official page includes photographs. | Carved thresholds, stepped pier crowns, bronze reveal courses and separated watching places give stone a human scale. This interpretation uses an enclosed interior. |
| [Salk Institute — official architecture guide](https://www.salk.edu/explore-salk-architecture-guide/), Louis Kahn | The courtyard between mirrored laboratory buildings gives the complex a strong shared focus. | Three entry bays look into a common meeting point. Piers interrupt those views so holding one position cannot watch every approach. |
| [Designing Fallingwater — Western Pennsylvania Conservancy](https://fallingwater.org/history/the-kaufmanns-fallingwater/designing-fallingwater/), Frank Lloyd Wright | Cantilevered concrete terraces relate to nearby rock ledges and a central stone mass. | Raised pale-stone landings and crossing slabs read as ledges among solid jade masses, rather than a second enclosed corridor. |

These architectural observations inform the design; the adaptation is original.
No photographs or imported game textures become assets. Procedural stone veins,
mineral paving and bronze finish are generated in the semantic style crate.

## Imagine

The entrance reveals tall, dark green stone faces catching amber light. A pale
crossing cuts across the room above head height. Another Observer stands on a
landing, visible between two piers, then disappears behind one as they move.
At ground level the passage remains broad; the same room offers shelter, exposure
and a choice of elevation.

Each bay has its own ramp and landing. The landings connect through exposed
crossing slabs at three metres above the storey base. A teammate can watch a
Guardian from a landing while another crosses, but must move to keep sight around
the opaque piers. No new observation or Guardian behavior is introduced.

## Design and existing rules

- One district-matching card per normal Loyal or Rogue deck containing Monument.
  The repeated district shares that card vocabulary; it does not receive duplicate
  cards merely because it appears on two floors. New IDs append after all existing
  cards and archetypes.
- Three cells commit atomically with three exterior doorways and full-height
  internal spans. Six exact lattice orientations preserve the joins.
- Floors and ceilings seal the eight-metre storey. The higher route is reached
  through three solid ramps, with no required jumping or new inter-floor aperture.
- Jade piers, their crowns, raised slabs, supporting cantilever arms and landing
  balustrades have physical collision. Fine stone finish and reveal bands remain bounded visual detail.
- Nine fixed architectural downlights and diffuse fills light the composition
  before entry. The moving district key is disabled within the Nave. Existing
  generator power dims the lights to their emergency fraction and switches the
  coffer material through cached handles.
- Existing occupancy, observation, anchor, prison, immutable-cell, attachment and
  collapse protections apply to all three cells. Protected generator rooms retain
  their original geometry and position.

The first pass establishes a location for the existing ascent and gaze mechanics.
Physical traversal checks establish reachability; competitive sightline balance
still needs multiplayer playtesting. Ordinary Monument tiles are a subsequent
art-direction decision.

## Future Rogue goal — proposal only

**The Broken Vigil:** the Rogue could try to make a Guardian reach the Nave's
central ground-level meeting point and remain unwatched there for a configurable
number of consecutive actor beats. Loyal Observers would use different landings
and reconnect their sightlines around the piers, while their Architect maintains
approaches to the room. Moving the Guardian would use existing directive cards.

Before implementation, define which Guardian counts, how the Nave meeting point
is identified, the warning shown to both sides, the grace period, and whether a
brief restored sightline resets or merely interrupts progress. Require a fair
recovery route and test the goal on both Monument floors. No goal predicate,
scoring counter, trigger volume or new victory mechanic is implemented here.

## Evidence

[Game views, actual-controller walkthrough, production CAD and verification](evidence/jade-nave/README.md).
