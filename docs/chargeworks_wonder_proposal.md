# The Chargeworks: a Reactor wonder

The user approved the location composition on 2026-10-04. The cargo objective below is a future proposal. One Architect card will place three adjoining hexes: a fabrication bay, a conveyor transfer floor, and a receiving vault.

## Research

The closest match to the remembered Halo containers is Halo 2's Forerunner gas canister: an enclosed blue gas payload transported through Threshold's gas mine. That suggests a readable source, transport path, and destination, with the dangerous contents visible inside a protective frame. [Canister reference](https://www.halopedia.org/Forerunner_gas_canister), [gas-mine reference](https://www.halopedia.org/Gas_mine).

Bungie's Elongation preview describes two conveyor corridors, ramps and upper catwalks, and crates that change cover and access. Tyson Green's account of sneaking along Colossus conveyors is especially useful: machinery can provide a route through a room as well as its industrial identity. [Bungie, “Longing for Elongation,” June 9, 2005, archived developer article](https://bnetarchive.haloman30.com/news/content6683-2.html?cid=5278&type=topnews).

Our design inference is to borrow that relationship between machinery, routes and elevation. The Chargeworks has original geometry and equipment silhouettes. It does not reproduce a Halo map or import its assets.

For the material pass, the TSC:E artist's first-hand postmortem is a useful account of early Halo's layered gray surfaces, dark blue support panels, inset texture strips and recessed light. This is a community map artist's analysis, rather than an official Bungie specification. We interpret those relationships as original chamfered metal plates with stepped inlays and directional electrical interference across the belts. [Siliconmaster, “Forerunner Art,” 2015](https://tsce.info/postmortem-2015-silicon.html).

## Imagine

An abandoned charge factory still waits for the next production run. An oversized angular press opens onto a low conveyor. Caged translucent cyan samples sit beside the line. Across the chamber, an overhead track and a service gantry mark the transfer floor. The belt bends into a tall receiving mouth, surrounded by armour, storage saddles and a crown that almost reaches the ceiling.

The three spaces share layered gray metal, chamfered plates, dark blue recessed ribs, amber route markings and fixed cool work lights. Cyan field surfaces shimmer along the belt direction; the original machinery remains grounded in large, simple shapes. The source is squat and heavy; the transfer is open and crossed by a gantry; the destination is tall and recessed. Machinery should be recognizable from a doorway, without a HUD explanation.

## Design the card

**District:** Reactor / Megastructure. **Footprint:** three adjoining hexes on one floor, placed atomically. **Rotation:** six exact lattice orientations. Internal joins are full-height spans; three external doorways face out at 120-degree intervals.

The exposed conveyor crossing is the shortest ground route. Side aisles let Observers move around stationary machinery. A shallow ramp reaches the transfer gantry, giving a teammate an elevated view across the line and a separate position for observation. The receiving mouth gives the room a strong destination while preserving circulation around its sides.

Placement uses the existing discovered-board, district, observation, occupancy, anchor, prison and local attachment rules. The location consumes one card and one normal play. Its three sectors enter the physical facility together. It should also appear as the complete composition in the card preview and placement ghost.

This approved version contains **static belts, machinery and sample canisters**, with animated decorative electric fields. The shimmer conveys the machinery's visual direction; it does not move bodies or cargo or count deliveries. It adds no manufacturing, cargo motion, explosions, damage, switches, delivery counter or alternate victory rule. Work lights belong to the room and illuminate it before the player arrives. The existing generator controls their power and dims field emission to a legible emergency minimum.

Acceptance evidence: authored overhead plan; a real Architect-card placement; eye-level views of all three roles; a physical walkthrough through the joins and up the gantry; all six rotations checked for collision and containment.

## Future Rogue objective: Critical Delivery

The Rogue produces charged canisters at a powered factory, builds a continuous compatible conveyor route, and delivers **X distinct canisters to one designated receiving room**. Begin a later isolated lab with **X = 6**, one factory and one public receiver. This is a test value, not a production balance decision. Factory and objective receiver must occupy separate marked sites. The local receiving vault in this showcase is storage; its built-in belt cannot satisfy the remote delivery quota. A later lab would need real transport ports and separate source, path and destination cards.

Production has a finite rate and requires floor power. Cargo advances deterministically through directed compatible ports. A missing segment, closed gate, incompatible join or unpowered required drive pauses that route. Canisters have stable IDs; reception consumes each ID once, so loops, duplication and rewriting the same container cannot advance the quota twice. A card cannot conjure an already delivered payload.

A first lab should test a visible overload hold after the quota, rather than immediate victory. Observers can physically isolate the receiver, divert a junction, or contest generator power; their Architect can rebuild exposed routes under the normal mutation rules. Isolation must remain possible during a power outage. Public quota and takeover warnings identify the target without revealing the unexplored network.

The explosive Halo reference supplies visual tension. Observed's canon forbids direct player harm, so any later failure or overload must affect machinery and architecture rather than introduce combat damage. Decide whether containers can vent, jam or be diverted only after the basic delivery loop is fun.

Factory production, transport networks, receiver controls, payload state and victory detection all belong in deterministic simulation. Rendering follows that state. They require separate design approval and lab proof before production integration.

Measure time to assemble a route, time for an Observer to interrupt it, how often the summit race is abandoned, and whether multiple route choices create meaningful cooperation. A working conveyor alone does not prove a worthwhile alternate victory.

Related rules: [Architect Ascent](architect_ascent_design.md), [architecture reference](../agents.md), [tile authoring](tile_authoring.md). This is the next Reactor wonder candidate after the Cistern's seven-district shortlist.

## Implementation and evidence

Implemented on `codex/chargeworks-wonder`, branched from the approved Cistern lighting revision `e3b070b1`. The original `.claude/worktrees/architect-multi-tile` checkout remains unchanged.

The location ships as one finite-deck wonder card for Reactor in both loyal and Rogue decks. Loyal dealing still unlocks by reached district. Eighteen forge sources provide the three roles at six grid-fitted orientations; the complete card preview and placement ghost use their authored geometry. Canisters and conveyors are stationary; translucent decorative gas leaves obstruction to the authored cage. The generator dims and restores fixed work lights.

See [game captures, authored plan and verification](evidence/chargeworks/README.md). This evidence establishes placement and local traversal; the proposed delivery game still needs its own later lab.

The folded-metal material has also been promoted to the shared Reactor district
finish. Ordinary Reactor structural surfaces and the Chargeworks share the same
cached materials, while its electric conveyors remain location-specific.
See [district material evidence](evidence/reactor-metal/README.md).
