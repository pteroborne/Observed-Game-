//! What each district's surfaces are made of: a tiling detail image and a normal map
//! for its floor, its walls and its ceiling.
//!
//! The district tint says what colour a surface is and how dark; this says what it is
//! made of - wallpaper or panelling, carpet or tatami, plaster or steel plate - so a
//! floor reads as a place rather than as a colour. Every image is drawn here, from a
//! height field, and never loaded: one function per material, deterministic, tiling.
//!
//! Two rules, both tested:
//!
//! * **Detail only darkens.** The albedo is a multiplier at most one, mostly near it,
//!   so the tint stays the surface's brightness and the Legibility Contract's
//!   atmosphere ceiling, which binds the tint, still binds the surface.
//! * **Every image tiles.** One tile is [`SURFACE_TILE_METRES`] square, every pattern
//!   repeats a whole number of times across it, and every noise is periodic on it, so
//!   there is no seam where one repeat meets the next.

use observed_content::ArchitectureRegister;

use crate::ArchitectureSurfaceRole;

/// Side of every surface image, texels.
pub const SURFACE_TEXTURE_SIZE: u32 = 512;
/// How many metres one tile of a surface image covers: the renderer's box projection
/// repeats a texture once every four metres.
pub const SURFACE_TILE_METRES: f32 = 4.0;

/// A surface's images: the albedo multiplier as RGBA8 (sRGB), and a
/// tangent-space normal map as RGBA8 (linear).
pub struct SurfaceImages {
    pub albedo: Vec<u8>,
    pub normal: Vec<u8>,
}

/// One texel of a material: how high the surface stands there (0 to 1, a joint or a
/// groove low) and how much of the tint it reflects (at most 1).
#[derive(Clone, Copy)]
struct Texel {
    height: f32,
    albedo: f32,
}

/// The images for `register`'s `role`.
#[must_use]
pub fn surface_images(
    register: ArchitectureRegister,
    role: ArchitectureSurfaceRole,
) -> SurfaceImages {
    if register == ArchitectureRegister::ShadowScreen
        && role != ArchitectureSurfaceRole::PracticalFixture
    {
        return SurfaceImages {
            albedo: crate::rain_court::albedo(if role == ArchitectureSurfaceRole::Wall {
                1
            } else {
                0
            }),
            // Physical lattice and slats supply the relief, as in the Rain Court.
            normal: [128, 128, 255, 255]
                .repeat((SURFACE_TEXTURE_SIZE * SURFACE_TEXTURE_SIZE) as usize),
        };
    }
    if register == ArchitectureRegister::Megastructure
        && role != ArchitectureSurfaceRole::PracticalFixture
    {
        return crate::reactor::panel_images(role);
    }
    let material = material(register, role);
    // A wall carries its district's weave too - the lattice, the dado, the shuttering
    // (`architecture_weave`) - struck into the material as joints.
    let weave = (role == ArchitectureSurfaceRole::Wall)
        .then(|| crate::surface_weave_rgba(crate::architecture_weave(register)))
        .flatten();
    let weave_size = crate::SURFACE_WEAVE_SIZE as usize;
    let n = SURFACE_TEXTURE_SIZE as usize;
    let mut texels = Vec::with_capacity(n * n);
    for y in 0..n {
        for x in 0..n {
            #[allow(clippy::cast_precision_loss)]
            let (u, v) = (
                (x as f32 + 0.5) / n as f32 * SURFACE_TILE_METRES,
                (y as f32 + 0.5) / n as f32 * SURFACE_TILE_METRES,
            );
            let mut texel = material(u, v);
            if let Some(weave) = &weave {
                let (wx, wy) = (x * weave_size / n, y * weave_size / n);
                let struck = f32::from(weave[(wy * weave_size + wx) * 4]) / 255.0;
                texel.albedo *= struck;
                texel.height *= struck;
            }
            texels.push(texel);
        }
    }
    let mut albedo = Vec::with_capacity(n * n * 4);
    for texel in &texels {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let a = (texel.albedo.clamp(0.0, 1.0) * 255.0).round() as u8;
        albedo.extend_from_slice(&[a, a, a, 255]);
    }
    let strength = relief(register, role);
    let height = |x: usize, y: usize| texels[(y % n) * n + (x % n)].height;
    let mut normal = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            // Central differences, wrapping, so the normal map tiles as the height does.
            let dx = height(x + 1, y) - height(x + n - 1, y);
            let dy = height(x, y + 1) - height(x, y + n - 1);
            let (nx, ny, nz) = (-dx * strength, -dy * strength, 1.0);
            let length = (nx * nx + ny * ny + nz * nz).sqrt();
            let encode = |c: f32| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let byte = ((c / length * 0.5 + 0.5) * 255.0).round() as u8;
                byte
            };
            normal.extend_from_slice(&[encode(nx), encode(ny), encode(nz), 255]);
        }
    }
    // Share the Archive Well's checker-cut floor, including the established
    // relief beneath it. Walls and ceilings already use the same district images.
    if register == ArchitectureRegister::InfiniteGallery && role == ArchitectureSurfaceRole::Floor {
        albedo = crate::archive::floor_albedo();
    }
    SurfaceImages { albedo, normal }
}

/// How strongly a material's height reads as relief: a texel of height step becomes
/// this much slope in the normal map. Deep joints for blockwork and panelling, a
/// whisper for paper and paint.
fn relief(register: ArchitectureRegister, role: ArchitectureSurfaceRole) -> f32 {
    use ArchitectureRegister as R;
    use ArchitectureSurfaceRole as S;
    match (register, role) {
        // Carpet and rush are soft: a hard relief on their fine weave reads as gravel.
        (R::LiminalGrid, S::Floor) | (R::ShadowScreen, S::Floor) => 2.5,
        // Plaster and poured concrete are smooth: their mottling is colour, not form.
        (R::Thinning, S::Wall | S::Ceiling | S::PracticalFixture) | (R::Monolith, _) => 3.0,
        (R::ShadowScreen, S::Wall) | (R::LiminalGrid, S::Wall) => 6.0,
        (R::Megastructure, _) | (R::FacetMonument, _) | (R::Institutional, S::Wall) => 18.0,
        _ => 11.0,
    }
}

type Material = Box<dyn Fn(f32, f32) -> Texel>;

/// The material for a register's role, as a function of position in metres on one tile.
fn material(register: ArchitectureRegister, role: ArchitectureSurfaceRole) -> Material {
    use ArchitectureRegister as R;
    use ArchitectureSurfaceRole as S;
    match (register, role) {
        // The Backrooms: mono-yellow wallpaper in faint stripes, damp carpet, and a
        // dropped ceiling of acoustic tiles. The uncanny part is that it is ordinary.
        (R::LiminalGrid, S::Wall) => Box::new(wallpaper),
        (R::LiminalGrid, S::Floor) => Box::new(carpet),
        (R::LiminalGrid, S::Ceiling | S::PracticalFixture) => Box::new(acoustic_tiles),
        // The Library: panelled in boards, floored in staggered planks, coffered above.
        (R::InfiniteGallery, S::Wall) => Box::new(panelling),
        (R::InfiniteGallery, S::Floor) => Box::new(planks),
        (R::InfiniteGallery, S::Ceiling | S::PracticalFixture) => Box::new(coffers),
        // Lumen: tile, everywhere, and a grid of light above.
        (R::OverlitGrid, S::Wall) => Box::new(|u, v| tiles(u, v, 0.5, 0.5, 0.012)),
        (R::OverlitGrid, S::Floor) => Box::new(|u, v| tiles(u, v, 1.0, 1.0, 0.015)),
        (R::OverlitGrid, S::Ceiling | S::PracticalFixture) => {
            Box::new(|u, v| tiles(u, v, 0.5, 0.5, 0.03))
        }
        // Zen: paper behind a lattice, tatami, and a slatted timber ceiling.
        (R::ShadowScreen, S::Wall) => Box::new(paper),
        (R::ShadowScreen, S::Floor) => Box::new(tatami),
        (R::ShadowScreen, S::Ceiling | S::PracticalFixture) => Box::new(|u, v| slats(u, v, 0.25)),
        // The Monument: ashlar, polished slabs, coffers.
        (R::FacetMonument, S::Wall) => Box::new(|u, v| ashlar(u, v, 1.0, 0.5)),
        (R::FacetMonument, S::Floor) => Box::new(|u, v| tiles(u, v, 2.0, 2.0, 0.02)),
        (R::FacetMonument, S::Ceiling | S::PracticalFixture) => Box::new(coffers),
        // The Reactor: riveted plate, tread plate, ribs.
        (R::Megastructure, S::Wall) => Box::new(riveted_plate),
        (R::Megastructure, S::Floor) => Box::new(tread_plate),
        (R::Megastructure, S::Ceiling | S::PracticalFixture) => Box::new(|u, v| slats(u, v, 1.0)),
        // The Sky: limewashed plaster and weathered deck boards.
        (R::Thinning, S::Wall | S::Ceiling | S::PracticalFixture) => Box::new(plaster),
        (R::Thinning, S::Floor) => Box::new(planks),
        // Off the climb: poured concrete, painted block, stone courses.
        (R::Monolith, _) => Box::new(concrete),
        (R::Institutional, S::Wall) => Box::new(|u, v| ashlar(u, v, 0.4, 0.2)),
        (R::Institutional, _) => Box::new(|u, v| tiles(u, v, 0.5, 0.5, 0.01)),
        (R::Wellshaft, S::Floor) => Box::new(|u, v| tiles(u, v, 1.0, 1.0, 0.02)),
        (R::Wellshaft, _) => Box::new(|u, v| ashlar(u, v, 1.0, 0.5)),
    }
}

// ---- noise ---------------------------------------------------------------------

/// A hash of an integer lattice point, in [0, 1).
fn hash(x: i32, y: i32, seed: u32) -> f32 {
    #[allow(clippy::cast_sign_loss)]
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ seed.wrapping_mul(0xCB1A_B31F);
    h = (h ^ (h >> 13)).wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    #[allow(clippy::cast_precision_loss)]
    let value = h as f32 / u32::MAX as f32;
    value
}

/// Value noise with `cells` lattice cells across the tile, periodic on it.
fn noise(u: f32, v: f32, cells: i32, seed: u32) -> f32 {
    stretched(u, v, cells, cells, seed)
}

/// Value noise with `across` lattice cells along `u` and `down` along `v`: a grain,
/// periodic on the tile in both.
fn stretched(u: f32, v: f32, across: i32, down: i32, seed: u32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let (x, y) = (
        u * across as f32 / SURFACE_TILE_METRES,
        v * down as f32 / SURFACE_TILE_METRES,
    );
    #[allow(clippy::cast_possible_truncation)]
    let (x0, y0) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let at = |i: i32, j: i32| hash(i.rem_euclid(across), j.rem_euclid(down), seed);
    let a = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * sx;
    let b = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * sx;
    a + (b - a) * sy
}

/// Fractal noise: `octaves` of [`noise`] from `cells` across, each twice as fine.
fn fbm(u: f32, v: f32, cells: i32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amplitude, mut total) = (0.0, 0.5, 0.0);
    for octave in 0..octaves {
        sum += noise(u, v, cells << octave, seed + octave) * amplitude;
        total += amplitude;
        amplitude *= 0.5;
    }
    sum / total
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far `t` is from the nearest multiple of `pitch`, in the same units.
fn from_line(t: f32, pitch: f32) -> f32 {
    let phase = t.rem_euclid(pitch);
    phase.min(pitch - phase)
}

/// A joint's profile: 0 on the line, rising to 1 at `half` from it, with a bevel.
fn joint(distance: f32, half: f32) -> f32 {
    smoothstep(half * 0.4, half, distance)
}

// ---- materials -----------------------------------------------------------------

fn wallpaper(u: f32, v: f32) -> Texel {
    // Stripes a fifth of a metre apart in two barely different tones, a hairline at each
    // seam, and the faintest water stains low down.
    let stripe = if (u / 0.2).floor().rem_euclid(2.0) < 1.0 {
        0.0
    } else {
        1.0
    };
    let seam = joint(from_line(u, 0.5), 0.006);
    let stain = smoothstep(0.55, 0.85, fbm(u, v, 4, 4, 11)) * 0.10;
    let paper = fbm(u, v, 64, 2, 12) * 0.05;
    Texel {
        height: 0.6 + seam * 0.4 - paper,
        albedo: 0.92 - stripe * 0.05 - (1.0 - seam) * 0.18 - stain - paper,
    }
}

fn carpet(u: f32, v: f32) -> Texel {
    let pile = fbm(u, v, 128, 2, 21);
    let wear = fbm(u, v, 4, 3, 22);
    Texel {
        height: pile * 0.5,
        albedo: 0.86 + pile * 0.06 - smoothstep(0.6, 0.9, wear) * 0.10,
    }
}

fn acoustic_tiles(u: f32, v: f32) -> Texel {
    // Two-foot tiles in a T-bar grid, the tile face pitted.
    // Seven to the tile: near enough two feet, and a whole number of them.
    let pitch = SURFACE_TILE_METRES / 7.0;
    let grid = joint(from_line(u, pitch), 0.025).min(joint(from_line(v, pitch), 0.025));
    let pits = smoothstep(0.62, 0.7, noise(u, v, 256, 31));
    Texel {
        height: grid * (1.0 - pits * 0.3),
        albedo: 0.70 + grid * 0.26 - pits * 0.08,
    }
}

fn panelling(u: f32, v: f32) -> Texel {
    // Boards a quarter-metre wide with V-grooves, the grain running up them.
    let groove = joint(from_line(u, 0.25), 0.012);
    #[allow(clippy::cast_possible_truncation)]
    let board = (u / 0.25).floor() as i32;
    let grain = stretched(u, v, 192, 12, 41 + board.rem_euclid(16) as u32);
    let tone = hash(board.rem_euclid(16), 0, 42) * 0.10;
    Texel {
        height: groove * (0.9 + grain * 0.1),
        albedo: 0.72 + grain * 0.14 - tone + groove * 0.12 - (1.0 - groove) * 0.3,
    }
}

fn planks(u: f32, v: f32) -> Texel {
    // Boards a fifth of a metre wide, butted every metre at staggered places.
    let row = (v / 0.2).floor();
    #[allow(clippy::cast_possible_truncation)]
    let stagger = hash(row as i32, 1, 51) * 1.0;
    let side = joint(from_line(v, 0.2), 0.008);
    let end = joint(from_line(u + stagger, 1.0), 0.008);
    #[allow(clippy::cast_possible_truncation)]
    let grain = stretched(u, v, 12, 160, 52 + (row as i32).rem_euclid(20) as u32);
    #[allow(clippy::cast_possible_truncation)]
    let tone = hash(row as i32, (u + stagger).floor() as i32, 53) * 0.12;
    let gap = side.min(end);
    Texel {
        height: gap * (0.92 + grain * 0.08),
        albedo: 0.70 + grain * 0.16 - tone - (1.0 - gap) * 0.35,
    }
}

fn coffers(u: f32, v: f32) -> Texel {
    // A metre grid of sunken panels: the rib high, the coffer low, a bevel between.
    let rib = from_line(u, 1.0).min(from_line(v, 1.0));
    let depth = smoothstep(0.06, 0.14, rib);
    Texel {
        height: 1.0 - depth * 0.8,
        albedo: 0.92 - depth * 0.16,
    }
}

fn tiles(u: f32, v: f32, w: f32, h: f32, grout: f32) -> Texel {
    let line = joint(from_line(u, w), grout).min(joint(from_line(v, h), grout));
    #[allow(clippy::cast_possible_truncation)]
    let tone = hash((u / w).floor() as i32, (v / h).floor() as i32, 61) * 0.05;
    let glaze = fbm(u, v, 16, 2, 62) * 0.04;
    Texel {
        height: line,
        albedo: 0.95 - tone - glaze - (1.0 - line) * 0.30,
    }
}

fn paper(u: f32, v: f32) -> Texel {
    // Washi: fibres, and nothing else - the lattice in front of it is the district's
    // weave, drawn over this.
    let fibre = stretched(u, v, 144, 48, 71) * 0.6 + fbm(u, v, 24, 2, 72) * 0.4;
    Texel {
        height: fibre * 0.4,
        albedo: 0.90 + fibre * 0.08,
    }
}

fn tatami(u: f32, v: f32) -> Texel {
    // Mats a metre by two, turned in alternate two-metre squares, each with a dark
    // cloth border and its rush woven along its length.
    let (bu, bv) = ((u / 2.0).floor(), (v / 2.0).floor());
    let turned = (bu + bv).rem_euclid(2.0) >= 1.0;
    let (along, across) = if turned { (u, v) } else { (v, u) };
    let border = from_line(across, 1.0).min(from_line(along, 2.0));
    let cloth = smoothstep(0.035, 0.05, border);
    let weave = (along * 120.0 * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    Texel {
        height: 0.5 + cloth * 0.3 + weave * 0.15 * cloth,
        albedo: 0.62 + cloth * (0.24 + weave * 0.06),
    }
}

fn slats(u: f32, v: f32, pitch: f32) -> Texel {
    let _ = v;
    let gap = joint(from_line(u, pitch), pitch * 0.12);
    Texel {
        height: gap,
        albedo: 0.85 - (1.0 - gap) * 0.45,
    }
}

fn ashlar(u: f32, v: f32, w: f32, h: f32) -> Texel {
    // Blocks in running bond, chamfered joints, each block's face a little different.
    let row = (v / h).floor();
    let offset = if row.rem_euclid(2.0) >= 1.0 {
        w * 0.5
    } else {
        0.0
    };
    let line = joint(from_line(u + offset, w), 0.025).min(joint(from_line(v, h), 0.025));
    #[allow(clippy::cast_possible_truncation)]
    let tone = hash(((u + offset) / w).floor() as i32, row as i32, 81) * 0.08;
    let face = fbm(u, v, 16, 3, 82) * 0.10;
    Texel {
        height: line * (0.9 + face),
        albedo: 0.88 - tone - face - (1.0 - line) * 0.32,
    }
}

fn riveted_plate(u: f32, v: f32) -> Texel {
    // Plates a metre by two, seamed, with a row of rivets in from each edge.
    let seam = joint(from_line(u, 1.0), 0.01).min(joint(from_line(v, 2.0), 0.01));
    let (ru, rv) = (u.rem_euclid(1.0), v.rem_euclid(0.25));
    let near_edge = ru.min(1.0 - ru) < 0.09;
    let rivet = if near_edge {
        let du = ru.min(1.0 - ru) - 0.05;
        let dv = rv - 0.125;
        1.0 - smoothstep(0.012, 0.022, (du * du + dv * dv).sqrt())
    } else {
        0.0
    };
    let grime = fbm(u, v, 8, 4, 91);
    Texel {
        height: 0.5 * seam + rivet * 0.5,
        albedo: 0.80 - grime * 0.18 - (1.0 - seam) * 0.35 + rivet * 0.08,
    }
}

fn tread_plate(u: f32, v: f32) -> Texel {
    // Raised lozenges at right angles in alternate cells of a 5 cm grid.
    let (cu, cv) = ((u / 0.05).floor(), (v / 0.05).floor());
    let (fu, fv) = (
        u.rem_euclid(0.05) / 0.05 - 0.5,
        v.rem_euclid(0.05) / 0.05 - 0.5,
    );
    let (a, b) = if (cu + cv).rem_euclid(2.0) >= 1.0 {
        (fu, fv)
    } else {
        (fv, fu)
    };
    let lozenge = 1.0 - smoothstep(0.85, 1.0, (a * 2.2).abs() + (b * 7.0).abs());
    let grime = fbm(u, v, 8, 3, 101);
    Texel {
        height: lozenge,
        albedo: 0.74 + lozenge * 0.12 - grime * 0.14,
    }
}

fn plaster(u: f32, v: f32) -> Texel {
    let wash = fbm(u, v, 4, 5, 111);
    let trowel = fbm(u, v, 24, 2, 112);
    Texel {
        height: trowel * 0.6 + wash * 0.4,
        albedo: 0.86 + wash * 0.10 - trowel * 0.06,
    }
}

fn concrete(u: f32, v: f32) -> Texel {
    // Board-formed: a shutter seam every seventh of the tile and tie holes on its grid.
    let board = SURFACE_TILE_METRES / 7.0;
    let seam = joint(from_line(v, board), 0.006);
    let (hu, hv) = (from_line(u, 1.0), from_line(v - board * 0.5, board));
    let hole = 1.0 - smoothstep(0.012, 0.02, (hu * hu + hv * hv).sqrt());
    let aggregate = fbm(u, v, 32, 3, 121);
    Texel {
        height: seam * (1.0 - hole) * (0.85 + aggregate * 0.15),
        albedo: 0.82 + aggregate * 0.12 - (1.0 - seam) * 0.15 - hole * 0.4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_surface() -> impl Iterator<Item = (ArchitectureRegister, ArchitectureSurfaceRole)> {
        ArchitectureRegister::ALL.into_iter().flat_map(|register| {
            [
                ArchitectureSurfaceRole::Floor,
                ArchitectureSurfaceRole::Wall,
                ArchitectureSurfaceRole::Ceiling,
            ]
            .map(move |role| (register, role))
        })
    }

    /// Detail only darkens, and not by much on average: the tint stays the surface's
    /// brightness, so the atmosphere ceiling that binds the tint binds the surface.
    #[test]
    fn detail_darkens_a_little_and_never_brightens() {
        for (register, role) in every_surface() {
            let images = surface_images(register, role);
            let values: Vec<f32> = images
                .albedo
                .chunks(4)
                .map(|t| f32::from(t[0]) / 255.0)
                .collect();
            #[allow(clippy::cast_precision_loss)]
            let mean = values.iter().sum::<f32>() / values.len() as f32;
            assert!(
                (0.6..=0.97).contains(&mean),
                "{register:?} {role:?}: mean albedo {mean}"
            );
            assert!(values.iter().all(|&v| v <= 1.0));
        }
    }

    /// Every image tiles: its last column runs on into its first, and its last row into
    /// its first, with no jump bigger than a pattern's own edges make inside the tile.
    #[test]
    fn every_surface_tiles() {
        let n = SURFACE_TEXTURE_SIZE as usize;
        for (register, role) in every_surface() {
            let images = surface_images(register, role);
            let at = |x: usize, y: usize| i32::from(images.albedo[(y * n + x) * 4]);
            let worst_inside = (0..n)
                .flat_map(|y| (0..n - 1).map(move |x| (x, y)))
                .map(|(x, y)| (at(x, y) - at(x + 1, y)).abs())
                .max()
                .unwrap_or(0);
            let across = (0..n)
                .map(|y| (at(n - 1, y) - at(0, y)).abs())
                .max()
                .unwrap_or(0);
            let down = (0..n)
                .map(|x| (at(x, n - 1) - at(x, 0)).abs())
                .max()
                .unwrap_or(0);
            assert!(
                across <= worst_inside && down <= worst_inside,
                "{register:?} {role:?}: seam {across}/{down} against {worst_inside} inside"
            );
        }
    }

    /// The normal maps are unit normals facing out of the surface.
    #[test]
    fn normals_face_out() {
        let images = surface_images(
            ArchitectureRegister::Megastructure,
            ArchitectureSurfaceRole::Wall,
        );
        for texel in images.normal.chunks(4) {
            let decode = |c: u8| f32::from(c) / 255.0 * 2.0 - 1.0;
            let (x, y, z) = (decode(texel[0]), decode(texel[1]), decode(texel[2]));
            assert!(z > 0.0);
            assert!(((x * x + y * y + z * z).sqrt() - 1.0).abs() < 0.03);
        }
    }

    /// The districts on the climb do not share a material: each floor is made of
    /// something of its own.
    #[test]
    fn the_climb_is_made_of_different_things() {
        let mut seen = Vec::new();
        for register in ArchitectureRegister::CLIMB {
            let images = surface_images(register, ArchitectureSurfaceRole::Floor);
            if !seen.contains(&images.albedo) {
                seen.push(images.albedo);
            }
        }
        assert!(
            seen.len() >= 6,
            "only {} distinct floors on the climb",
            seen.len()
        );
    }
}
