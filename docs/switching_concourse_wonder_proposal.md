# The Switching Concourse — Lumen wonder

One Architect card creates a continuous transit hall across three adjoining hexes
on Lumen, displayed floor **3** / simulation level **2** of the eight-floor ascent.
White ceramic, exposed canopy ribs and luminous ceiling coffers make it feel like
an empty station that continues operating after its passengers disappeared.

## Research

| Primary reference | Architectural observation | Our interpretation |
| --- | --- | --- |
| [Liège-Guillemins station — Santiago Calatrava](https://calatrava.com/projects/guillemins-tgv-railway-station-liege.html) · [official photographs](https://calatrava.com/projects/guillemins-tgv-railway-station-liege.html?view_mode=gallery) | The terminal joins two sides of the city beneath a glass-and-steel vault; its canopy makes the organization of the platforms legible. | White ribs point toward a shared crossing. Repetition gives an oversized room a direction without filling its walking routes with columns. |
| [Moynihan Train Hall — SOM](https://www.som.com/projects/moynihan-train-hall/) | Four catenary skylight vaults sit above exposed original steel trusses; repeated glass-and-steel panels articulate the overhead light. | Separate the luminous coffers from their supporting ribs. Light belongs to the building and reveals its structure before an Observer arrives. |

These are inspiration, not reproductions or imported game assets. The adaptation
is an original interior within the game's sealed eight-metre storey. The canopy
has electric ceiling panels beneath a solid roof, rather than admitting sky from
the district above. The materials and geometry are generated in Rust.

## Imagine

A narrow approach opens into a room far larger than its doorway suggests. Cool
white light washes the paving. Ceiling ribs fan out overhead; three platform
edges turn toward a shared, faceted crossing. Dark inset strips and ochre edges
recall tracks and boarding positions, although no train ever arrives.

Across the hall, another doorway remains visible. A teammate can watch the open
crossing while someone slips behind a service fin beside a platform bench. The
floor is flat and continuous: the apparent tracks are surface insets, with no
unmarked pits. The furniture stays at the edges of circulation.

## Design and existing rules

- One finite, district-matching card in each Loyal or Rogue deck where Lumen exists.
  New card identities append after all existing content.
- Three sectors commit atomically, with three exterior doorways and full-height
  spans between them. Six authored lattice orientations preserve exact seams.
- Nine fixed, room-owned downlights and diffuse fills illuminate the room.
  The moving district key is disabled inside the wonder. Generator power dims
  the sources to the existing emergency fraction and extinguishes the luminous
  panel finish; it does not move the lights.
- Benches, columns, canopy ribs and service fins are authoritative collision
  hulls. Ceramic relief, markings and ceiling-panel frames are bounded visual
  detail owned by their resident cell. No alternate collision model is added.
- Existing observation, occupancy, anchor, prison, immutable-structure, attachment
  and collapse checks govern every cell of the placement.

**Generator relationship:** each floor already has exactly one protected generator
room, sited before play. A legal card cannot overwrite that cell. The Concourse is
a connecting hall whose three approaches can join routes toward the generator;
it does not guarantee placement around that room, move its fixture, or duplicate
its function. This clarifies the earlier shorthand “around the generator.”
Architects still have to build the connections and Observers operate the actual
fixture in person.

The hall also supports the existing [Darkness objective](objective_primitives.md):
when selected, it requires at least one Active Observer and no lit sightlines for
12 consecutive actor beats. Cutting power on Lumen alone does not satisfy that
facility-wide condition. Broad powered crossings help Observers maintain those
sightlines; service fins and alternate approaches give the Rogue ways to contest
the return to a generator. The existing predicate and default objective are unchanged.

## Future goal idea — proposal only

**Last Service**, a district-specific alternative to the existing Darkness goal:
the Rogue could eventually need to keep Lumen's existing
power switched off for a configurable total duration while loyal teams restore
service. The Concourse would be a visible staging and observation space along
those routes. A final design would need to resolve accumulated versus consecutive
time, recovery opportunities, and how this competes with the ascent objective.
There is no new power-control device, timer, scoring predicate or victory rule
in this location implementation.

## Implementation and evidence

[Game views, controller walkthrough, production CAD and verification](evidence/switching-concourse/README.md).
The seven-district progression remains unchanged; Monument still occupies floors
5 and 6. Ordinary Lumen tiles retain their current treatment pending a separate
art-direction pass, following the same wonder-first sequence as Babel and Zen.
