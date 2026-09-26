//! The building on the Architect's board: the real authored tiles, as the team remembers
//! them.
//!
//! Drawn the way `architect_lab` draws its board, which is what makes that board read as a
//! place rather than a diagram: the actual hulls of each tile, ceilings dropped, the walls
//! nearest the camera taken away and the rest capped low, so every room is open to a
//! camera looking down at the isometric pitch; shaded in the architect palette
//! (`observed_style::architect::surface`), with the cut tops of walls dark so the floors
//! read first.
//!
//! **As remembered, never as it is.** A cell the team remembers exactly as it now stands
//! is drawn from the live geometry; a cell that has changed since the team saw it is
//! projected from the placement it remembers (`project_hypothetical_cell`). The board can
//! show the Architect nothing the team has not seen, including that something changed.
//! Cells the team can see now are drawn clear; cells it only remembers are hazed.
//!
//! The floor in view is drawn with the two below it as hazed context decks, so the climb
//! is under the board rather than only beside it.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexPlacement;
use observed_hex::{HexCoord, TILE_LEVEL_HEIGHT, hex_origin};
use observed_match::hex_wfc::{HexPiecePart, HexStructurePiece, project_hypothetical_cell};
use observed_style::architect::{CUT_SURFACE_MULTIPLIER, Role, hazed, surface};
use observed_style::iso::{HullRegion, detent_bearing, hull_region};
use observed_traversal::{ColliderShape, ConvexRenderMesh};

use super::ArchitectDesk;
use super::board::BOARD_LAYER;
use super::feedback;
use super::pick;
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

/// How many floors below the one in view are drawn, as context.
const CONTEXT_FLOORS: u8 = 2;
/// Walls are capped this high above their floor, and the near ones lower still, so a
/// room is open to the camera and its doorways still read.
const WALL_CAP: f32 = 2.6;
const NEAR_WALL_CAP: f32 = 1.5;

/// How a cell is drawn: clear where the team is looking, hazed where it only remembers,
/// and further hazed a floor or two below the one in view.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Tone {
    Seen,
    Remembered,
    Below(u8),
}

impl Tone {
    /// The material a tile's floors (`floor`) or walls take in this tone, as the lab
    /// shades them: lit concrete in view, the same concrete dimmed where only remembered
    /// (dimmed rather than hazed, so its district's hue survives), and a flat unlit
    /// silhouette in the context colour a floor or two below.
    fn material(self, register: ArchitectureRegister, floor: bool) -> StandardMaterial {
        let lit = |gain: f32| StandardMaterial {
            base_color: Color::LinearRgba(surface(register, floor).to_linear() * gain),
            perceptual_roughness: 0.9,
            ..default()
        };
        match self {
            Self::Seen => lit(1.0),
            Self::Remembered => lit(0.62),
            Self::Below(depth) => StandardMaterial {
                base_color: hazed(
                    Role::Context,
                    if depth > 1 { 0.55 } else { 0.3 } - if floor { 0.0 } else { 0.12 },
                ),
                unlit: true,
                ..default()
            },
        }
    }
}

#[derive(Component)]
pub(super) struct BuiltCell;

/// What is drawn, so a cell is rebuilt only when what it should show changes.
#[derive(Resource, Default)]
pub(super) struct Building {
    /// Each room drawn: its entity, what it was drawn from, and the placement it shows.
    drawn: BTreeMap<HexCoord, (Entity, u64, HexPlacement)>,
    /// The Architect's own builds that have built in, by cell and tick, so each does once.
    celebrated: BTreeSet<(HexCoord, u64)>,
    materials: HashMap<(ArchitectureRegister, bool, Tone), Handle<StandardMaterial>>,
    /// The live geometry's pieces by the cell that owns them, for one generation.
    index: Option<(u32, HashMap<HexCoord, Vec<usize>>)>,
}

pub(super) fn draw(
    mut commands: Commands,
    desk: Res<ArchitectDesk>,
    runtime: Res<HexWfcRuntime>,
    mut building: ResMut<Building>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let rules = ascent.rules();
    let Some(knowledge) = rules.team_knowledge.get(&desk.team) else {
        return;
    };
    let physical = &runtime.match_state;
    let generation = physical.geometry.generation;
    if building
        .index
        .as_ref()
        .is_none_or(|(at, _)| *at != generation)
    {
        let mut index: HashMap<HexCoord, Vec<usize>> = HashMap::new();
        for (i, piece) in physical.geometry.pieces.iter().enumerate() {
            index.entry(piece.source_cell).or_default().push(i);
        }
        building.index = Some((generation, index));
    }

    // What should be drawn, and how.
    let lowest = desk.floor.saturating_sub(CONTEXT_FLOORS);
    let mut wanted: BTreeMap<HexCoord, (Tone, HexPlacement)> = BTreeMap::new();
    for (&cell, known) in &knowledge.cells {
        let Some(placement) = desk.believed(cell, Some(known)) else {
            continue;
        };
        if cell.level > desk.floor || cell.level < lowest || !placement.space.built() {
            continue;
        }
        let tone = if cell.level < desk.floor {
            Tone::Below(desk.floor - cell.level)
        } else if knowledge.visible_cells.contains(&cell) {
            Tone::Seen
        } else {
            Tone::Remembered
        };
        // A room is drawn whole from its anchor, which owns all its pieces.
        let owner = physical
            .facility
            .blueprints
            .iter()
            .find(|blueprint| blueprint.cells.contains(&cell))
            .map_or(cell, |blueprint| blueprint.anchor);
        let entry = wanted.entry(owner).or_insert((tone, placement));
        entry.0 = entry.0.min(tone);
    }

    // Retire what should no longer be there.
    let gone: Vec<HexCoord> = building
        .drawn
        .keys()
        .filter(|cell| !wanted.contains_key(cell))
        .copied()
        .collect();
    for cell in gone {
        if let Some((entity, ..)) = building.drawn.remove(&cell) {
            commands.entity(entity).despawn();
        }
    }

    let bearing = bearing();
    let layer = RenderLayers::layer(BOARD_LAYER);
    for (cell, (tone, remembered)) in wanted {
        let current = physical.facility.placements.get(&cell).copied();
        let as_it_is = current == Some(remembered);
        let signature = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::hash::DefaultHasher::new();
            (tone, as_it_is, remembered.doors, remembered.archetype as u8).hash(&mut hasher);
            if as_it_is {
                generation.hash(&mut hasher);
            }
            hasher.finish()
        };
        if building
            .drawn
            .get(&cell)
            .is_some_and(|(_, drawn, _)| *drawn == signature)
        {
            continue;
        }
        let before = building.drawn.remove(&cell).map(|(entity, _, before)| {
            commands.entity(entity).despawn();
            before
        });
        // Whether it builds in: only when what stands there has changed.
        let own = desk.built.get(&cell).copied();
        let glow = feedback::build_glow(
            own.map(|(placement, _)| placement),
            own.is_some_and(|(_, at)| building.celebrated.contains(&(cell, at))),
            before,
            remembered,
        );
        if glow == Some(Role::Selected)
            && let Some((_, at)) = own
        {
            building.celebrated.insert((cell, at));
        }
        let pieces: Vec<HexStructurePiece> = if as_it_is {
            building
                .index
                .as_ref()
                .and_then(|(_, index)| index.get(&cell))
                .map(|found| {
                    found
                        .iter()
                        .map(|&i| physical.geometry.pieces[i].clone())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            // Changed since the team saw it: draw what it remembers.
            project_hypothetical_cell(
                &physical.facility,
                cell,
                remembered,
                physical.content().cells(),
            )
            .unwrap_or_default()
        };
        let register = physical
            .facility
            .architecture
            .get(&cell)
            .copied()
            .unwrap_or(ArchitectureRegister::ALL[0]);
        let root = commands
            .spawn((
                BuiltCell,
                DespawnOnExit(GameState::HexWfc),
                if glow.is_some() {
                    feedback::start()
                } else {
                    Transform::from_translation(pick::BOARD_ORIGIN)
                },
                Visibility::default(),
                layer.clone(),
            ))
            .id();
        let glow = glow.map(|role| (role, materials.add(feedback::glow_material(role))));
        let mut glows = Vec::new();
        for floor in [true, false] {
            let Some(mesh) = cutaway_mesh(&pieces, floor, bearing) else {
                continue;
            };
            let material = building
                .materials
                .entry((register, floor, tone))
                .or_insert_with(|| materials.add(tone.material(register, floor)))
                .clone();
            let mesh = meshes.add(mesh);
            let child = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    layer.clone(),
                ))
                .id();
            commands.entity(root).add_child(child);
            if let Some((_, glow)) = &glow {
                // The same room again, a hair proud of it, in light.
                let lit = commands
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(glow.clone()),
                        Transform::from_xyz(0.0, 0.02, 0.0),
                        layer.clone(),
                    ))
                    .id();
                commands.entity(root).add_child(lit);
                glows.push(lit);
            }
        }
        if let Some((role, glow)) = glow {
            commands.entity(root).insert(feedback::BuildIn {
                age: 0.0,
                glow,
                glows,
                own: role == Role::Selected,
            });
        }
        building.drawn.insert(cell, (root, signature, remembered));
    }
}

/// A tile's floors (`floor`) or walls in `register`'s concrete, lit, as a room in view.
#[must_use]
pub(super) fn tile_material(register: ArchitectureRegister, floor: bool) -> StandardMaterial {
    Tone::Seen.material(register, floor)
}

/// The bearing the board is looked at from, which decides which walls are cut away.
#[must_use]
pub(super) fn bearing() -> Vec2 {
    detent_bearing(0)
}

/// Forget everything drawn: a new floor, or a new match.
pub(super) fn clear(
    mut commands: Commands,
    mut building: ResMut<Building>,
    desk: Res<ArchitectDesk>,
    mut floor: Local<Option<u8>>,
) {
    if *floor == Some(desk.floor) {
        return;
    }
    *floor = Some(desk.floor);
    for (_, (entity, ..)) in std::mem::take(&mut building.drawn) {
        commands.entity(entity).despawn();
    }
}

/// One mesh of the floor hulls (`floor`) or of everything standing on them, cut away for
/// a camera on `bearing`: ceilings dropped, near walls dropped, the rest capped low, the
/// cut tops of walls dark.
pub(super) fn cutaway_mesh(
    pieces: &[HexStructurePiece],
    floor: bool,
    bearing: Vec2,
) -> Option<Mesh> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for piece in pieces {
        if !piece.part.drawn() || matches!(piece.part, HexPiecePart::Glazing) {
            continue;
        }
        let world = world_points(piece);
        if world.is_empty() {
            continue;
        }
        let centroid = world.iter().copied().sum::<Vec3>() / world.len() as f32;
        let Some(cell) = pick::cell_at_level(centroid, piece.source_cell.level) else {
            continue;
        };
        let origin = Vec3::from_array(hex_origin(cell));
        let base = f32::from(cell.level) * TILE_LEVEL_HEIGHT;
        let min_y = world.iter().map(|p| p.y).fold(f32::INFINITY, f32::min) - base;
        let max_y = world.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max) - base;
        let local = centroid - origin;
        let region = hull_region(min_y, max_y, local);
        let is_floor = matches!(region, HullRegion::Floor);
        if is_floor != floor || matches!(region, HullRegion::Ceiling) {
            continue;
        }
        let near = Vec2::new(local.x, local.z).dot(bearing) > 0.0;
        if matches!(region, HullRegion::Perimeter) && near {
            continue;
        }
        let cap = base + if near { NEAR_WALL_CAP } else { WALL_CAP };
        let capped: Vec<Vec3> = world
            .iter()
            .map(|p| {
                if is_floor {
                    *p
                } else {
                    Vec3::new(p.x, p.y.min(cap), p.z)
                }
            })
            .collect();
        let Some(render) = ConvexRenderMesh::from_convex_hull(&capped) else {
            continue;
        };
        let first = u32::try_from(positions.len()).unwrap_or(u32::MAX);
        for (position, normal) in render.positions.iter().zip(&render.normals) {
            positions.push(*position);
            normals.push(*normal);
            colors.push(if !is_floor && normal[1] > 0.5 {
                CUT_SURFACE_MULTIPLIER
            } else {
                [1.0; 4]
            });
        }
        indices.extend(render.indices.iter().map(|i| i + first));
    }
    if indices.is_empty() {
        return None;
    }
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

/// A piece's hull corners in world space.
fn world_points(piece: &HexStructurePiece) -> Vec<Vec3> {
    let rotation = Quat::from_array(piece.rotation);
    let local: Vec<Vec3> = match &piece.shape {
        ColliderShape::ConvexHull { points } => points.clone(),
        ColliderShape::Cuboid { half } => (0..8u8)
            .map(|i| {
                Vec3::new(
                    if i & 1 == 0 { -half.x } else { half.x },
                    if i & 2 == 0 { -half.y } else { half.y },
                    if i & 4 == 0 { -half.z } else { half.z },
                )
            })
            .collect(),
    };
    local
        .into_iter()
        .map(|p| piece.center + rotation * p)
        .collect()
}
