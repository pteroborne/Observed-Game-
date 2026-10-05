# The Last Promenade — Sky wonder

The last of the seven district wonders is an exposed promenade for **Sky / Thinning**, floor **8** (simulation level **7**) in the current eight-floor ascent. One finite, district-matching Architect card commits three connected cells at once. It is a place to regroup before the existing summit exit.

## Research and inspirational references

The references inform spatial decisions; all game geometry and finishes are authored procedurally in the repository.

### Oslo Opera House — Snøhetta

[The architect's project and photographs](https://www.snohetta.com/projects/norwegian-national-opera-and-ballet) describe a walkable roof and plaza that make architecture into public ground. The continuous pale marble surface is the useful reference here: the promenade's walking surface should feel deliberate and inhabitable even as the building around it disappears. Our bridges and mineral landings are an interpretation, rather than a reproduction of the roof.

![Oslo Opera House marble roof; photograph Jiri Havran, via Snøhetta](https://snohetta.b-cdn.net/uploads/oa/the-norwegian-opera-ballet/20000681_N367_webimageport.jpg?crop=1318%2C1903%2C0%2C0&height=1083&quality=85&width=750)

### Blur Building — Diller Scofidio + Renfro

[The architects' project, photographs and film](https://dsrny.com/project/blur-building) describe an open platform reached across a long approach, with atmosphere replacing most of the building's visual mass. The interpretation for Sky is subtraction: remove the continuous floor and roof, keep only the path, the shelter, and the next visible landing. The reference's cloud is atmospheric inspiration; this implementation does not add fog that obscures Guardians or changes observation rules.

![Blur Building reference photograph, via Diller Scofidio + Renfro](https://cdn.sanity.io/images/q2tdbkqz/production/d19c2005df1702fed1c5c0419dbe03478b4b4a7c-2788x1850.jpg?fit=max&w=300)

The architects credit the Blur photography series to Beat Widmer, Massimo Vitali, Dirk Hebel and Jeroen Musch. Reference photographs remain hosted by their original publishers and are not copied into game assets.

## Imagine

The ascent has exhausted its architecture. What remains is a pale route crossing a hole in the facility, a line of slender portal frames, and three small shelters where teammates can wait and watch. A player sees the next landing before committing to the crossing. Looking down reveals surviving structure below; looking up reveals the sky.

The moment is cooperative: the first arrival can hold a useful sightline while the last teammate catches up. The finish still happens at the facility's existing summit exit, when the existing loyal-team quorum is met.

## Design and implementation

- **Three-cell footprint:** the same exact hex-lattice triad, with sectors turned by 0, 2 and 4 headings. Six authored orientations preserve exact physical joins.
- **Walking surfaces:** small faceted gathering platforms, two bridge arms per sector approximately 2.6 m wide, and a 4.5 m threshold approach. Entry landings meet the existing 0.5 m doorway datum; broad ramps rise to the 2.5 m bridge loop. The two-metre rise gives the gaps visible depth even when a lower roof survives beneath them.
- **Authored traversal:** each sector supplies a height-aware T-shaped guide joining both raised bridge ends to the ramp and threshold. Bots consume the same authored guide through their existing graph follower.
- **True openings:** each sector declares `floor = open`. There is no concealed full floor slab and no roof over the gathering landing. The central well and outer openings are genuine absent geometry.
- **Sheltered waiting bays:** a short canopy, a seat, and an opaque physical side blade provide a recognizable waiting position and a useful interrupted sightline.
- **Physical boundaries:** low mineral perimeter walls keep undeclared lateral faces sealed. No new up/down traversal port is declared. The open roof is an exposure to sky, not an additional ascent route.
- **Local finish:** pale chalk stone, subtle joints, brushed titanium portal details and slim warm guide lights. Palette, procedural texture, emission, roughness and lighting are owned by `observed_style::promenade`.
- **Fixed lighting:** portal and canopy sources exist before entry. Floor power changes their intensity and switches the visible light panels between cached powered/unpowered materials. The moving district key is disabled within this wonder.
- **Finite card:** `LastPromenade` and its IDs are appended after existing content. Both loyal and Rogue decks receive the Sky copy only when Sky is part of the configured climb; normal discovery, district, power, cooldown, observation, occupancy, attachment, anchor and protected-structure rules apply to the whole composition.
- **Presentation ownership:** finishes use the authoritative hulls and shallow coatings. Cells own their rendered children; identical geometry reuses cached meshes and materials.

Ordinary Sky tiles retain their current treatment. This establishes a wonder-specific visual language for a possible later district update.

## Relationship to the active goal

The Last Promenade supports the **existing loyal-team summit victory** through visible regrouping and covering teammates. It does not become an exit, auto-place beside the exit, reveal unexplored cells, move the summit, or change the loyal quorum. Its usefulness depends on the Architect building a real route from the three exterior thresholds toward the summit.

The Rogue can contest the same route with existing architecture, power and Guardians. There is no new Rogue victory condition in this implementation. Any future objective tied to holding or denying this approach would need a separate rules proposal and simulation/lab proof before implementation.

A fall uses existing physical traversal and corruption rules. Surviving geometry below can catch the body; only a fall through the surviving stack into true void corrupts. The wonder does not make every misstep an automatic Rogue conversion.

## Evidence and verification

See [the evidence record](evidence/last-promenade/README.md) for real Architect placement, powered/unpowered inspection, the production-controller tour, CAD, and the final verification results.
