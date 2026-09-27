# Kinetic Tool Lab

Candidates for the Observer's kinetic tool. The current tool in `kinetic_lab` and
`wfc_kinetic_lab` is a placeholder, two scaled cubes. Three designs stand side by side
on turntables, and the selected one is held in a first-person hand at the size the game
holds the observation torch.

```powershell
cargo dev-run -p kinetic_tool_lab
$env:OBSERVED2_CAPTURE = "docs/evidence/kinetic_tool_lab"; cargo dev-run -p kinetic_tool_lab
```

![The three candidates](../../docs/evidence/kinetic_tool_lab/lineup.png)

The designs live in the shared `observed_tool` crate, which carries their tests and is
built from the Guardians' own shape kit (`observed_guardian`). The chosen design goes
into the game unchanged.

## The candidates

- **Coil**, kin to the Guardians. A barrel of hex tiers that turn against each other like
  the Tumbler's, snap into line on a push or a lash, and spin hard on a pull. A lidded
  eye at the muzzle looks the way the lash will send things. The facility's own
  machinery, turned in the Observer's hand.
- **Plumb**, a surveyor's instrument. A banded body, a spirit vial whose bubble runs to
  the high end, and a gimballed plumb bob at the muzzle that hangs along the armed
  direction: the lash's new down, shown literally. It reads as measuring, not as a
  weapon, which suits a tool that damages nothing.
- **Lance**, a working emitter. A long hex barrel with three prongs that open to push
  and close to pull, and a dial on its flank whose needle stands at the armed pitch.
  The plainest to read at a glance, and the most like a gun.

## Why every design shows the armed pitch

The armed lash turns with the Observer (see `wfc_kinetic_lab`): it always drives the way
you face, at the pitch you armed. That pitch is otherwise invisible, so each design
carries it on the tool: the Coil's pupil, the Plumb's bob, the Lance's needle.

![Armed steeply down: the bob hangs, the needle dips, the pupil drops](../../docs/evidence/kinetic_tool_lab/lineup-armed-down.png)

## Held

| Coil | Plumb | Lance |
|---|---|---|
| ![](../../docs/evidence/kinetic_tool_lab/held-coil.png) | ![](../../docs/evidence/kinetic_tool_lab/held-plumb.png) | ![](../../docs/evidence/kinetic_tool_lab/held-lance.png) |

## Controls

| Input | Action |
|---|---|
| 1 / 2 | Lineup / held |
| Tab | Next design (held) |
| Space / E / F | Push / pull / lash |
| Up / Down | Armed pitch |
| A | Automatic cycle on or off |
| P | Pause |
