# The Archive Well — a Library wonder

The user approved research and implementation on 2026-10-04. One Architect card
places a continuous three-hex reading chamber in the Library / Infinite Gallery
district. Exploration, reporting, coordinated observation and rescue use the
existing rules; this location introduces no alternate victory condition.

## Research and inspirational references

| Reference | What it contributes | Our interpretation |
| --- | --- | --- |
| [George Peabody Library — Johns Hopkins](https://www.library.jhu.edu/library-departments/special-collections/historic-collection-at-george-peabody-library/) | Five tiers of cast-iron balconies encircle a reading room below a skylight 61 feet above the floor. | One coherent chamber, repeated shelf courses, slender metal detail, and a luminous ceiling. The playable upper circuit fits our existing eight-metre storey rather than copying the building's height. |
| [Library of Babel — Jonathan Basile's interactive project](https://www.libraryofbabel.info/browse.cgi) | Its hexagonal chambers contain four walls of bookcases with five shelves per wall. | Repetition and packed bindings make the Library recognizable; the three bays join through full-height spans so it feels like one location. This is an original composition inspired by the project and Borges, not a reconstruction of the story. |
| [Escuelas Pías — UNED's account and interior photograph](https://www.uned.es/universidad/centros/gl/madrid/bibliotecas/visitas-biblioteca-escuela-pias.html) | The former church's ruins were rehabilitated as a university library by J. I. Linazasoro. | A monumental mineral shell holds finer shelf and gallery insertions. The contrast suggests an archive that has outlived the building's original purpose. |

These architectural choices are design inferences from the references. The
linked photographs remain with their original publishers; no photographs or
external artwork are imported into the game's assets.

[Peabody interior photograph — Library of Congress](https://www.loc.gov/item/2013646465/)
· [Escuelas Pías interior photograph — UNED](https://www.uned.es/universidad/.imaging/default/dam/centros/madrid/imagenes/varias/visita-biblioteca-escuelas-pias.jpg/jcr%3Acontent.jpg)

![George Peabody Library interior — Library of Congress](https://tile.loc.gov/image-services/iiif/service%3Apnp%3Ahighsm%3A18300%3A18383/full/pct%3A25/0/default.jpg)

The Peabody reference supplies the repeated galleries and the sense of one room
held inside its shelves. [Photograph and attribution — Library of Congress](https://www.loc.gov/item/2013646465/).

![Escuelas Pías library interior — UNED](https://www.uned.es/universidad/.imaging/default/dam/centros/madrid/imagenes/varias/visita-biblioteca-escuelas-pias.jpg/jcr%3Acontent.jpg)

The Escuelas Pías reference supplies the mineral shell and inserted library
structure. [Photograph and building account — UNED](https://www.uned.es/universidad/centros/gl/madrid/bibliotecas/visitas-biblioteca-escuela-pias.html).

## Imagine

A low entrance releases into a reading well. Shelves climb the pale walls,
filled with subdued leather bindings and narrow bronze spine bands. Three
reading perches stand above the lower floor. Their bridge arms connect across
the chamber into an exposed upper circuit; from below, another person's crossing
is silhouetted against the ceiling panels. Repeated bays and shelf courses imply
an archive continuing beyond the doors.

The contrast with the Cistern and Chargeworks is deliberate: dense, quiet storage
and fine metalwork surround a generous shared interior. The well is the lower
reading floor enclosed by galleries, not a bottomless shaft through another
floor. This preserves the existing one-storey placement envelope.

## Design

- Three adjacent hexes, one card, one normal play, committed atomically.
- Six exact quantized lattice orientations; two full-height internal spans per bay.
- Three outward doorways at 120-degree intervals, maintaining ordinary attachment rules.
- A lower circuit crosses the internal joins beneath the bridges. Each bay has a
  shallow ramp to its reading perch, and the upper bridge circuit connects all three bays.
- Shelved outer walls create enclosure. The sheltered perches and exposed bridge
  arms provide different positions for observation; no balcony grants extra range.
- Fixed room-owned downlights illuminate all bays before entry and follow existing
  generator power. The moving district key is disabled within the composition.
- Authored shelves, supports, decks, ramp, desk and perch rail determine collision.
  Batched decorative bindings and ceiling panel detail follow that geometry.
- Pale mineral surfaces, checker-cut floor stone and muted bindings are
  architectural treatments. Gameplay colors keep their normal semantic meanings.

The location supports the loyal ascent goal through exploration and safe reporting
from elevated positions. A teammate can cover a crossing or watch an approach
while another moves below. The Rogue can contest its approaches and surrounding
mutable routes under the same existing rules. There are no collectible books,
archive terminals, automatic map reveals, transport powers or new victory predicates.

## Implementation and evidence

Implemented on `codex/archive-well`, continuing the approved district material
revision `4ce10549` in `Observed-cistern-reimagined`. The source
`.claude/worktrees/architect-multi-tile` checkout remains unchanged.

The approved follow-up shares the Archive's mineral tint, checker-cut floor,
panelled walls, coffered ceiling and roughness across Babel / Infinite Gallery.
Warm reading-room lighting replaces its inherited teal interior cast.
The wonder uses the district's exact cached structural materials. Its shelves,
bindings, bridges and fixed wonder lights retain their local geometry and setup.
[Ordinary Babel tile and material verification](evidence/babel-mineral/README.md).

[Game captures, authored plan, physical walkthrough and verification](evidence/archive-well/README.md)

Related: [district wonder shortlist](cistern_wonders_proposal.md),
[Architect Ascent](architect_ascent_design.md), [authoring workflow](tile_authoring.md).

## Ordinary Library dressing

The shared finish and warm interior colors are implemented. The approved first
dressing pass adds the contents and repeated bays of a library to ordinary
Babel tiles:

- Fill existing shelves with the Archive's muted bindings, broken by occasional
  gaps, tilted volumes and small stacks. Fit books to each tile's actual shelves.
- Group the shelves into tall bookcase bays with narrow uprights and bronze trim;
  retain pale mineral frames, columns and ceiling coffers.
- Fit shallow bookcase inlays to inward-facing convex wall surfaces where the
  source has only a wall and plinth or leaves wide gaps between shelf courses.
  Keep openings clear and preserve traversal.

The remaining proposals are separate authoring and lighting passes:

- Place warm fixed light on shelf faces and reading spots so collections remain
  visible before entry. Keep light budgets and powered behavior reviewable.
- Give routes different library functions: stack corridors, catalogue junctions
  and reading rooms with small desks or recessed alcoves.

Shelf contents and bookcase bays use existing ordinary geometry, share the
Archive's binding and bronze materials, and stream with their owning cells.
Reading alcoves and new collision geometry can follow as a separate authoring
pass. The Archive's three-bay room, elevated perches and continuous upper circuit
remain its distinguishing features.
[Ordinary library dressing captures and verification](evidence/babel-books/README.md).
