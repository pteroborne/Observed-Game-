# Lighting Lab

**Lighting Lab** showcases the 9 production architecture-register dioramas (Arc I: Light & Line) under `observed_style`.

---

## Core Features & Functionality

- **9 Production Architecture Dioramas**: Evaluates light, fog, bloom, and legibility across all 9 architecture registers (`ShadowScreen`, `Monolith`, `OverlitGrid`, `Institutional`, `FacetMonument`, `Megastructure`, `Wellshaft`, `InfiniteGallery`, `Thinning`).
- **Legibility Contract Verification**: Evaluates the signal kit (objective, anchor, rival, exit frame) against the luminance corridor (`luminance.rs`).
- **Capture Loop & Verification**: Automated screenshot grading matrix over volumetrics and bloom toggles.

---

## Controls

- `1`–`9`: Select diorama scene
- `V`: Toggle volumetric fog
- `B`: Toggle bloom
- `R`: Respawn scene
- `F1`: Toggle debug overlay

---

## Run & Evidence Capture

```powershell
cargo run -p lighting_lab
$env:OBSERVED2_CAPTURE = "docs/evidence/lighting_lab"
cargo run -p lighting_lab
```
