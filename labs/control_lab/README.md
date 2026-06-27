# Control Lab

The Control Lab is an independently runnable prototype for the player-control
boundary. Four stable player IDs drive four probe entities through
`PlayerIntent`; probe behavior never reads keyboard or gamepad state directly.

## What it demonstrates

- Two simultaneous keyboard layouts
- Native Bevy gamepad input
- Explicit player-to-device assignment
- Stable `PlayerId` values independent of ECS entities
- Abstract movement, look, and action intent
- Runtime jump-key rebinding
- Neutral input and frozen playback while the window is unfocused
- Per-player input recording and looping playback
- Switching any probe between human, scripted, and playback control
- Reset to a known baseline without restarting or duplicating entities

## Controls

- `1`–`4`: select a player
- `Tab`: switch selected player between human and scripted control
- `C`: cycle the selected player's human device
- `F5`: start/stop recording the selected player
- `F6`: play the selected player's recording
- `F7`: rebind Jump for the selected player's keyboard slot, then press a key
- `F8`: reset the lab
- Controller `Start`: assign that controller to the selected player

Default keyboard layouts:

- Keyboard A: move `WASD`, look `IJKL`, jump `Space`, sprint `Shift`,
  interact `E`, climb `Q`
- Keyboard B: move arrows, look numpad `8456`, jump numpad `0`, sprint right
  control, interact numpad `1`, climb numpad `2`

## Success conditions

1. All four probes exist and expose independent intent.
2. Keyboard A and B can drive separate players simultaneously.
3. Pressing Start on a connected controller assigns it to the selected player;
   a controller is never assigned to two players.
4. Rebinding Jump changes the generated edge-triggered intent.
5. Losing window focus immediately produces neutral intent and does not advance
   playback.
6. A recording replays the same intent frames in order.
7. Human/scripted switching changes the source without replacing the player.
8. Repeated reset leaves exactly four probes, one UI root, empty recordings, and
   default assignments.

## Manual verification

1. Run `cargo run -p control_lab`.
2. Move Player 1 with `WASD` and Player 2 with the arrow keys. Confirm their
   intent vectors and probes respond independently.
3. Select Player 1, press `F7`, then bind Jump to an unused key. Press it and
   confirm the Jump indicator flashes.
4. Hold a movement key and switch away from the window. Confirm the focus
   indicator changes and the intent returns to zero.
5. Select a human player, press `F5`, perform a short input sequence, then press
   `F5` again. Press `F6` and confirm the sequence loops.
6. Press `Tab` to switch the selected player to Scripted and back to Human.
7. If a controller is available, select a player and press controller Start.
   Confirm the device label changes and the sticks/buttons drive intent.
8. Press `F8` at least five times. Confirm the debug footer continues to report
   `4 probes`, `1 UI root`, and `PASS`.
