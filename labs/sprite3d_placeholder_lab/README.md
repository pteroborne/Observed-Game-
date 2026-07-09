# sprite3d_placeholder_lab

Technical question: can CC0 2.5D PNG placeholders render as readable actors/dev
devices inside the 3D neon-noir scene without making art assets part of simulation?

Run:

```powershell
cargo run -p sprite3d_placeholder_lab
```

Evidence capture:

```powershell
$env:OBSERVED2_CAPTURE = "docs/evidence/sprite3d_placeholder_lab.png"
cargo run -p sprite3d_placeholder_lab
```

The lab loads the shared `observed_assets` sprite slots from `assets/sprites/`.
Sprites spawn only after Bevy has loaded their `Image`; until then, each role uses a
style-coloured procedural fallback. Press `R` to reset the projected entities.

Source packs:

- Kenney Platformer Characters: runner, rival, guardian pose sprites.
- Kenney Sci-Fi RTS: control/deployable device sprite.

Success conditions:

- Four placeholders appear as upright 2.5D sprites once images are ready.
- The sprites yaw to face the camera while staying foot-positioned on the floor.
- Missing or unloaded images show procedural fallback meshes instead of panicking.
- Reset rebuilds the projection without leaking placeholder entities.
