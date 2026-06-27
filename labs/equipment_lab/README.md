# Equipment Lab

The Equipment Lab proves a generic **persistent-item framework**: a logical
`EquipmentWorld` (the model lives in
[`crates/observed_interaction`](../../crates/observed_interaction), promoted from this
lab in refactor R8; the lab is its projection) owns every item by its stable
`observed_core::EquipmentId`, and rendering is a pure projection of that state.
Equipment keeps a valid location while its visual is despawned, survives its
carrier leaving the facility, and survives room replacement — it never exists
only as a temporary player ability.

This lab is also the second consumer of `observed_core::RoomId` (after
`room_lab`), which is why rooms use the shared identifier.

## Functionality evidence

![Equipment Lab showcase](../../docs/evidence/equipment_lab.png)

The authored showcase (captured via `OBSERVED2_CAPTURE`) shows, in one frame:
P1 carrying the grapple device (ownership line), a battery socketed in room A's
power socket, room A lit (the deployed light is powered), a deployed structural
jack, and a cable spool resting on the ground in the unpowered room B — with the
monitor reading `[PASS]` and `visuals: equip 5/5 player 4/4 UI 1`.

## Test objects

Portable battery, structural jack, cable spool, deployable light, grapple device.

## Operations (all pure logic, all tested)

Spawn (authored), pick up, carry, drop, hand off to another player, deploy,
connect to a socket, recover, lose power (a socketed battery drains and the room
goes dark), persist after a player leaves, and survive room replacement
(equipment relocates to the room's fallback point — the item count is invariant).

## Power model

A battery socketed in a power socket powers its room while it has charge; a
deployed light in a powered room lights up. The battery drains while sourcing and
eventually loses power, darkening the room.

## Controls

- `WASD` / arrows: move the selected player
- `E`: pick up the nearest loose item in reach
- `G`: drop the carried item
- `C`: place — socket into a compatible socket in reach, otherwise deploy
- `V`: recover the nearest socketed/deployed item
- `F`: hand the carried item to the nearest empty-handed player
- `L`: toggle the selected player "left the facility" (drops what they carry)
- `T`: replace the room the selected player is in
- `1`–`4`: select a player
- `R`: reset the showcase
- `F1`: toggle the debug overlay

## Debug visualization

- Room bounds tinted green when powered, grey when dark; a fallback marker per room
- Sockets as circles (gold = power socket, blue = tool socket) with a white inner
  dot when occupied
- A yellow ownership line from each carrier to the item it carries
- A power glow around powered equipment
- A ring on the selected player
- Monitor panel: selected player + what they carry, per-room power, battery
  charge, equipment breakdown (ground/carried/socketed/deployed), player and
  socket occupancy, live visual-vs-logical counts, and a `[PASS]`/`[FAIL]` flag

## Success conditions

1. Pick up / drop / hand off / deploy / socket / recover all behave and keep the
   item in the world.
2. A socketed battery powers its room and its light; draining it loses power.
3. A leaving player's carried item drops to the ground and persists.
4. Replacing a room relocates its equipment to the fallback without changing the
   item count, and empties that room's sockets.
5. Despawning equipment visuals does not affect logical state; the projection is
   rebuilt next frame.
6. Repeated reset restores exactly five equipment visuals, four player visuals,
   and one UI root.

## Manual verification

1. Run `cargo run -p equipment_lab`.
2. Select P2 (`2`), walk onto the cable spool in room B, press `E` to pick it up,
   walk to the empty socket, and press `C`.
3. Select P1 (`1`); press `L` and confirm P1 vanishes while the grapple device
   drops to the ground (still drawn). Press `L` again to return.
4. Walk a battery out of room A's power socket (`V`) and confirm room A goes dark
   and its light dims.
5. Stand in a room and press `T`; confirm equipment relocates to the fallback and
   the item count is unchanged.
6. Press `R` several times; the monitor must stay `[PASS]`.

## Regenerating the evidence screenshot

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/equipment_lab.png"
cargo run -p equipment_lab
```
