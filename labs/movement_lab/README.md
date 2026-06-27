# Movement Lab

The Movement Lab tests a deterministic 2D kinematic character controller
against authored grey-box terrain. Hardware input is translated into the shared
`PlayerIntent` type before the movement simulation runs.

No third-party physics dependency is used. Collision behavior, support
attachment, jump timing, and failure cases remain visible in the lab and
directly testable as pure logic.

## Mechanics demonstrated

- Walking and running
- Ground and air acceleration
- Ground deceleration
- Jumping and falling
- Coyote time
- Jump buffering
- Ground detection and contact normals
- Authored walkable slopes
- Authored stair step-up
- Moving-platform attachment and inherited displacement
- Out-of-bounds respawn
- Four independent movement bodies
- In-place reset without entity duplication

## Controls

- `A` / `D`: move the selected body
- `Shift`: run
- `Space`: jump
- `1`–`4`: choose which body receives human intent
- `T`: cycle the selected body through course checkpoints
- `R`: reset all bodies and the moving platform
- `F1`: toggle debug geometry and help

Bodies not selected for human control use deterministic scripted intent so
multiple movement simulations remain active.

## Course

From left to right:

1. Flat acceleration/deceleration strip
2. Walkable slope
3. Five authored stair steps
4. Upper running and jump ledge
5. Vertically moving platform over a gap
6. Right landing platform
7. Visible kill plane below the course

## Success conditions

1. Walk and run converge on distinct configured speeds.
2. Acceleration and release deceleration are gradual and predictable.
3. Jump transitions through rising, falling, and grounded states.
4. Coyote time accepts a jump immediately after leaving support.
5. A jump pressed shortly before landing fires on contact.
6. The body follows the slope and reports its contact normal.
7. Each stair is climbed through the configured authored step height.
8. A grounded body inherits moving-platform displacement and remains attached.
9. Crossing world bounds returns the body to its authored spawn.
10. Repeated reset leaves four player visuals, one UI root, neutral intent, and
    no duplicated entities.

## Manual verification

1. Run `cargo run -p movement_lab`.
2. Hold `D`, release, then repeat with `Shift`. Compare velocity and
   acceleration in the movement monitor.
3. Jump normally and watch velocity, grounded state, and contact normal.
4. Walk off the upper ledge and press Jump just after leaving it to test coyote
   time.
5. Fall toward a platform and press Jump just before contact to test buffering.
6. Traverse the slope and stairs without jumping.
7. Stand on the orange moving platform and confirm the support label remains
   `moving platform 0`.
8. Use `T` to reach the high checkpoint and fall through the gap. Confirm a
   respawn is counted.
9. Press `R` at least five times. The monitor must continue to show four bodies,
   one UI root, and `[PASS]`.
