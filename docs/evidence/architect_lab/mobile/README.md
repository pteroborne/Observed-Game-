# Rogue Architect browser proof — 2026-09-11

The browser uses the Rust lab simulation compiled to WASM, with a touch-oriented
DOM/SVG view. These images are captures of the running bundle, not concept art.

## Verification

- `cargo fmt --all`: completed.
- `cargo dev-clippy`: passed with warnings denied.
- `cargo dev-test`: 2,061 passed, zero failed, 37 ignored by the existing suite;
  zero warnings. Existing ignored tests were not silently counted as passes.
- Focused desktop + web-feature rotation suite: 29 passed, including the final
  vertical-port regression and browser adapter tests.
- Renderer-free web-feature suite: 20 passed.
- Browser flow: real touch tap → preview → play → capture → victory → reset;
  pan and two-finger pinch without accidental selection; floor switching.
- Layout captures: 320×740, 390×844, 844×390, 1440×960; no horizontal overflow.
- Native Bevy lab launched, rendered a diagnostic capture, and exited successfully.

The fixed scenario probe leaves all three hunts unresolved without a card after
180 simulated seconds. The optional bot completes Pocket in 13 seconds / 1 card,
Quick Climb in 67 seconds / 4 cards, and Full Ascent in 122 seconds / 12 cards.
The probe exercises cell-level simulation, not physical first-person traversal.

Bundle size: 989,220 bytes WASM; 199,928 bytes gzip. Compression requires
server configuration; Python's simple preview server serves the uncompressed file.
The five locally served illustrated tiles add 496,612 bytes.
No third-party font, asset, or gameplay requests are required.

The illustrated follow-up and six-direction rotation proof are documented in
[illustrated evidence](../illustrated/README.md).

## Captures

- `phone-preview.png`: a legal opening repair and its immediate consequence.
- `phone-victory.png`: the actual all-jailed result after that repair.
- `phone.png`, `small.png`, `landscape.png`, `desktop.png`: responsive layouts.
- `native-diagnostic.png`: the retained desktop diagnostic view.

## Remaining human gate

Chromium touch emulation is automated evidence. Physical iOS Safari/Android
performance, comfort, and whether a person enjoys making these choices still need
a hands-on playtest. No perceptual or fun gate is marked complete by these tests.
