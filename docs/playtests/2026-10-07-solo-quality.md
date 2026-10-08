# Solo quality assessment — 2026-10-07

The player reports another solo playthrough: the game is visually more interesting,
but needs stronger spatial composition. They agree with the broader assessment of
core-loop proof, memorable places, integrated feedback, frame pacing and complete
release acceptance. This is human evidence, not a claim that an automated run was
played by a person. The report does not include the seed or tick.

## Reported defects

1. **Major Guardian sinks into a ramp and chases while watched.**
   [Player screenshot](../evidence/solo_quality_2026_10_07/guardian-ramp.png).
   Source inspection establishes that major movement uses cell jumps, presentation
   glides independently, and its displayed height uses the nominal storey floor.
   Observation checks the logical position rather than that independently drawn
   position. This is a confirmed architectural mismatch; the precise encounter
   remains unlocated until reproduced in a production fixture.
2. **Ceiling flicker.**
   [Player screenshot](../evidence/solo_quality_2026_10_07/ceiling-flicker.png).
   The still shows irregular overlapping surfaces. Duplicate/coplanar geometry,
   proxy/detail overlap and temporal rendering artifacts are hypotheses. A still
   cannot establish the temporal cause. Diagnose in motion before closing it.
3. **Ceiling lights hover halfway up rooms.** No dedicated screenshot supplied.
   Source inspection finds generic diffusers generated from light positions without
   mounting geometry, and a fixed-height fallback. These can disagree with actual
   enclosure height. Verify all district fixtures and multilevel rooms.

## Accepted direction

The complete eight-floor Architect Ascent, realistic procedural construction and
original PBR finishes, authored landmarks with mutable routes, current rules tuned
rather than redesigned, 20–30 minute typical successful matches, premium solo/bot
and LAN play, desktop 60 fps and Steam Deck 30 fps. Internet services are outside
this milestone. See [the implementation roadmap](../quality/aaa_bar.md).

## Acceptance boundary

Saving this report does not close the three defects. Native production evidence,
regression checks, and a human recheck of the reported situations are separate.

## Engineering follow-up — 2026-10-08

[Native evidence, diagnostics and current performance](../evidence/solo_quality_2026_10_07/README.md)
record the physical major, local observation freeze, geometry-backed fixture mounts
and coplanar render-face union. The final workspace gate is green; input compatibility
is 13. Human recheck remains pending, and the desktop timing budget remains red.
