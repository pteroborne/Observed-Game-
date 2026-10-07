#!/usr/bin/env python3
"""Rebuild original, periodic Backrooms PBR textures. Requires numpy and Pillow.

Albedo is a neutral multiplier: observed_style owns colour. Normals are
OpenGL/Y-up, ORM packs occlusion/roughness/metalness in R/G/B. All physical
relief is in metres, rather than an arbitrary normal-map strength.
"""
from pathlib import Path
import numpy as np
from PIL import Image

SIZE = 2048
ROOT = Path(__file__).resolve().parents[1] / "assets/textures/backrooms"


def noise(cells, seed):
    rng = np.random.default_rng(seed)
    grid = rng.random((cells, cells)).astype(np.float32)
    at = (np.arange(SIZE, dtype=np.float32) + 0.5) * cells / SIZE
    index = np.floor(at).astype(np.int32)
    t = at - index
    t = t * t * (3 - 2 * t)
    a = grid[index[:, None] % cells, index[None, :] % cells]
    b = grid[index[:, None] % cells, (index[None, :] + 1) % cells]
    c = grid[(index[:, None] + 1) % cells, index[None, :] % cells]
    d = grid[(index[:, None] + 1) % cells, (index[None, :] + 1) % cells]
    return (a * (1-t)[None, :] + b * t[None, :]) * (1-t)[:, None] + (c * (1-t)[None, :] + d * t[None, :]) * t[:, None]


def save(name, albedo, height, roughness, metres):
    dx = (np.roll(height, -1, 1) - np.roll(height, 1, 1)) * SIZE / (2 * metres)
    dy = (np.roll(height, -1, 0) - np.roll(height, 1, 0)) * SIZE / (2 * metres)
    normal = np.stack((-dx, -dy, np.ones_like(dx)), axis=-1)
    normal /= np.linalg.norm(normal, axis=-1, keepdims=True)
    rgb = np.repeat(np.clip(albedo[..., None], 0, 1), 3, axis=-1)
    orm = np.stack((np.ones_like(height), np.clip(roughness, 0, 1), np.zeros_like(height)), axis=-1)
    for suffix, data in (("albedo", rgb), ("normal", normal * .5 + .5), ("orm", orm)):
        Image.fromarray(np.round(data * 255).astype(np.uint8)).save(ROOT / f"{name}_{suffix}.png", optimize=True)


def main():
    ROOT.mkdir(parents=True, exist_ok=True)
    fine = noise(768, 21)
    pile = noise(1024, 22)
    macro = noise(8, 23)
    # Sub-millimetre fibres, on the floor's separate half-metre normal repeat.
    # Albedo and roughness retain their four-metre macro repeat.
    save("carpet", .94 + (fine-.5)*.035 + (macro-.5)*.025,
         (pile-.5)*.00005625, .96 + (fine-.5)*.025, .5)

    x = (np.arange(SIZE, dtype=np.float32)+.5) / SIZE * 4
    seam = np.minimum(x % .5, .5 - x % .5)
    seam = np.exp(-(seam / .0025)**2)[None, :]
    paper = noise(512, 31)
    stripe = np.sin(x * (2*np.pi/.2))[None, :]
    save("wallpaper", .965 + (paper-.5)*.015 + stripe*.008 - seam*.025,
         (paper-.5)*.00008 - seam*.00015, .82 + (paper-.5)*.035, 4.0)

    # A six-metre repeat contains exact 600 x 1200 mm ceiling panels.
    x = (np.arange(SIZE, dtype=np.float32)+.5) / SIZE * 6
    line_x = np.minimum(x % .6, .6 - x % .6)[None, :]
    line_y = np.minimum(x % 1.2, 1.2 - x % 1.2)[:, None]
    grid = np.maximum(np.exp(-(line_x/.009)**4), np.exp(-(line_y/.009)**4))
    pores = noise(768, 41)
    pits = np.maximum(pores-.66, 0) / .34
    face = noise(10, 42)
    save("acoustic", .96 - grid*.13 - pits*.03 + (face-.5)*.014,
         -grid*.0012 - pits*.00025, .91 - grid*.2 + (pores-.5)*.02, 6.0)


if __name__ == "__main__":
    main()
