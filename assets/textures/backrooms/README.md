# Backrooms materials

Nine original 2048 × 2048 PNGs, generated deterministically by
[`tools/generate_backrooms_materials.py`](../../../tools/generate_backrooms_materials.py).
They use no photographs, downloaded textures, or external source material.

| Finish | Albedo / ORM repeat | Normal repeat | Character |
| --- | --- | --- | --- |
| Carpet | 4 m | 0.5 m | Fine low pile, subdued wear, matte reflection |
| Wallpaper | 4 m | 4 m | Faint stripes, 500 mm roll seams, shallow paper relief |
| Acoustic panels | 6 m | 6 m | 600 × 1200 mm grid and shallow pores |

Albedo is a neutral sRGB multiplier; `observed_style::backrooms` owns colour and
emission. Normals use linear OpenGL tangent-space encoding. ORM is linear:
R = occlusion, G = roughness, B = metalness. Carpet uses the mesh's second UV
channel for its micro normal repeat. All maps receive filtered mip chains;
albedo filters in linear light, normal mips renormalize, and ORM stays data.

The asset manifest names every map. Native clients read those slots, falling
back to baked copies when absent or invalid; web clients use the baked copies.
Rebuild source maps rather than hand-editing the emitted PNGs.

Example using the bundled workspace Python runtime:

```bash
/home/will/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 tools/generate_backrooms_materials.py
```
