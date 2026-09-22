# Suspension Lab

Two questions, headless, with numbers at the end. Neither can be asked inside
`architect_lab`, because both need a rule the facility does not have yet.

## Question 1 — what does seeing across open air cost?

Observation currently protects a tile from retraction, and an Observer sees **two cells**:
the one they stand on, and one step forward — computed with `step_through`, which is a
*movement* function. So today you can only see where you could walk.

Open air breaks that, and it should: you can see across an atrium you cannot cross. But
protection is the Observer's real power, and a sightline that runs the length of a
concourse could ward a third of a floor. **If protection scales with sight, the Rogue
loses the ability to touch anything in view of a window.**

So the measurement is protected-cell count, old rule against new, on the same scenes. If
it explodes, then *seeing* and *warding* have to be separate ranges — which is a better
mechanic than a patch, because it gives "get line of sight" and "hold ground" different
values.

## Question 2 — does a support cascade read as drama or as a softlock?

If a tile above needs a pylon below, then retracting a pylon should drop what it carried,
and that drop may remove another support. A cascade that takes out half a tower is either
the best moment in the game or an instant unwinnable state, and the difference is a
distribution we can measure rather than an opinion.

So: remove each support in turn, and record how much comes down.

## What this lab deliberately is not

It is a **model of two rules, not the rules themselves**. `tactics_lab` drives the real
`HexWfcWorld` and says so; this one cannot, because `HexSpace` has no `Air` and the
solver has no notion of load. Borrowing the real machinery would mean changing it first,
which is the decision this lab exists to inform.

What that costs: the grid here is hand-built and rectangular, there is no WFC, no
corpus, no districts, and no doors. Nothing measured here proves the real facility
behaves the same way. It is meant to answer *whether the rules are worth having* cheaply
enough that being wrong is affordable.
