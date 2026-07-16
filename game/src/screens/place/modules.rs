//! Hallway light modules (Arc I Phase 71): the nine lab registers enter
//! generated hallways as a WFC-style module layer. Each grid cell collapses to
//! one module — slat screen, seam strip, panel grid, practical, shelf run,
//! void edge, or bare — under adjacency constraints, weighted by the
//! district's register identity (`observed_style::hallway_module_weights`).
//!
//! Two properties are load-bearing:
//! - **Solvable by construction:** `Bare` is legal in every cell, so the
//!   collapse can never contradict — the same "always solvable" discipline the
//!   facility graph keeps, in miniature. `Bare`'s weight also grows with
//!   distance from the entry, which IS the thinning register (Rudon's Plane):
//!   variety decays diegetically down every corridor.
//! - **Decoration only:** the solver reads the finished geometry (footprint,
//!   interior walls, gaps) and never moves any of it. Threshold-adjacent cells
//!   are forced `Bare`, so the Phase-64 gate is untouchable from here. Module
//!   entities carry no colliders.

use bevy::prelude::*;
use observed_core::SplitMix;
use observed_style as style;

use crate::GameState;
use crate::layout::WALL_HEIGHT;
use crate::teleport::{DoorGap, PlaceGeom, WallSeg};
use crate::view::assets::MatchAssets;
use crate::view::components::{PassagePreview, PlaceGeometry};

/// One collapsed module kind. Indexes `style::hallway_module_weights` — the
/// correspondence is pinned by `weight_order_matches_the_style_crate`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModuleKind {
    Slat,
    Seam,
    Panel,
    Practical,
    Shelf,
    Void,
    Bare,
}

impl ModuleKind {
    const ALL: [ModuleKind; style::HALLWAY_MODULE_COUNT] = [
        ModuleKind::Slat,
        ModuleKind::Seam,
        ModuleKind::Panel,
        ModuleKind::Practical,
        ModuleKind::Shelf,
        ModuleKind::Void,
        ModuleKind::Bare,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// A wall face a module attaches to: a point on the wall plane and the normal
/// pointing into the corridor. Grid walls are axis-aligned, so `normal` is one
/// of the four cardinal directions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WallFace {
    pub(crate) center: Vec2,
    pub(crate) normal: Vec2,
    /// True when this face belongs to the hallway's outer boundary (the only
    /// faces a `Void` edge may imply depth behind).
    pub(crate) boundary: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ModulePlacement {
    pub(crate) kind: ModuleKind,
    pub(crate) cell_center: Vec2,
    pub(crate) face: Option<WallFace>,
    /// 0 at the entry row, 1 at the far end — the thinning gradient. Lights
    /// and emission fade with it.
    pub(crate) decay: f32,
}

/// The per-hallway module seed: a pure function of the map seed and the edge,
/// so live place and preview collapse identically.
pub(crate) fn hall_module_seed(seed: u64, from: u32, to: u32) -> u64 {
    seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (from as u64).wrapping_mul(0x0000_0001_0000_01B3)
        ^ (to as u64).wrapping_mul(0x0100_0000_01B3_0000)
}

/// Threshold clearance: cells whose center is within this many cell-widths of
/// any doorway gap are forced `Bare` (prop rule: nothing near a threshold).
const GAP_CLEAR_CELLS: f32 = 1.25;
const SLAT_RUN_MIN: usize = 3;
const SLAT_RUN_MAX: usize = 6;
const SHELF_RUN_MIN: usize = 2;
const SHELF_RUN_MAX: usize = 4;
const PRACTICAL_SEPARATION: i32 = 2;

/// Collapse the module layer for a grid hallway. Pure: same inputs, same
/// placements. Returns only non-`Bare` placements. Gantry hallways (any
/// `decks`) keep their own established language and get no modules.
pub(crate) fn solve_hallway_modules(
    hall_seed: u64,
    geom: &PlaceGeom,
    cell: f32,
    district: style::District,
) -> Vec<ModulePlacement> {
    if !geom.decks.is_empty() || geom.poly.is_some() {
        return Vec::new();
    }
    let (hx, hz) = (geom.half.x, geom.half.y);
    let cols = ((2.0 * hx) / cell).round() as usize;
    let rows = ((2.0 * hz) / cell).round() as usize;
    if cols < 2 || rows < 2 {
        return Vec::new();
    }

    let weights = style::hallway_module_weights(district);
    let mut rng = SplitMix::new(hall_seed);
    let mut kinds: Vec<ModuleKind> = vec![ModuleKind::Bare; cols * rows];
    let mut faces: Vec<Option<WallFace>> = vec![None; cols * rows];
    let idx = |c: usize, r: usize| r * cols + c;
    let cell_center = |c: usize, r: usize| {
        Vec2::new(-hx + (c as f32 + 0.5) * cell, -hz + (r as f32 + 0.5) * cell)
    };

    for r in 0..rows {
        let decay = r as f32 / (rows - 1).max(1) as f32;
        for c in 0..cols {
            let center = cell_center(c, r);
            if near_gap(center, &geom.gaps, cell) {
                continue; // forced Bare: threshold clearance
            }
            let cell_faces = faces_for_cell(center, c, r, cols, rows, hx, hz, cell, &geom.interior);
            let junction = cell_faces.iter().any(|f| f.normal.x.abs() > 0.5)
                && cell_faces.iter().any(|f| f.normal.y.abs() > 0.5);
            let boundary_face = cell_faces.iter().find(|f| f.boundary).copied();
            let any_face = cell_faces.first().copied();

            // Continuation reads: the neighbor toward the entry on the same column.
            let prev = if r > 0 {
                Some(kinds[idx(c, r - 1)])
            } else {
                None
            };
            let prev_run = run_length_back(&kinds, cols, c, r);
            let left = if c > 0 {
                Some(kinds[idx(c - 1, r)])
            } else {
                None
            };

            let mut w = [0u32; style::HALLWAY_MODULE_COUNT];
            // Slat / Shelf want a wall face; runs continue strongly, then cap.
            if any_face.is_some() {
                w[ModuleKind::Slat.index()] = match (prev, prev_run) {
                    (Some(ModuleKind::Slat), n) if n >= SLAT_RUN_MAX => 0,
                    (Some(ModuleKind::Slat), _) => weights[ModuleKind::Slat.index()] * 5,
                    _ => weights[ModuleKind::Slat.index()],
                };
                w[ModuleKind::Shelf.index()] = match (prev, prev_run) {
                    (Some(ModuleKind::Shelf), n) if n >= SHELF_RUN_MAX => 0,
                    (Some(ModuleKind::Shelf), _) => weights[ModuleKind::Shelf.index()] * 4,
                    _ => weights[ModuleKind::Shelf.index()],
                };
            }
            if junction {
                w[ModuleKind::Seam.index()] = weights[ModuleKind::Seam.index()];
            }
            // Panels grow regions (ceiling module — no face needed).
            w[ModuleKind::Panel.index()] =
                if prev == Some(ModuleKind::Panel) || left == Some(ModuleKind::Panel) {
                    weights[ModuleKind::Panel.index()] * 4
                } else {
                    weights[ModuleKind::Panel.index()]
                };
            // Practicals keep pool separation.
            if any_face.is_some()
                && !practical_nearby(&kinds, cols, rows, c, r, PRACTICAL_SEPARATION)
            {
                w[ModuleKind::Practical.index()] = weights[ModuleKind::Practical.index()];
            }
            // At most one void edge, boundary walls only, never on the end rows.
            if boundary_face.is_some()
                && r > 0
                && r < rows - 1
                && !kinds.contains(&ModuleKind::Void)
            {
                w[ModuleKind::Void.index()] = weights[ModuleKind::Void.index()];
            }
            // Bare is always legal and thickens toward the far end: the
            // thinning gradient. (+1 keeps the total non-zero even if a
            // district zeroes bare out by mistake.)
            w[ModuleKind::Bare.index()] =
                weights[ModuleKind::Bare.index()] * (1 + (decay * 3.0) as u32) + 1;

            let total: u32 = w.iter().sum();
            let mut pick = rng.below(total as usize) as u32;
            let mut chosen = ModuleKind::Bare;
            for kind in ModuleKind::ALL {
                let kw = w[kind.index()];
                if pick < kw {
                    chosen = kind;
                    break;
                }
                pick -= kw;
            }
            kinds[idx(c, r)] = chosen;
            faces[idx(c, r)] = match chosen {
                ModuleKind::Void => boundary_face,
                ModuleKind::Slat | ModuleKind::Shelf | ModuleKind::Practical | ModuleKind::Seam => {
                    any_face
                }
                _ => None,
            };
        }
    }

    // Post passes: no orphan runs, no lone panels — architecture, not confetti.
    demote_short_runs(&mut kinds, cols, rows, ModuleKind::Slat, SLAT_RUN_MIN);
    demote_short_runs(&mut kinds, cols, rows, ModuleKind::Shelf, SHELF_RUN_MIN);
    for r in 0..rows {
        for c in 0..cols {
            if kinds[idx(c, r)] == ModuleKind::Panel && !has_orthogonal(&kinds, cols, rows, c, r) {
                kinds[idx(c, r)] = ModuleKind::Bare;
            }
        }
    }

    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let kind = kinds[idx(c, r)];
            if kind == ModuleKind::Bare {
                continue;
            }
            out.push(ModulePlacement {
                kind,
                cell_center: cell_center(c, r),
                face: faces[idx(c, r)],
                decay: r as f32 / (rows - 1).max(1) as f32,
            });
        }
    }
    match geom.architecture_register {
        Some(observed_content::ArchitectureRegister::ShadowScreen) => out
            .into_iter()
            .enumerate()
            .filter_map(|(index, placement)| {
                (matches!(placement.kind, ModuleKind::Slat | ModuleKind::Seam)
                    && index.is_multiple_of(3))
                .then_some(placement)
            })
            .collect(),
        Some(observed_content::ArchitectureRegister::Institutional) => out
            .into_iter()
            .enumerate()
            .filter_map(|(index, placement)| {
                (matches!(
                    placement.kind,
                    ModuleKind::Panel | ModuleKind::Practical | ModuleKind::Seam
                ) && index.is_multiple_of(2))
                .then_some(placement)
            })
            .collect(),
        _ => out,
    }
}

fn near_gap(center: Vec2, gaps: &[DoorGap], cell: f32) -> bool {
    gaps.iter()
        .any(|g| center.distance(g.center) < cell * GAP_CLEAR_CELLS)
}

/// The wall faces bordering a cell: outer boundary sides plus any interior
/// wall segment lying along one of the cell's four edges.
#[allow(clippy::too_many_arguments)]
fn faces_for_cell(
    center: Vec2,
    c: usize,
    r: usize,
    cols: usize,
    rows: usize,
    hx: f32,
    hz: f32,
    cell: f32,
    interior: &[WallSeg],
) -> Vec<WallFace> {
    let mut faces = Vec::new();
    let half = cell * 0.5;
    if c == 0 {
        faces.push(WallFace {
            center: Vec2::new(-hx, center.y),
            normal: Vec2::X,
            boundary: true,
        });
    }
    if c == cols - 1 {
        faces.push(WallFace {
            center: Vec2::new(hx, center.y),
            normal: -Vec2::X,
            boundary: true,
        });
    }
    // ±Z boundary rows hold the doorway walls; those cells are almost always
    // inside gap clearance anyway, so boundary faces are only offered on ±X.
    let _ = (r, rows, hz);
    for seg in interior {
        let vertical = seg.half.y > seg.half.x;
        if vertical {
            let along = (center.y - seg.center.y).abs() <= seg.half.y + 0.01;
            if along && (seg.center.x - (center.x - half)).abs() < cell * 0.35 {
                faces.push(WallFace {
                    center: Vec2::new(seg.center.x, center.y),
                    normal: Vec2::X,
                    boundary: false,
                });
            } else if along && (seg.center.x - (center.x + half)).abs() < cell * 0.35 {
                faces.push(WallFace {
                    center: Vec2::new(seg.center.x, center.y),
                    normal: -Vec2::X,
                    boundary: false,
                });
            }
        } else {
            let along = (center.x - seg.center.x).abs() <= seg.half.x + 0.01;
            if along && (seg.center.y - (center.y - half)).abs() < cell * 0.35 {
                faces.push(WallFace {
                    center: Vec2::new(center.x, seg.center.y),
                    normal: Vec2::Y,
                    boundary: false,
                });
            } else if along && (seg.center.y - (center.y + half)).abs() < cell * 0.35 {
                faces.push(WallFace {
                    center: Vec2::new(center.x, seg.center.y),
                    normal: -Vec2::Y,
                    boundary: false,
                });
            }
        }
    }
    faces
}

/// Length of the contiguous same-kind run ending at `(c, r-1)` in column `c`.
fn run_length_back(kinds: &[ModuleKind], cols: usize, c: usize, r: usize) -> usize {
    if r == 0 {
        return 0;
    }
    let kind = kinds[(r - 1) * cols + c];
    let mut n = 0;
    let mut row = r;
    while row > 0 && kinds[(row - 1) * cols + c] == kind {
        n += 1;
        row -= 1;
    }
    n
}

fn practical_nearby(
    kinds: &[ModuleKind],
    cols: usize,
    rows: usize,
    c: usize,
    r: usize,
    sep: i32,
) -> bool {
    for dr in -sep..=sep {
        for dc in -sep..=sep {
            let (nc, nr) = (c as i32 + dc, r as i32 + dr);
            if nc >= 0
                && nr >= 0
                && (nc as usize) < cols
                && (nr as usize) < rows
                && kinds[nr as usize * cols + nc as usize] == ModuleKind::Practical
            {
                return true;
            }
        }
    }
    false
}

fn has_orthogonal(kinds: &[ModuleKind], cols: usize, rows: usize, c: usize, r: usize) -> bool {
    let kind = kinds[r * cols + c];
    [(0i32, 1i32), (0, -1), (1, 0), (-1, 0)]
        .iter()
        .any(|(dc, dr)| {
            let (nc, nr) = (c as i32 + dc, r as i32 + dr);
            nc >= 0
                && nr >= 0
                && (nc as usize) < cols
                && (nr as usize) < rows
                && kinds[nr as usize * cols + nc as usize] == kind
        })
}

/// Demote runs of `kind` shorter than `min` (counted per column, the axis runs
/// grow along) back to `Bare`.
fn demote_short_runs(
    kinds: &mut [ModuleKind],
    cols: usize,
    rows: usize,
    kind: ModuleKind,
    min: usize,
) {
    for c in 0..cols {
        let mut r = 0;
        while r < rows {
            if kinds[r * cols + c] == kind {
                let start = r;
                while r < rows && kinds[r * cols + c] == kind {
                    r += 1;
                }
                if r - start < min {
                    for row in start..r {
                        kinds[row * cols + c] = ModuleKind::Bare;
                    }
                }
            } else {
                r += 1;
            }
        }
    }
}

// --- spawning -----------------------------------------------------------------

/// One module block: an emissive/matte scaled cube under the place transform,
/// tagged for the standard teardown (and preview) paths.
fn spawn_block(
    commands: &mut Commands,
    assets: &MatchAssets,
    xform: Transform,
    preview: bool,
    local: Transform,
    mat: &Handle<StandardMaterial>,
    name: &'static str,
) {
    let mut e = commands.spawn((
        PlaceGeometry,
        DespawnOnExit(GameState::Match),
        Mesh3d(assets.placeholder_mesh.clone()),
        MeshMaterial3d(mat.clone()),
        xform.mul_transform(local),
        Name::new(name),
    ));
    if preview {
        e.insert(PassagePreview);
    }
}

/// What a module spawn needs beyond the ECS handles: the solved placements,
/// the district palette, the grid cell size, and the placement frame (identity
/// live; the doorway-alignment transform for previews — module light must be
/// preview-consistent like every other place light).
pub(crate) struct ModuleSpawn<'a> {
    pub(crate) palette: &'a style::DistrictPalette,
    pub(crate) placements: &'a [ModulePlacement],
    pub(crate) cell: f32,
    pub(crate) xform: Transform,
    pub(crate) preview: bool,
}

/// Spawn the solved modules. At most ONE shadow-casting light per hallway: the
/// first slat run's blade lamp; everything else is emissive or unshadowed fill.
pub(crate) fn spawn_hallway_modules(
    commands: &mut Commands,
    assets: &MatchAssets,
    materials: &mut Assets<StandardMaterial>,
    spawn: ModuleSpawn,
) {
    let ModuleSpawn {
        palette,
        placements,
        cell,
        xform,
        preview,
    } = spawn;
    if placements.is_empty() {
        return;
    }
    let place_in = |local: Transform| xform.mul_transform(local);
    let key_srgb = palette.key_color.to_srgba();

    let slat_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.042, 0.038),
        perceptual_roughness: 0.95,
        ..default()
    });
    let shelf_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.08, 0.06),
        perceptual_roughness: 0.9,
        ..default()
    });
    let seam_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: palette.accent * 7.0,
        perceptual_roughness: 1.0,
        ..default()
    });
    let panel_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: palette.light_color.to_linear() * 3.5,
        perceptual_roughness: 1.0,
        ..default()
    });
    let practical_mat = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(key_srgb.red, key_srgb.green * 0.8, key_srgb.blue * 0.5) * 6.0,
        perceptual_roughness: 1.0,
        ..default()
    });
    let void_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.008, 0.01, 0.014),
        perceptual_roughness: 1.0,
        ..default()
    });

    let mut shadow_budget_left = true;
    let mut prev_slat_col: Option<f32> = None;
    for p in placements {
        let fade = 1.0 - 0.65 * p.decay;
        match p.kind {
            ModuleKind::Slat => {
                let Some(face) = p.face else { continue };
                let along = Vec2::new(-face.normal.y, face.normal.x);
                for i in -2..=2 {
                    let pos = face.center + along * (i as f32 * cell / 5.2) + face.normal * 0.24;
                    let size = if face.normal.x.abs() > 0.5 {
                        Vec3::new(0.07, WALL_HEIGHT - 0.5, 0.17)
                    } else {
                        Vec3::new(0.17, WALL_HEIGHT - 0.5, 0.07)
                    };
                    spawn_block(
                        commands,
                        assets,
                        xform,
                        preview,
                        Transform::from_xyz(pos.x, (WALL_HEIGHT - 0.5) * 0.5, pos.y)
                            .with_scale(size),
                        &slat_mat,
                        "Module slat",
                    );
                }
                // One blade lamp per run: only where the run starts (a new
                // wall column), and only the first run casts shadows.
                let is_new_run = prev_slat_col != Some(face.center.x);
                prev_slat_col = Some(face.center.x);
                if is_new_run {
                    let lamp_pos = face.center + face.normal * 0.06;
                    let target = face.center + face.normal * (cell * 1.6);
                    let mut lamp = commands.spawn((
                        PlaceGeometry,
                        DespawnOnExit(GameState::Match),
                        SpotLight {
                            color: palette.key_color,
                            intensity: 1_600_000.0 * fade,
                            range: cell * 5.0,
                            radius: 0.04,
                            shadows_enabled: shadow_budget_left,
                            inner_angle: 0.55,
                            outer_angle: 1.05,
                            ..default()
                        },
                        place_in(
                            Transform::from_xyz(lamp_pos.x, 0.95, lamp_pos.y)
                                .looking_at(Vec3::new(target.x, 0.0, target.y), Vec3::Y),
                        ),
                        Name::new("Module blade lamp"),
                    ));
                    if preview {
                        lamp.insert(PassagePreview);
                    }
                    shadow_budget_left = false;
                }
            }
            ModuleKind::Seam => {
                let Some(face) = p.face else { continue };
                let pos = face.center + face.normal * 0.10;
                spawn_block(
                    commands,
                    assets,
                    xform,
                    preview,
                    Transform::from_xyz(pos.x, (WALL_HEIGHT - 0.8) * 0.5 + 0.2, pos.y)
                        .with_scale(Vec3::new(0.07, WALL_HEIGHT - 0.8, 0.07)),
                    &seam_mat,
                    "Module seam",
                );
            }
            ModuleKind::Panel => {
                spawn_block(
                    commands,
                    assets,
                    xform,
                    preview,
                    Transform::from_xyz(p.cell_center.x, WALL_HEIGHT - 0.06, p.cell_center.y)
                        .with_scale(Vec3::new(cell * 0.55, 0.06, cell * 0.55)),
                    &panel_mat,
                    "Module panel",
                );
                let mut fill = commands.spawn((
                    PlaceGeometry,
                    DespawnOnExit(GameState::Match),
                    PointLight {
                        color: palette.light_color,
                        intensity: 42_000.0 * fade,
                        range: cell * 1.9,
                        shadows_enabled: false,
                        ..default()
                    },
                    place_in(Transform::from_xyz(
                        p.cell_center.x,
                        WALL_HEIGHT - 0.35,
                        p.cell_center.y,
                    )),
                    Name::new("Module panel fill"),
                ));
                if preview {
                    fill.insert(PassagePreview);
                }
            }
            ModuleKind::Practical => {
                let Some(face) = p.face else { continue };
                let pos = face.center + face.normal * 0.28;
                spawn_block(
                    commands,
                    assets,
                    xform,
                    preview,
                    Transform::from_xyz(pos.x, 2.15, pos.y).with_scale(Vec3::new(0.18, 0.26, 0.18)),
                    &practical_mat,
                    "Module practical",
                );
                let mut lamp = commands.spawn((
                    PlaceGeometry,
                    DespawnOnExit(GameState::Match),
                    PointLight {
                        color: Color::srgb(
                            key_srgb.red,
                            key_srgb.green * 0.75,
                            key_srgb.blue * 0.45,
                        ),
                        intensity: 130_000.0 * fade,
                        range: 6.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    place_in(Transform::from_xyz(pos.x, 2.0, pos.y)),
                    Name::new("Module practical light"),
                ));
                if preview {
                    lamp.insert(PassagePreview);
                }
            }
            ModuleKind::Shelf => {
                let Some(face) = p.face else { continue };
                let along_len = cell * 0.8;
                for (i, y) in [0.6_f32, 1.3, 2.0].into_iter().enumerate() {
                    let proud = 0.16 + (i % 2) as f32 * 0.03;
                    let pos = face.center + face.normal * proud;
                    let size = if face.normal.x.abs() > 0.5 {
                        Vec3::new(0.09, 0.07, along_len)
                    } else {
                        Vec3::new(along_len, 0.07, 0.09)
                    };
                    spawn_block(
                        commands,
                        assets,
                        xform,
                        preview,
                        Transform::from_xyz(pos.x, y, pos.y).with_scale(size),
                        &shelf_mat,
                        "Module shelf",
                    );
                }
            }
            ModuleKind::Void => {
                let Some(face) = p.face else { continue };
                let pos = face.center + face.normal * 0.05;
                let size = if face.normal.x.abs() > 0.5 {
                    Vec3::new(0.06, WALL_HEIGHT - 1.2, cell * 0.7)
                } else {
                    Vec3::new(cell * 0.7, WALL_HEIGHT - 1.2, 0.06)
                };
                spawn_block(
                    commands,
                    assets,
                    xform,
                    preview,
                    Transform::from_xyz(pos.x, (WALL_HEIGHT - 1.2) * 0.5, pos.y).with_scale(size),
                    &void_mat,
                    "Module void edge",
                );
                let ambient_srgb = palette.ambient_color.to_srgba();
                let mut glow = commands.spawn((
                    PlaceGeometry,
                    DespawnOnExit(GameState::Match),
                    PointLight {
                        color: Color::srgb(
                            ambient_srgb.red * 0.8,
                            ambient_srgb.green * 0.9,
                            ambient_srgb.blue,
                        ),
                        intensity: 9_000.0,
                        range: 4.5,
                        shadows_enabled: false,
                        ..default()
                    },
                    place_in(Transform::from_xyz(
                        pos.x + face.normal.x * 0.4,
                        1.6,
                        pos.y + face.normal.y * 0.4,
                    )),
                    Name::new("Module void glow"),
                ));
                if preview {
                    glow.insert(PassagePreview);
                }
            }
            ModuleKind::Bare => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teleport::{MAZE_CELL, ThresholdSlotId, geom};
    use observed_core::RoomId;

    const CELL: f32 = MAZE_CELL;

    /// A real 6×7 grid hallway built through the production geometry path, so
    /// the solver is tested against exactly what the game hands it.
    fn test_geom() -> PlaceGeom {
        let variation = crate::hallway::TEMPLATES
            .iter()
            .position(|t| t.grid == Some((18, 21)))
            .expect("a 18x21 grid template exists");
        geom::hallway_geom_with_slots(
            geom::HallwayGeomEndpoints {
                from: RoomId(0),
                to: RoomId(1),
                from_room_slot: ThresholdSlotId(0),
                to_room_slot: ThresholdSlotId(0),
                exit_room: RoomId(99),
            },
            crate::hallway::template(variation),
            0,
            false,
        )
    }

    #[test]
    fn weight_order_matches_the_style_crate() {
        // The style crate documents [slat, seam, panel, practical, shelf,
        // void, bare]; ModuleKind::ALL must index it in exactly that order.
        assert_eq!(ModuleKind::ALL.len(), style::HALLWAY_MODULE_COUNT);
        assert_eq!(ModuleKind::ALL[0], ModuleKind::Slat);
        assert_eq!(ModuleKind::ALL[1], ModuleKind::Seam);
        assert_eq!(ModuleKind::ALL[2], ModuleKind::Panel);
        assert_eq!(ModuleKind::ALL[3], ModuleKind::Practical);
        assert_eq!(ModuleKind::ALL[4], ModuleKind::Shelf);
        assert_eq!(ModuleKind::ALL[5], ModuleKind::Void);
        assert_eq!(ModuleKind::ALL[6], ModuleKind::Bare);
    }

    #[test]
    fn the_collapse_is_deterministic() {
        let geom = test_geom();
        for district in style::District::ALL {
            let a = solve_hallway_modules(0xC0FFEE, &geom, CELL, district);
            let b = solve_hallway_modules(0xC0FFEE, &geom, CELL, district);
            assert_eq!(a, b, "{district:?}: same seed, same placements");
        }
    }

    #[test]
    fn different_seeds_compose_differently_somewhere() {
        let geom = test_geom();
        let a = solve_hallway_modules(1, &geom, CELL, style::District::Reactor);
        let b = solve_hallway_modules(2, &geom, CELL, style::District::Reactor);
        assert_ne!(a, b, "the layer varies with the hallway seed");
    }

    #[test]
    fn nothing_lands_inside_threshold_clearance() {
        let geom = test_geom();
        for district in style::District::ALL {
            for seed in 0..24u64 {
                for p in solve_hallway_modules(seed, &geom, CELL, district) {
                    for gap in &geom.gaps {
                        assert!(
                            p.cell_center.distance(gap.center) >= CELL * GAP_CLEAR_CELLS,
                            "{district:?} seed {seed}: {:?} at {} violates gap clearance",
                            p.kind,
                            p.cell_center
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn solvable_for_every_district_and_never_panics_with_zeroed_weights() {
        // Bare's +1 floor makes the collapse total non-zero even for a
        // hypothetical all-zero weight row: solvability by construction.
        let geom = test_geom();
        for district in style::District::ALL {
            for seed in 0..8u64 {
                let _ = solve_hallway_modules(seed, &geom, CELL, district);
            }
        }
    }

    #[test]
    fn slat_and_shelf_runs_respect_the_minimum() {
        let geom = test_geom();
        for district in style::District::ALL {
            for seed in 0..24u64 {
                let placements = solve_hallway_modules(seed, &geom, CELL, district);
                for (kind, min) in [
                    (ModuleKind::Slat, SLAT_RUN_MIN),
                    (ModuleKind::Shelf, SHELF_RUN_MIN),
                ] {
                    // Group by column and count contiguous rows.
                    let mut of_kind: Vec<Vec2> = placements
                        .iter()
                        .filter(|p| p.kind == kind)
                        .map(|p| p.cell_center)
                        .collect();
                    of_kind.sort_by(|a, b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());
                    let mut run = 1;
                    for w in of_kind.windows(2) {
                        if (w[0].x - w[1].x).abs() < 0.01 && (w[1].y - w[0].y - CELL).abs() < 0.01 {
                            run += 1;
                        } else {
                            assert!(
                                run >= min,
                                "{district:?} seed {seed}: {kind:?} run of {run}"
                            );
                            run = 1;
                        }
                    }
                    if !of_kind.is_empty() {
                        assert!(
                            run >= min,
                            "{district:?} seed {seed}: trailing {kind:?} run of {run}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn practicals_keep_pool_separation() {
        let geom = test_geom();
        for district in style::District::ALL {
            for seed in 0..24u64 {
                let placements = solve_hallway_modules(seed, &geom, CELL, district);
                let practicals: Vec<Vec2> = placements
                    .iter()
                    .filter(|p| p.kind == ModuleKind::Practical)
                    .map(|p| p.cell_center)
                    .collect();
                for (i, a) in practicals.iter().enumerate() {
                    for b in practicals.iter().skip(i + 1) {
                        let cells = ((*a - *b).abs() / CELL).max_element().round() as i32;
                        assert!(
                            cells > PRACTICAL_SEPARATION,
                            "{district:?} seed {seed}: practicals {cells} cells apart"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn at_most_one_void_edge_and_only_on_the_boundary() {
        let geom = test_geom();
        for district in style::District::ALL {
            for seed in 0..24u64 {
                let placements = solve_hallway_modules(seed, &geom, CELL, district);
                let voids: Vec<&ModulePlacement> = placements
                    .iter()
                    .filter(|p| p.kind == ModuleKind::Void)
                    .collect();
                assert!(
                    voids.len() <= 1,
                    "{district:?} seed {seed}: {} voids",
                    voids.len()
                );
                for v in voids {
                    assert!(
                        v.face.is_some_and(|f| f.boundary),
                        "{district:?} seed {seed}: void off the boundary"
                    );
                }
            }
        }
    }

    #[test]
    fn variety_thins_toward_the_far_end() {
        // The Rudon gradient: across seeds, the entry half must carry at
        // least as many modules as the far half — on average, strictly more.
        let geom = test_geom();
        let mut near = 0usize;
        let mut far = 0usize;
        for seed in 0..48u64 {
            for p in solve_hallway_modules(seed, &geom, CELL, style::District::Archive) {
                // Symmetric thirds (the middle band is no one's evidence).
                if p.decay < 0.4 {
                    near += 1;
                } else if p.decay > 0.6 {
                    far += 1;
                }
            }
        }
        assert!(
            near > far,
            "variety must thin with distance from the entry: near={near} far={far}"
        );
    }

    #[test]
    fn gantry_hallways_keep_their_own_language() {
        let mut geom = test_geom();
        geom.decks.push(crate::teleport::DeckSeg {
            center: Vec2::ZERO,
            half: Vec2::splat(1.0),
            top_y: 2.0,
            bottom_y: 0.0,
        });
        assert!(
            solve_hallway_modules(7, &geom, CELL, style::District::Reactor).is_empty(),
            "gantry places get no modules"
        );
    }
}
