# Hex Room Lab

**Hex Room Lab** showcases multi-cell hex room composition and district lighting across the 9 architecture registers in Observed 2.

---

## Core Features & Functionality

- **Multi-Cell Blueprints**: Renders 5 multi-cell room roles (`DecoherenceFork`, `Decision`, `DualStation`, `AnchorCheckpoint`, `GuardianControl`) constructed from per-cell authored tile prototypes.
- **Dollhouse Cutaway**: Automatically drops top ceiling slabs so interior geometry and district key lighting can be inspected during camera rotation.
- **Turntable Evidence Capture**: Supports full 360° turntable screenshot capture for all 9 architecture registers.

---

## Controls

- `1`–`9`: Select architecture register
- `Tab`: Cycle room blueprint role
- Auto-orbiting 3D camera

---

## Run & Evidence Capture

```powershell
cargo run -p hex_room_lab
$env:OBSERVED2_CAPTURE = "docs/evidence/hex_room_lab"
cargo run -p hex_room_lab
```
