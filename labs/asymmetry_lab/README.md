# asymmetry_lab

Two seats, two screens, one match. The architect sees the whole lattice and
holds a hand of tiles. The operator moves the squad and sees only what the squad
has looked at.

## The technical question

*Does an asymmetric pair of roles produce a conversation worth having, or does
one seat become the game and the other its input device?*

That is the failure mode asymmetric designs die of, and it is not answerable
from the rules — only from two people playing.

## Why this is not a first-person prototype

**The asymmetry is information, not camera.** An operator is interesting because
they cannot see the system they are inside; first person is one way to arrange
that and a fogged board is another, for a hundredth of the cost. If the
request-and-serve loop fails here it would fail in first person too, and if it
works here then first person becomes a presentation question rather than a
design one.

## The two views

| | Architect | Operator |
| --- | --- | --- |
| board | whole lattice | only what the squad has seen |
| walls | as they are | **as they were last seen** |
| telegraph | every pending change | only where somebody is watching |
| hand | four tiles, one play per turn | none |
| controls | place, rotate, submit | move, turn, hold, plant, submit |

The operator's board is drawn from `Knowledge`, not from the board with a mask
over it. That distinction is the point: masking would have shown stale ground
redrawn correctly, which is precisely the mistake the design punishes. A
remembered cell is dimmed *and* hatched, because staleness is the single most
important thing an operator can misjudge and it must survive being read without
colour.

## Seats are URLs

```
http://<host>:8081/?seat=architect
http://<host>:8081/?seat=operator
```

Two devices join one match by opening different URLs. There is no lobby because
there is nothing to negotiate: the relay keys a match by name and a seat by its
query string.

```powershell
bash scripts/build-asymmetry-web.sh
python3 scripts/serve-asymmetry.py --directory web-dist/asymmetry-lab --port 8081
```

## Not wired yet

**The lab does not talk to the relay.** `scripts/serve-asymmetry.py` implements
the turn relay and it is proven end to end — two seats posting separate orders,
a turn settling only once both have spoken, and the resulting log replaying in
the sim — but the wasm client still resolves locally. `Swap seat` is a hot-seat
toggle, so one person can drive both chairs and judge the *views* before the
*conversation* is testable.

## First look

The contrast is severe. An operator opens on **5 of 37 cells known** — a small
cluster around the squad with the rest of the board simply absent. Whether that
is good tension or unplayable darkness is exactly the judgement this lab exists
to collect, and it cannot be settled from here.

One question it raises immediately: the operator's board is drawn in world
position, so a squad in one corner leaves most of the screen empty. Anchoring
the view on the squad instead would fill the screen but would cost the operator
their sense of *where* they are on a board they cannot see — which may be the
more interesting loss to keep.
