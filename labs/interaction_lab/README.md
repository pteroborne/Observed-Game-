# Interaction Lab

The Interaction Lab proves a reusable logical interaction state machine before
specialized gameplay depends on it. Simulation state uses stable player,
interaction, equipment, and socket IDs; Bevy entities only present that state.

## Supported operations

- Activate
- Sustained hold
- Pick up
- Carry
- Drop
- Operate
- Climb
- Shared interaction
- Exclusive interaction
- Interrupted interaction

## Fixtures

| Fixture | Purpose |
| --- | --- |
| Power lever | Instant activation and explicit state |
| Powered door | Operate action plus denied state until the lever is active |
| Timed control | Exclusive sustained hold and contention |
| Two-player control | Shared quorum and simultaneous progress |
| Portable battery | Pick up, carry, and drop |
| Equipment socket | Insert, operate, recover |
| Climb marker | Separate climb intent |

Every fixture exposes its logical state and active users in the right-hand
panel. Interaction radii and current target lines are visible when debugging is
enabled.

## Controls

Player 1:

- `WASD`: move
- `E`: interact or hold
- `Q`: climb

Player 2:

- Arrow keys: move
- Numpad `1`: interact or hold
- Numpad `2`: climb

Lab controls:

- `T`: cycle P1 and P2 through fixture scenarios
- `R`: reset every player, fixture, item, socket, event, and active interaction
- `F1`: toggle debug ranges and target lines

Players 3 and 4 use harmless deterministic movement so the lab remains
multiplayer-shaped without stealing fixture ownership.

## Success conditions

1. Activating the lever changes its state and enables the powered door.
2. Operating the door before power is denied visibly and does not open it.
3. The exclusive timed control admits one owner and denies simultaneous
   contention deterministically.
4. Releasing the hold or leaving interaction range interrupts the exclusive
   interaction and resets progress.
5. The shared control advances only while at least two players hold it.
6. Losing shared quorum interrupts progress and resets it.
7. The battery can be picked up, follows its carrier, and can be dropped.
8. The battery can be inserted into the socket, operated, and recovered.
9. Climb uses the separate climb action and visibly changes player state.
10. Prompts always state the available action, denial, ownership, or missing
    target.
11. Repeated reset preserves four player visuals, one UI root, seven fixtures,
    and no active/carry state.

## Manual verification

1. Run `cargo run -p interaction_lab`.
2. Press `T` once. Use P2 to try the door before P1 activates the lever; confirm
   the denied event. Activate the lever and operate the door again.
3. Press `T` again. Have P1 and P2 press their interaction keys together at the
   exclusive control. Confirm only one owner appears. Release before completion
   and confirm `Interrupted` with progress reset.
4. Press `T` again. Hold both interaction keys at the two-player control.
   Confirm progress advances only with both users. Release one and confirm the
   reset.
5. Press `T` again. Pick up the battery with P1, move away and press `E` to
   drop it. Pick it up again, approach the socket, insert it, operate it, and
   recover it.
6. Press `T` again and use `Q` near the climb marker.
7. Move out of range during a timed hold and confirm interruption.
8. Press `R` at least five times. The framework monitor must remain `[PASS]`
   with four players and seven objects.
