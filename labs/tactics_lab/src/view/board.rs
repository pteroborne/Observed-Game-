//! Drawing the board.
//!
//! One pass over the lattice, batched by meaning: every cell painted the same
//! way lands in the same mesh, so a production-scale board costs six meshes
//! rather than six thousand entities. The batching primitives are
//! `observed_schematic`, shared with `iso_observer_lab`.
//!
//! The two view modes are parameters here, not separate renderers — see the note
//! in [`super`].

use bevy::prelude::*;
use observed_core::PlayerId;
use observed_hex::{HexCoord, HexFace, TILE_LEVEL_HEIGHT, hex_origin};
use observed_schematic::{
    LineBatch, SurfaceBatch, floor_ring, ramp_glyph, stair_glyph, wall_bands,
};
use observed_style::{HexSketchRole, Treatment, hex_sketch};
use observed_style::{TacticsRole, tactics};

use crate::sim::TacticsGame;
use crate::sim::unit::PLAYER_TEAM;
use crate::sim::vision;

use super::{BoardVisual, CellPaint, ViewMode, paint, sketch_role, unit_marker};
use crate::BoardGeometry;

/// Height of a unit's marker above its cell's floor.
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
        selected,
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
    let mut lines: Vec<(CellPaint, LineBatch)> = CellPaint::ALL
        .iter()
        .map(|&state| (state, LineBatch::default()))
        .collect();
    let mut map_surfaces: Vec<(CellPaint, SurfaceBatch)> = CellPaint::ALL
        .iter()
        .map(|&state| (state, SurfaceBatch::default()))
        .collect();
    for (&cell, placement) in &game.world.placements {
        if !draws_level(mode, cell, level) {
            continue;
        }
        let state = paint(game, cell);
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
            .find(|(candidate, _)| *candidate == state)
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
            HexSketchRole::Shaft => Some(stair_glyph(height)),
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
                .find(|(candidate, _)| *candidate == state)
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

    for (state, batch) in lines {
        if let Some(mesh) = batch.build() {
            commands.spawn((
                BoardVisual,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(
                    materials.add(line_material(board_treatment(mode, state), state.dimmed())),
                ),
                Transform::IDENTITY,
                Name::new(format!("Board lines: {}", state.legend())),
            ));
        }
    }
    if mode == ViewMode::Overview {
        for (state, batch) in map_surfaces {
            let Some(mesh) = batch.build() else { continue };
            let treatment = state.treatment();
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
    report.units = spawn_units(commands, meshes, materials, game, selected, mode, level);
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

fn board_treatment(mode: ViewMode, state: CellPaint) -> Treatment {
    if mode == ViewMode::Deck && matches!(state, CellPaint::Known | CellPaint::Stale) {
        tactics(TacticsRole::DevGrid)
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
    let bearing = observed_style::iso::detent_bearing(0);
    let (focus, context, detail) = observed_cutaway::build_quarter_walls(
        &game.world,
        &geometry.snapshot,
        &cells,
        selected,
        TILE_LEVEL_HEIGHT * 0.25,
        cache,
    );
    for (batch, treatment) in context
        .into_values()
        .map(|batch| (batch, tactics(TacticsRole::DevContext)))
        .chain(std::iter::once((focus, tactics(TacticsRole::DevSurface))))
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
            Name::new("Authored tactical quarter-wall deck"),
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
    detail.hulls_drawn
}

/// Units, drawn as a floating marker so they read over the line work rather than
/// inside it.
fn spawn_units(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    game: &TacticsGame,
    selected: Option<PlayerId>,
    mode: ViewMode,
    level: u8,
) -> usize {
    let mut drawn = 0;
    for unit in game.units.values().filter(|unit| !unit.escaped) {
        if !draws_level(mode, unit.cell, level) {
            continue;
        }
        let rival = unit.team != PLAYER_TEAM;
        // A rival is only drawn where the squad can actually see it. Otherwise
        // the race would be run against an opponent whose position the player is
        // simply told, which is not the game.
        if rival && !game.observation.visible_cells.contains(&unit.cell) {
            continue;
        }
        let is_selected = selected == Some(unit.id);
        let treatment = unit_marker(is_selected, rival);
        // Squad members frequently share a cell. Fan their markers out just
        // enough that selection stays legible without implying a different
        // simulation position.
        let marker_offset = if rival {
            Vec3::ZERO
        } else {
            let angle = f32::from(unit.id.0) * std::f32::consts::TAU / 6.0;
            Vec3::new(angle.cos(), 0.0, angle.sin()) * 1.8
        };
        let radius = if is_selected { 2.0 } else { 1.45 };
        commands.spawn((
            BoardVisual,
            Mesh3d(meshes.add(Sphere::new(radius).mesh().ico(2).expect("icosphere"))),
            MeshMaterial3d(materials.add(line_material(treatment, false))),
            Transform::from_translation(
                cell_origin(mode, unit.cell) + marker_offset + Vec3::Y * MARKER_HEIGHT,
            ),
            Name::new(format!("Unit {}", unit.id.0)),
        ));
        drawn += 1;
    }
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
    drawn
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
fn line_material(treatment: Treatment, dimmed: bool) -> StandardMaterial {
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
