# Menu Lab

The Menu Lab is a focused lifecycle prototype. It deliberately uses a tiny
technical playfield rather than game content so state transitions and cleanup
remain easy to inspect.

## Success conditions

1. Boot reaches the main menu.
2. Settings and Controls return to the main menu without duplicate UI.
3. Start reaches Loading and then Gameplay.
4. Pause preserves the same gameplay session.
5. Reset replaces the gameplay session without increasing its entity count.
6. Return to Main Menu removes every gameplay-owned entity and the session
   resource.
7. Repeating the cycle keeps all diagnostics green.

## Manual verification

Run `cargo run -p menu_lab`, then:

1. Visit Settings, toggle both options, and return.
2. Visit Controls and return.
3. Start the lab and allow Loading to finish.
4. Press `Escape` twice to pause and resume. Confirm the session generation does
   not change.
5. Press `R`. Confirm generation increases by one and the owned entity count
   remains at its expected value.
6. Pause and choose **Return to Main Menu**. Confirm `SESSION 0 / 0` and
   `RESOURCE absent` in the diagnostics.
7. Repeat steps 3–6 at least three times. `SCREEN ROOTS`, `SESSION`, and
   `LAST CLEANUP` should remain `PASS`.

The automated lifecycle tests repeat the same transition cycle ten times.
