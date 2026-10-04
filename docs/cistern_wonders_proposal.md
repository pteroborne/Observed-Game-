# The Cistern: a Poolrooms wonder for Architect Ascent

**The location phase was approved on 2026-10-03. The hydraulic victory rule remains a proposal.**

The Cistern should be an impossibly quiet public bath inside a building that has forgotten its occupants. Cream ceramic arcades repeat across pale turquoise water. Steps descend into a basin whose far edge disappears behind columns. An elevated aqueduct crosses the room, making the eventual gameplay question visible: **who controls where this water goes?**

This is the proposed Backrooms wonder: one coherent place assembled from several hexes, recognizable from an Observer's eye level and useful in the ascent race even before it has a hydraulic objective.

## What the Poolrooms references suggest

Jared Pike's *Dream Pools*, begun in 2020, explores eerie nostalgia through seemingly endless pool environments. It is an appropriate visual reference for the familiarity and impossible scale of this room. [Artist's project and gallery](https://www.jaredpike.art/projects/dream-pools/).

Tensori describes *POOLS* as exploration driven by architecture and sound: changing echoes, no background music, and unease from getting lost or encountering dark and confined spaces. It credits Pike among its inspirations. My design inference is that the Cistern should earn its atmosphere through space, light, and sound before adding spectacle. Observed's Guardians would supply the existing threat. [Developer's description](https://store.steampowered.com/app/2663530/POOLS/?cc=jp&l=english).

The industrial reference is Japan's Metropolitan Outer Area Underground Discharge Channel. Its pressure-adjusting tank regulates water momentum and backflow; the column-filled chamber has a real hydraulic purpose. That suggests giving our aqueducts, inlet, outlet, and columns a legible relationship. [MLIT's facility description](https://www.ktr.mlit.go.jp/edogawa/edogawa00728.html).

These are references for an original composition, rather than a claim that Poolrooms has one authoritative fictional canon. The architectural and gameplay choices below are proposals.

## The room worth building

**Footprint:** three adjoining hexes on one floor, forming a broad triangular chamber. Keep within the existing storey envelope. The internal joins must be open architectural spans: crossing a hex boundary should feel like crossing the bath, with continuous flooring, water height, material scale, and lighting.

**Three spatial beats:**

1. **The arrival arcade.** A dry threshold opens beneath a low tiled arch into a much larger room. A submerged stair and a waterline ruler establish scale immediately. The far entrance is visible in fragments between columns.
2. **The main bath.** One generous basin spans the internal joins. A raised dry causeway crosses it; a longer perimeter gallery offers an alternate route. Columns interrupt observation without turning the room into a corridor maze. Repeated bays imply more building beyond the playable chamber.
3. **The service edge.** A smaller settling pool, an overhead inlet aqueduct, and a low outlet disappear through the rear wall. A dry gallery reaches both control positions. This is where the beautiful bath begins to look like machinery.

Use off-white and warm cream ceramic, restrained grout, softly lit recesses, still blue-green water, and occasional dark maintenance openings. Reflections should help sell the volume. They must not count as observation of Guardians or unexplored geometry. A plausible acoustic direction is a quiet drip at arrival, a long echo across the basin, and a localized rush at the inlet; this sound work would be a separate task if approved.

The important silhouette is **arches, horizontal water, submerged steps, and an aqueduct**, rather than three ordinary halls with a water material. Structural columns should create rhythm while preserving broad views across the composition.

Before hydraulics, the room supports existing play: a fast exposed crossing, a slower sheltered route, places to hold a Major Guardian in view, and approaches that a teammate can watch during traversal or rescue. It grants no new reflection power, damage pulse, or passive stealth bonus.

### Proposed approval boundary

The next authoring phase would deliver this **location only**: bespoke geometry, traversable dry routes, fixed decorative water, district styling, and ascent placement using the existing multi-tile composition mechanism. Inlet and outlet geometry would reserve space for future controls without pretending to be interactive.

Review it with an overhead plan, an arrival image, a basin-level image, and a first-person traversal showing all external approaches and internal joins. Acceptance means it reads as one memorable room, works at every supported rotation, and provides useful observation and traversal choices. Visual evidence must come from the game; concept art alone cannot establish those properties.

## Future Rogue victory proposal: Hydraulic Override

**The Rogue connects aqueducts to the designated Cistern reservoir and either fills it above its upper operating limit or drains it below its lower limit, then sustains that state through a live network for a visible countdown.**

The objective is a hydraulic takeover of the facility. It is an alternate Rogue victory, not drowning or direct player damage. The loyal summit race and the existing all-loyal-Observers-jailed Rogue victory would remain. Adding another victory condition requires its own later design approval and lab proof.

### Fill and drain are different network problems

| Operation | Required live route | Architectural consequence to explore later |
| --- | --- | --- |
| Fill | Powered supply → connected aqueducts → Cistern inlet | The rising water overtakes the lower crossing, concentrating movement and observation on the upper gallery. |
| Drain | Cistern outlet → connected aqueducts → discharge sink | The receding water exposes a lower service route through the basin, creating another approach to defend. |

The room always retains a dry route to its controls. These route changes are future authored states, not a promise of free-form fluid simulation. Water cannot spill arbitrarily through the WFC facility. No player should be trapped underwater or require swimming to interrupt the objective.

The Rogue's strategic choice is whether to obtain supply access and fill, or obtain discharge access and drain. Aqueduct placement uses the same known-board and mutation constraints as other architecture. A missing segment, incompatible port, shut sluice, or unavailable required pump breaks the live route. A chain that merely touches the room is insufficient.

### A concrete first lab rule

Start with **one marked reservoir, one supply, and one discharge sink** in a compact, physically explored test facility. Additional decorative pools are not targets. The room's connected main and settling basins can initially share one simulated level.

For the first experiment, use a normalized level from 0 to 1, beginning at 0.5. Proposed thresholds are below 0.1 for drain and above 0.9 for fill, followed by **60 continuous seconds** of takeover. These are test values, not balanced production numbers.

The countdown runs only while the level is beyond a limit, the appropriate aqueduct route remains connected, its controls are open, and its required pump has power. Failure of any condition resets the countdown. A severed aqueduct therefore interrupts victory immediately even though the basin still needs restoring. A later refill or drain attempt starts a fresh hold.

The prototype should show a readable waterline, inlet/outlet flow direction, and the countdown. A verified takeover must produce a match event that players can understand. The hydraulic state belongs in deterministic simulation; rendered water and reflections follow it and never decide victory.

For a later multi-reservoir variant, the target set must be fixed and marked before activation. All targets must satisfy the takeover condition together; adding decorative rooms cannot multiply required targets or create instant wins. Decide the number of reservoirs from playtest evidence rather than assuming all seven wonders should become plumbing objectives.

### Observer and loyal Architect counterplay

Observers physically operate the inlet and outlet sluices. They can close the active feed, reopen a balancing route, or contest pump power at the floor's existing generator. Manual isolation remains available without power. Their Architect can reroute a balancing aqueduct or replace an exposed segment when normal observation, occupancy, anchor, and prison protections permit it.

Looking at a room continues to protect its mutable architecture; it does not magically stop water. Guardians use their existing observation rules. This creates an active division of labor: one teammate holds a sightline while another reaches the sluice, and the Architect repairs the network they have discovered.

A global victory timer needs a public warning of the active reservoir and operation, without revealing surrounding unexplored tiles. Control access must stay physically reachable, including during a power outage. The isolated lab can guarantee that layout; production integration must prove it under the actual ascent, door, rescue, and mutation rules before enabling this victory in matches.

The central playtest question is whether the timer produces a worthwhile choice between **continue ascending** and **send someone to interrupt the takeover**. Measure time to build a live route, time to reach and isolate a sluice, and whether fill and drain offer meaningfully different tactics. A successful pipe connection alone is not enough evidence that the victory rule is fun or fair.

## One wonder for each district

Each wonder should make that district's existing play memorable. These are a shortlist for later discussion, not seven approved implementations or seven new objective systems.

| District | Wonder composition | Connection to active goals |
| --- | --- | --- |
| Backrooms / Liminal Grid | **The Cistern** — tiled baths and aqueduct arcade; three hexes | A contested crossing now; the proposed fill-or-drain Rogue objective later. |
| Library of Babel / Infinite Gallery | [**The Archive Well**](archive_well_proposal.md) — packed shelf bays and connected upper galleries around a lower reading floor | Teammates hold different views while locating the onward route; pillars and balconies contest information and rescue access. |
| Lumen / Overlit Grid | **The Switching Concourse** — an oversized, brightly lit station hall | Multiple approaches around the floor's existing generator let teams contest power while keeping sightlines readable. |
| Zen / Shadow Screen | **The Rain Court** — sheltered engawa around an open garden | Screens and offset paths reward coordinated observation during crossing and rescue. |
| Monument / Facet Monument | **The Jade Nave** — monumental piers and separated elevated landings | One Observer watches a Guardian while another crosses toward an ascent connection. |
| Reactor / Megastructure | [**The Chargeworks**](chargeworks_wonder_proposal.md) — charge factory, conveyor transfer floor and receiving vault; three hexes | Ground crossing and gantry observation now; a proposed factory-to-receiver Rogue delivery quota later. |
| Sky / Thinning | **The Last Promenade** — thin bridges approaching the summit | The existing loyal-team finish becomes a visible act of regrouping and covering the last teammate. |

The requested planning model is seven districts with one wonder per district floor. The branch currently defines an eight-floor climb with Monument repeated: see `ArchitectureRegister::CLIMB` in [observed_content](../crates/observed_content/src/lib.rs). This proposal does not change that progression. The seven-wonder set could serve a future seven-floor climb, or use two placements/variations of the Jade Nave on the current repeated district.

## Branch and draft status

This proposal lives on `codex/cistern-reimagined`, branched from `feat/architect-multi-tile-room` at `f2adea27`, in the sibling checkout `Observed-cistern-reimagined`. The source `.claude/worktrees/architect-multi-tile` checkout is unchanged.

The user approved the location phase on 2026-10-03. The authored composition and its fixed-water presentation are now implemented; see the [location evidence and verification](evidence/cistern-reimagined/README.md). No hydraulic victory feature has been implemented. Hydraulic Override requires a separate later decision.

Related project rules: [Architect Ascent design](architect_ascent_design.md), [architecture north star](../agents.md), and [tile authoring workflow](tile_authoring.md).
