# Liminal Grid Hex-Tileset Evidence

Deterministic visual checks for the production Liminal Grid expansion:

- `first_person.png` — lit player-height read of an authored 4.5 m threshold, yellow wall masses, fluorescent housing, floor, and ceiling.
- `overhead/slice_*.png` — solved level slices from the hex-WFC lab. Classic-yellow cells are explicitly labeled in the overlay as the seed-stable 7×7 Liminal Grid zone (plus any whole-room normalization across its edge).
- `colliders.png` — cross-section collider rendering of a sparse four-way junction, including its safe partitions and square supports.

Regenerate the tile views from PowerShell:

```powershell
$env:OBSERVED2_SCRIPT='docs/evidence/liminal_grid/first_person.json'; cargo dev-run -p hex_tile_lab
$env:OBSERVED2_SCRIPT='docs/evidence/liminal_grid/colliders.json'; cargo dev-run -p hex_tile_lab
```

Regenerate the overhead slices:

```powershell
$env:OBSERVED2_CAPTURE='docs/evidence/liminal_grid/overhead'; cargo dev-run -p hex_wfc_lab
```
