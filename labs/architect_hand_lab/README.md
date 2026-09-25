# Architect Hand Lab

A focused UI/UX instrument for the Architect's tile-card loop. It uses the
real radius-three board and `observed_mechanics::architect::inspect`, but stops
before turns, opponents, rogue mutation, or networking.

## Run

```bash
cargo run -p architect_hand_lab
```

Tap a card, tap a glowing hex, rotate beside the preview, then explicitly
`PLACE`. Dragging a card onto the board reaches the same preview. Keyboard:
`1`–`4` select, `Q`/`E` rotate, `Enter` places, `Esc` cancels, `Z` undoes, and
`[`/`]` changes the task.

The fifth task is free play. Trial counts remain local and `COPY` only
copies the one-line summary to the browser clipboard.

## Browser build

```bash
CARGO_TARGET_DIR=/srv/build-cache/asymmetry-target ./scripts/build-architect-hand-web.sh
./scripts/serve-architect-hand.py --directory web-dist/architect-hand-lab --port 8082
```

## Evidence

```bash
OBSERVED2_CAPTURE=docs/evidence/architect_hand_portrait.png \
OBSERVED2_CAPTURE_SIZE=375x812 cargo run -p architect_hand_lab
```
