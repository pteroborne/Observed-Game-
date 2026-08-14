//! Drawing the board.
//!
//! One pass over the lattice, batched by meaning: every cell painted the same
//! way lands in the same mesh, so a production-scale board costs six meshes
//! rather than six thousand entities. The batching primitives are
//! `observed_schematic`, shared with `iso_observer_lab`.
//!
//! The two view modes are parameters here, not separate renderers — see the note
//! in [`super`].

use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_core::PlayerId;
use observed_hex::{HexCoord, HexFace, TILE_LEVEL_HEIGHT, hex_origin};
use observed_schematic::{
    LineBatch, SurfaceBatch, floor_ring, level_arrow_glyph, ramp_glyph, wall_bands,
};
use observed_style::{HexSketchRole, Treatment, hex_sketch};
use observed_style::{TacticsRole, tactics};

use crate::sim::TacticsGame;
use crate::sim::unit::PLAYER_TEAM;
use crate::sim::vision;

use super::{BoardVisual, CellPaint, ViewMode, paint, sketch_role};
use crate::BoardGeometry;

/// Height of the Guardian marker above its cell's floor.
const MARKER_HEIGHT: f32 = 7.0;
/// Wall bands are drawn low: a floor plan is read from above, and a waist-high
/// solid says "you cannot pass here" without occluding the cell behind it.
const WALL_FRACTION: f32 = 1.0 / 3.0;
/// What one rebuild drew. Reported in the status line and asserted in tests.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawReport {
    pub cells: usize,
    pub hidden: usize,
    pub segments: usize,
    pub walls: usize,
    pub units: usize,
    pub detail_hulls: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct BoardView {
    pub selected: Option<PlayerId>,
    pub mode: ViewMode,
    pub level: u8,
}

/// Rebuild the whole board.
///
/// Everything it spawns carries [`BoardVisual`], so the caller clears a board by
/// despawning that one marker — the reset contract every lab in this workspace
/// keeps.
pub fn build(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    game: &TacticsGame,
    geometry: Option<&BoardGeometry>,
    cache: &mut observed_cutaway::TileMeshCache,
    board_view: BoardView,
) -> DrawReport {
    let BoardView {
        selected: _,
        mode,
        level,
    } = board_view;
    let mut report = DrawReport::default();
    if mode == ViewMode::Deck
        && let Some(geometry) = geometry
    {
        report.detail_hulls = spawn_authored_deck(
            commands, meshes, materials, game, geometry, cache, board_view,
        );
    }
    let mut lines: Vec<((CellPaint, ArchitectureRegister), LineBatch)> = CellPaint::ALL
        .iter()
        .flat_map(|&state| {
            ArchitectureRegister::ALL
                .into_iter()
                .map(move |register| ((state, register), LineBatch::default()))
        })
        .collect();
    let mut map_surfaces: Vec<((CellPaint, ArchitectureRegister), SurfaceBatch)> = CellPaint::ALL
        .iter()
        .flat_map(|&state| {
            ArchitectureRegister::ALL
                .into_iter()
                .map(move |register| ((state, register), SurfaceBatch::default()))
        })
        .collect();
    for (&cell, placement) in &game.world.placements {
        if !draws_level(mode, cell, level) {
            continue;
        }
        let state = paint(game, cell);
        let register = game
            .world
            .architecture
            .get(&cell)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);
        if state == CellPaint::Unknown {
            // Fog is drawn by *not drawing*. An outline for "something is here"
            // would be a map of the facility's shape, which is the thing the
            // player is meant to be discovering.
            report.hidden += 1;
            continue;
        }
        let role = sketch_role(
            placement.archetype,
            placement.space,
            game.world.room_id_at(cell).is_some(),
        );
        if role == HexSketchRole::Void {
            continue;
        }
        let sketch = hex_sketch(role);
        let Some(height) = sketch.height else {
            continue;
        };
        let origin = cell_origin(mode, cell)
            + if mode == ViewMode::Deck {
                Vec3::Y * 0.65
            } else {
                Vec3::ZERO
            };
        report.cells += 1;

        let line = &mut lines
            .iter_mut()
            .find(|((candidate_state, candidate_register), _)| {
                *candidate_state == state && *candidate_register == register
            })
            .expect("every paint state has a batch")
            .1;
        if mode == ViewMode::Deck {
            for (from, to) in floor_ring(sketch.inset) {
                line.segment(origin + from, origin + to);
                report.segments += 1;
            }
        }
        // The glyphs are how a flat view says "the floor changes here" without
        // the reader counting edges or selecting anything.
        let vertical = match role {
            HexSketchRole::Shaft => Some(level_arrow_glyph(
                placement.is_open(HexFace::Up),
                placement.is_open(HexFace::Down),
            )),
            HexSketchRole::Ramp => Some(ramp_glyph(height)),
            _ => None,
        };
        for (from, to) in vertical.into_iter().flatten() {
            line.segment(origin + from, origin + to);
            report.segments += 1;
        }

        let open = HexFace::LATERAL.map(|face| placement.is_open(face));
        if mode == ViewMode::Overview {
            let surface = &mut map_surfaces
                .iter_mut()
                .find(|((candidate_state, candidate_register), _)| {
                    *candidate_state == state && *candidate_register == register
                })
                .expect("every paint state has a map surface")
                .1;
            add_map_floor(surface, origin, sketch.inset);
            for quad in wall_bands(height * WALL_FRACTION, open) {
                // A top-down map sees the wall's top edge. Drawing the old
                // vertical band edge-on made openings indistinguishable from
                // missing geometry; these segments preserve real doorway gaps.
                line.segment(origin + quad[2], origin + quad[3]);
                report.segments += 1;
                report.walls += 1;
            }
        }
    }

    for ((state, register), batch) in lines {
        if let Some(mesh) = batch.build() {
            commands.spawn((
                BoardVisual,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(line_material(
                    board_treatment(state, register),
                    state.dimmed(),
                ))),
                Transform::IDENTITY,
                Name::new(format!(
                    "Board lines: {} / {}",
                    state.legend(),
                    register.slug()
                )),
            ));
        }
    }
    if mode == ViewMode::Overview {
        for ((state, register), batch) in map_surfaces {
            let Some(mesh) = batch.build() else { continue };
            let treatment = board_treatment(state, register);
            let alpha = if state.dimmed() { 0.035 } else { 0.10 };
            commands.spawn((
                BoardVisual,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: treatment.base_color.with_alpha(alpha),
                    emissive: treatment.emissive * alpha,
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    cull_mode: None,
                    double_sided: true,
                    ..default()
                })),
                Name::new(format!("Map floor: {}", state.legend())),
            ));
        }
    }
    report.units = visible_unit_count(game, mode, level);
    spawn_guardian(commands, meshes, materials, game, mode, level);
    spawn_objectives(commands, meshes, materials, game, mode, level);
    report
}

fn add_map_floor(batch: &mut SurfaceBatch, origin: Vec3, inset: f32) {
    let vertices: Vec<Vec3> = floor_ring(inset)
        .into_iter()
        .map(|(from, _)| origin + from)
        .collect();
    for index in 0..vertices.len() {
        batch.triangle(
            origin,
            vertices[index],
            vertices[(index + 1) % vertices.len()],
        );
    }
}

fn board_treatment(state: CellPaint, register: ArchitectureRegister) -> Treatment {
    if state == CellPaint::Known {
        observed_style::architecture_tactical(register)
    } else {
        state.treatment()
    }
}

fn spawn_authored_deck(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    game: &TacticsGame,
    geometry: &BoardGeometry,
    cache: &mut observed_cutaway::TileMeshCache,
    board_view: BoardView,
) -> usize {
    let BoardView {
        selected, level, ..
    } = board_view;
    let cells = game
        .world
        .placements
        .keys()
        .filter(|cell| cell.level == level && paint(game, **cell) != CellPaint::Unknown)
        .copied()
        .collect();
    let selected = selected
        .and_then(|id| game.units.get(&id))
        .filter(|unit| unit.cell.level == level)
        .map(|unit| unit.cell);
    let focus_treatment = selected
        .and_then(|cell| game.world.architecture.get(&cell).copied())
        .map(observed_style::architecture_tactical)
        .unwrap_or_else(|| tactics(TacticsRole::DevSurface));
    let bearing = observed_style::iso::detent_bearing(0);
    let (focus, context, detail) = observed_cutaway::build_low_walls(
        &game.world,
        &geometry.snapshot,
        &cells,
        selected,
        TILE_LEVEL_HEIGHT / 3.0,
        cache,
    );
    for (batch, treatment) in context
        .into_iter()
        .map(|(register, batch)| (batch, observed_style::architecture_tactical(register)))
        .chain(std::iter::once((focus, focus_treatment)))
    {
        let Some(mesh) = batch.into_mesh() else {
            continue;
        };
        commands.spawn((
            BoardVisual,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: treatment.base_color,
                emissive: treatment.emissive,
                perceptual_roughness: 0.82,
                metallic: 0.04,
                cull_mode: None,
                double_sided: true,
                ..default()
            })),
            Name::new("Authored tactical third-wall deck"),
        ));
    }
    let bearing3 = Vec3::new(bearing.x, 0.0, bearing.y);
    let key_from =
        Quat::from_rotation_y(observed_style::iso::light::KEY_OFFSET) * bearing3 + Vec3::Y * 1.15;
    commands.spawn((
        BoardVisual,
        DirectionalLight {
            illuminance: observed_style::iso::light::KEY_ILLUMINANCE,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(key_from).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Tactical deck key"),
    ));
    spawn_district_lights(commands, game, level);
    detail.hulls_drawn
}

/// One local practical pool per visible architecture register. A tactical view
/// may contain several districts simultaneously, so a single global fog/ambient
/// palette would necessarily lie about all but one of them.
fn spawn_district_lights(commands: &mut Commands, game: &TacticsGame, level: u8) {
    let mut centres: BTreeMap<ArchitectureRegister, (Vec3, u16)> = BTreeMap::new();
    for (&cell, placement) in &game.world.placements {
        if cell.level != level
            || placement.archetype == observed_facility::hex_wfc::HexArchetype::Void
            || paint(game, cell) == CellPaint::Unknown
        {
            continue;
        }
        let register = game
            .world
            .architecture
            .get(&cell)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);
        let entry = centres.entry(register).or_insert((Vec3::ZERO, 0));
        entry.0 += cell_origin(ViewMode::Deck, cell);
        entry.1 = entry.1.saturating_add(1);
    }
    for (register, (sum, count)) in centres {
        let centre = sum / f32::from(count.max(1));
        let palette = observed_style::architecture(register);
        commands.spawn((
            BoardVisual,
            PointLight {
                color: palette.light_color,
                intensity: observed_style::iso::light::PRACTICAL_INTENSITY,
                range: observed_style::iso::light::PRACTICAL_RANGE,
                radius: observed_style::iso::light::PRACTICAL_RADIUS,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(
                centre + Vec3::Y * observed_style::iso::light::PRACTICAL_LIFT,
            ),
            Name::new(format!("District practical: {}", register.slug())),
        ));
    }
}

fn visible_unit_count(game: &TacticsGame, mode: ViewMode, level: u8) -> usize {
    game.units
        .values()
        .filter(|unit| {
            !unit.escaped
                && draws_level(mode, unit.cell, level)
                && (unit.team == PLAYER_TEAM || game.observation.visible_cells.contains(&unit.cell))
        })
        .count()
}

fn spawn_guardian(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    game: &TacticsGame,
    mode: ViewMode,
    level: u8,
) {
    if let Some(guardian) = game.guardian
        && draws_level(mode, guardian.cell, level)
        && game.observation.visible_cells.contains(&guardian.cell)
    {
        commands.spawn((
            BoardVisual,
            Mesh3d(meshes.add(Sphere::new(2.0).mesh().ico(2).expect("icosphere"))),
            MeshMaterial3d(materials.add(line_material(
                observed_style::marker(observed_style::MarkerRole::Director),
                false,
            ))),
            Transform::from_translation(cell_origin(mode, guardian.cell) + Vec3::Y * MARKER_HEIGHT),
            Name::new("Guardian"),
        ));
    }
}

/// The exit, and whatever objectives the squad still owes.
fn spawn_objectives(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    game: &TacticsGame,
    mode: ViewMode,
    level: u8,
) {
    let exit = game.world.config.exit();
    let mut targets = vec![(exit, observed_style::MarkerRole::Exit)];
    targets.extend(
        game.observation
            .objective_cells
            .iter()
            .map(|&cell| (cell, observed_style::MarkerRole::NextRoom)),
    );
    for (cell, role) in targets {
        if !draws_level(mode, cell, level) {
            continue;
        }
        // An objective the squad has not found yet stays hidden, like everything
        // else it has not found.
        if paint(game, cell) == CellPaint::Unknown {
            continue;
        }
        commands.spawn((
            BoardVisual,
            Mesh3d(meshes.add(Torus::new(2.2, 2.8).mesh().build())),
            MeshMaterial3d(materials.add(line_material(observed_style::marker(role), false))),
            Transform::from_translation(cell_origin(mode, cell) + Vec3::Y * 1.0),
            Name::new(format!("Objective {role:?}")),
        ));
    }
}

/// Where a cell is drawn.
///
/// The flat view collapses every level onto one plane because it only ever draws
/// one; the isometric view lifts them apart so a stack reads as a stack.
#[must_use]
pub fn cell_origin(mode: ViewMode, cell: HexCoord) -> Vec3 {
    let [x, _, z] = hex_origin(cell);
    let y = match mode {
        ViewMode::Overview => 0.0,
        ViewMode::Deck => hex_origin(cell)[1],
    };
    Vec3::new(x, y, z)
}

/// Whether this cell belongs in the current drawing.
#[must_use]
pub fn draws_level(mode: ViewMode, cell: HexCoord, level: u8) -> bool {
    match mode {
        ViewMode::Overview | ViewMode::Deck => cell.level == level,
    }
}

/// A schematic line is its own light source; shading it would only muddy the
/// read, and a ribbon standing in for a line has no meaningful back face.
pub(crate) fn line_material(treatment: Treatment, dimmed: bool) -> StandardMaterial {
    let scale = if dimmed { 0.22 } else { 1.0 };
    StandardMaterial {
        base_color: treatment.base_color,
        emissive: LinearRgba::rgb(
            treatment.emissive.red * scale,
            treatment.emissive.green * scale,
            treatment.emissive.blue * scale,
        ),
        unlit: true,
        cull_mode: None,
        double_sided: true,
        ..default()
    }
}

/// The cell under a click, given a ray from the camera.
///
/// Pointer-first input is a requirement, not a convenience — the mobile note in
/// the plan — so picking is part of the view's job rather than something the
/// keyboard path gets to skip.
#[must_use]
pub fn cell_at_ray(
    game: &TacticsGame,
    mode: ViewMode,
    level: u8,
    origin: Vec3,
    direction: Vec3,
) -> Option<HexCoord> {
    let mut best: Option<(f32, HexCoord)> = None;
    for &cell in game.world.placements.keys() {
        if !draws_level(mode, cell, level) || !vision::is_solid_space(&game.world, cell) {
            continue;
        }
        let centre = cell_origin(mode, cell);
        // Distance from the cell centre to the ray, which is a good enough pick
        // for hexes this size and needs no mesh to be present.
        let to_centre = centre - origin;
        let along = to_centre.dot(direction);
        if along <= 0.0 {
            continue;
        }
        let closest = origin + direction * along;
        let miss = closest.distance(centre);
        if miss > 6.5 {
            continue;
        }
        if best.is_none_or(|(best_miss, _)| miss < best_miss) {
            best = Some((miss, cell));
        }
    }
    best.map(|(_, cell)| cell)
}
