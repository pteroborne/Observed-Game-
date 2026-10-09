//! Instances resident portions of the authoritative hex geometry snapshot: one render
//! mesh per collider piece, district-tinted by role and source-cell architecture. The
//! simulation owns the complete snapshot; this module keeps only lightweight indices
//! into it and projects requested cell parents on demand. The outer boundary shell is
//! small and remains resident for the whole match.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexCoord, HexWfcWorld};
use observed_hex::hex_origin;
use observed_match::hex_wfc::{
    HexLightSource, HexStructurePiece, HexStructureRole, HexTrimPiece, HexWfcGeometrySnapshot,
    derive_trim_for,
};

use super::HexWfcGeometry;
use super::assets::HexWfcVisualAssets;
use super::fixtures::{PracticalProjection, spawn_cell_practicals};
use super::seams::spawn_trim;
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

mod catalog;
#[cfg(test)]
pub(super) use catalog::CellGeometryIndex;
pub(super) use catalog::HexGeometryCatalog;
use catalog::cell_footprint;

/// Result returned to the residency owner after one cell parent is projected. The
/// entity is a transient presentation handle; the stable key remains [`HexCoord`].
pub(super) struct SpawnedCell {
    pub(super) coord: HexCoord,
    pub(super) entity: Entity,
    pub(super) child_pieces: usize,
    pub(super) needs_decoration: bool,
}

pub(super) fn spawn_boundary(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    runtime: &HexWfcRuntime,
    catalog: &HexGeometryCatalog,
) {
    let world = &runtime.match_state.facility;
    let fallback_arch = *world
        .architecture
        .get(&world.config.spawn())
        .unwrap_or(&ArchitectureRegister::ALL[0]);
    for (hull_index, &piece_index) in catalog.boundary_piece_ids.iter().enumerate() {
        if let Some(piece) = runtime.match_state.geometry.piece(piece_index) {
            spawn_piece(
                commands,
                assets,
                meshes,
                piece,
                fallback_arch,
                None,
                hull_index,
            );
        }
    }
}

/// Group derived seam-trim pieces by their owning cell so each cell's parent
/// entity can own its trim (and a changed-cell rebuild regenerates only the
/// affected cells' trim).
fn group_trim_by_cell(trim: &[HexTrimPiece]) -> BTreeMap<HexCoord, Vec<&HexTrimPiece>> {
    let mut by_cell: BTreeMap<HexCoord, Vec<&HexTrimPiece>> = BTreeMap::new();
    for piece in trim {
        by_cell.entry(piece.cell).or_default().push(piece);
    }
    by_cell
}

pub(super) fn spawn_cells(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    (world, geometry): (&HexWfcWorld, &HexWfcGeometrySnapshot),
    catalog: &HexGeometryCatalog,
    requested: &BTreeSet<HexCoord>,
) -> Vec<SpawnedCell> {
    spawn_cells_bounded(
        commands,
        assets,
        meshes,
        (world, geometry),
        catalog,
        &requested.iter().copied().collect::<Vec<_>>(),
        None,
    )
}

pub(super) struct SpawnBudget<'a> {
    pub(super) time: std::time::Duration,
    pub(super) retained: &'a BTreeSet<HexCoord>,
}

pub(super) fn spawn_cells_bounded(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    (world, geometry): (&HexWfcWorld, &HexWfcGeometrySnapshot),
    catalog: &HexGeometryCatalog,
    requested: &[HexCoord],
    budget: Option<SpawnBudget<'_>>,
) -> Vec<SpawnedCell> {
    let fallback_arch = *world
        .architecture
        .get(&world.config.spawn())
        .unwrap_or(&ArchitectureRegister::ALL[0]);
    // Trim is derived only for the cells entering residency this frame. In particular,
    // off-screen relayout cells never pay a presentation rebuild cost.
    let trim = derive_trim_for(geometry, &requested.iter().copied().collect());
    let mut trim_by_cell = group_trim_by_cell(&trim);
    let mut spawned = Vec::with_capacity(requested.len());
    let started = std::time::Instant::now();
    if budget.is_some() {
        assets.poll_prepared_meshes(meshes);
    }
    for (visited, &coord) in requested.iter().enumerate() {
        if visited > 0
            && budget
                .as_ref()
                .is_some_and(|limit| started.elapsed() >= limit.time)
        {
            break;
        }
        if budget.is_some() && !assets.cell_can_retry(coord, geometry.generation) {
            continue;
        }
        let Some(index) = catalog.cells.get(&coord) else {
            continue;
        };
        let pieces: Vec<_> = index
            .piece_ids
            .iter()
            .filter_map(|&piece_index| geometry.piece(piece_index))
            .collect();
        if budget.is_some() {
            let key = cell_mesh_key(&pieces);
            if !assets.request_cell_meshes(key.as_deref(), &super::mesh_group::gather(&pieces)) {
                continue;
            }
        }
        let lights = index.lights.iter().collect();
        assets.preparing_cell = budget.is_some();
        assets.missing_meshes = false;
        let mut cell = spawn_cell(
            commands,
            assets,
            meshes,
            CellProjection {
                coord,
                pieces,
                lights,
                trim: trim_by_cell.remove(&coord).unwrap_or_default(),
            },
            world,
            fallback_arch,
        );
        assets.preparing_cell = false;
        assets.finish_cell_attempt(coord, geometry.generation);
        if assets.missing_meshes && budget.as_ref().is_some_and(|b| b.retained.contains(&coord)) {
            // Keep an existing projection until its replacement is complete.
            commands.entity(cell.entity).despawn();
        } else {
            // A new cell's complete structural shell must not wait on books or
            // other decoration. Schedule its finished dressing as a later swap.
            cell.needs_decoration = assets.missing_meshes;
            spawned.push(cell);
        }
    }
    spawned
}

struct CellProjection<'a> {
    coord: HexCoord,
    pieces: Vec<&'a HexStructurePiece>,
    lights: Vec<&'a HexLightSource>,
    trim: Vec<&'a HexTrimPiece>,
}

fn spawn_cell(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    projection: CellProjection<'_>,
    world: &observed_facility::hex_wfc::HexWfcWorld,
    fallback_arch: ArchitectureRegister,
) -> SpawnedCell {
    let CellProjection {
        coord,
        pieces,
        lights,
        trim,
    } = projection;
    let architecture = *world.architecture.get(&coord).unwrap_or(&fallback_arch);
    let cell_role = pieces
        .first()
        .map_or(HexStructureRole::Hall, |piece| piece.role);
    let composition = super::lighting::composition_at(world, coord);
    let footprint = cell_footprint(world, coord);
    let cell = commands
        .spawn((
            HexWfcGeometry,
            DespawnOnExit(GameState::HexWfc),
            Transform::IDENTITY,
            Visibility::default(),
            Name::new(format!(
                "Hex cell q{} r{} L{}",
                coord.q, coord.r, coord.level
            )),
        ))
        .id();
    let reservoir = world.placements.get(&coord).is_some_and(|p| {
        matches!(
            p.archetype,
            observed_facility::hex_wfc::HexArchetype::Cistern { .. }
        )
    });
    if reservoir {
        commands
            .entity(cell)
            .insert(super::cistern::ReservoirCell(coord));
    }
    let chargeworks = world
        .placements
        .get(&coord)
        .and_then(|p| match p.archetype {
            observed_facility::hex_wfc::HexArchetype::Chargeworks { part, heading } => {
                Some((part, heading))
            }
            _ => None,
        });
    let archive = world
        .placements
        .get(&coord)
        .and_then(|p| match p.archetype {
            observed_facility::hex_wfc::HexArchetype::ArchiveWell { heading } => Some(heading),
            _ => None,
        });
    let rain = world
        .placements
        .get(&coord)
        .and_then(|p| match p.archetype {
            observed_facility::hex_wfc::HexArchetype::RainCourt { heading } => Some(heading),
            _ => None,
        });
    let concourse = world
        .placements
        .get(&coord)
        .and_then(|p| match p.archetype {
            observed_facility::hex_wfc::HexArchetype::SwitchingConcourse { heading } => {
                Some(heading)
            }
            _ => None,
        });
    let jade = world
        .placements
        .get(&coord)
        .and_then(|p| match p.archetype {
            observed_facility::hex_wfc::HexArchetype::JadeNave { heading } => Some(heading),
            _ => None,
        });
    let promenade = world
        .placements
        .get(&coord)
        .and_then(|p| match p.archetype {
            observed_facility::hex_wfc::HexArchetype::LastPromenade { heading } => Some(heading),
            _ => None,
        });
    let wonder = if promenade.is_some() {
        Some(super::lighting::WonderLighting::Promenade)
    } else if jade.is_some() {
        Some(super::lighting::WonderLighting::Jade)
    } else if concourse.is_some() {
        Some(super::lighting::WonderLighting::Concourse)
    } else if rain.is_some() {
        Some(super::lighting::WonderLighting::Rain)
    } else if archive.is_some() {
        Some(super::lighting::WonderLighting::Archive)
    } else if chargeworks.is_some() {
        Some(super::lighting::WonderLighting::Chargeworks)
    } else if reservoir {
        Some(super::lighting::WonderLighting::Cistern)
    } else {
        None
    };
    let decoration = if architecture == ArchitectureRegister::LiminalGrid && wonder.is_none() {
        super::backrooms::spawn(commands, assets, meshes, cell, coord, &pieces)
    } else {
        super::backrooms::Decoration::default()
    };
    let mut child_pieces = decoration.meshes
        + spawn_cell_practicals(
            commands,
            assets,
            meshes,
            PracticalProjection {
                parent: cell,
                coord,
                footprint: &footprint,
                architecture,
                role: cell_role,
                composition,
                authored_lights: &lights,
                pieces: &pieces,
                fluorescent_field: decoration.fluorescent_field,
                wonder,
            },
        );
    if let Some((part, heading)) = chargeworks {
        child_pieces +=
            super::chargeworks::spawn(commands, assets, meshes, cell, coord, part, heading);
    }
    if let Some(heading) = archive {
        child_pieces += super::archive::spawn(commands, assets, meshes, cell, coord, heading);
    } else if architecture == ArchitectureRegister::InfiniteGallery && wonder.is_none() {
        child_pieces +=
            super::library::spawn(commands, assets, meshes, cell, coord, world, &pieces);
    }
    let mut zen_shell_ready = false;
    if let Some(heading) = rain {
        child_pieces += super::rain::spawn(commands, assets, meshes, cell, coord, heading, &pieces);
    } else if architecture == ArchitectureRegister::ShadowScreen && wonder.is_none() {
        let (count, ready) =
            super::zen::spawn(commands, assets, meshes, cell, coord, world, &pieces);
        child_pieces += count;
        zen_shell_ready = ready;
    }
    if let Some(heading) = concourse {
        child_pieces +=
            super::concourse::spawn(commands, assets, meshes, cell, coord, heading, &pieces);
    }
    if let Some(heading) = jade {
        child_pieces += super::jade::spawn(commands, assets, meshes, cell, coord, heading, &pieces);
    }
    if let Some(heading) = promenade {
        child_pieces +=
            super::promenade::spawn(commands, assets, meshes, cell, coord, heading, &pieces);
    }
    let origin = Vec3::from_array(hex_origin(coord));
    let tile_key = cell_mesh_key(&pieces);

    let groups = super::mesh_group::gather(&pieces);
    for (group_key, group) in groups {
        if group_key == super::assets::MeshGroupKey::Hidden
            || (zen_shell_ready
                && matches!(
                    group_key,
                    super::assets::MeshGroupKey::Interior
                        | super::assets::MeshGroupKey::Perimeter(_)
                ))
            || ((rain.is_some() || concourse.is_some() || jade.is_some() || promenade.is_some())
                && matches!(
                    group_key,
                    super::assets::MeshGroupKey::Floor
                        | super::assets::MeshGroupKey::Ceiling
                        | super::assets::MeshGroupKey::Interior
                        | super::assets::MeshGroupKey::Perimeter(_)
                ))
        {
            continue;
        }
        let Some(mesh) = assets.merged_mesh_for_owned(
            meshes,
            tile_key.as_deref(),
            group_key,
            &group.hulls,
            &group.occluders,
        ) else {
            continue;
        };
        let material = if archive.is_some() {
            assets.archive_material(group_key)
        } else if reservoir {
            assets.reservoir_material(group_key)
        } else if chargeworks.is_some() {
            assets.chargeworks_material(group_key)
        } else {
            assets.material_for_group_at(architecture, group_key, coord)
        };
        let mut entity = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(origin),
            ChildOf(cell),
            Name::new(format!("Hex cell {:?} mesh", group_key)),
        ));
        if group_key == super::assets::MeshGroupKey::Ceiling {
            entity.insert(super::spectate::CeilingCutaway);
        }
        if group_key == super::assets::MeshGroupKey::Boundary {
            entity.insert(super::spectate::BoundaryShell);
        } else {
            let local = match group_key {
                super::assets::MeshGroupKey::Floor
                | super::assets::MeshGroupKey::Ceiling
                | super::assets::MeshGroupKey::Interior => Vec3::ZERO,
                super::assets::MeshGroupKey::Perimeter(_) => {
                    if group.point_count > 0 {
                        #[allow(clippy::cast_precision_loss)]
                        let c = group.centroid_sum / group.point_count as f32;
                        c
                    } else {
                        Vec3::ZERO
                    }
                }
                _ => Vec3::ZERO,
            };
            entity.insert(super::spectate::Cutaway {
                local,
                min_y: group.min_y,
                max_y: group.max_y,
                origin_y: origin.y,
                cell_level: coord.level,
                climb_wall: matches!(
                    group_key,
                    super::assets::MeshGroupKey::Climb(
                        super::mesh_group::Facing::Side | super::mesh_group::Facing::Down
                    )
                ),
            });
        }
        child_pieces += 1;
    }
    for piece in trim {
        spawn_trim(commands, assets, meshes, piece, architecture, cell);
        child_pieces += 1;
    }
    SpawnedCell {
        coord,
        entity: cell,
        child_pieces,
        needs_decoration: false,
    }
}

/// Spawn one derived seam-trim descriptor as a dim structural mesh, parented to
/// its owning cell so it streams and relayout-rebuilds with that cell. Trim is
/// non-authoritative decoration (no collider) and non-signal (Legibility
/// Contract) — it only hides tile seams and makes the facility read as one
/// megastructure.
/// Project a standalone authored piece into the resident shell.
fn spawn_piece(
    commands: &mut Commands,
    assets: &mut HexWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    piece: &HexStructurePiece,
    architecture: ArchitectureRegister,
    parent: Option<Entity>,
    hull_index: usize,
) -> bool {
    let Some(mesh) = assets.mesh_for(meshes, piece, hull_index) else {
        return false;
    };
    let material = assets.material_for_piece(architecture, piece);
    let mut entity = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(piece.center).with_rotation(Quat::from_array(piece.rotation)),
        Name::new(format!("Hex {:?} collider {}", piece.role, piece.id.0)),
    ));
    if let Some(parent) = parent {
        entity.insert(ChildOf(parent));
    } else {
        entity.insert((HexWfcGeometry, DespawnOnExit(GameState::HexWfc)));
    }
    // Tagged where the role is already known, so the spectator overview can
    // drop it: in play the shell is the far wall you never reach, but from
    // outside looking in it is a lid over the whole building.
    if piece.role == HexStructureRole::Boundary {
        entity.insert(super::spectate::BoundaryShell);
    } else {
        // Measured once, here: the overview's cutaway needs a hull's height
        // range and its offset within its cell on every rebuild, and computing
        // it per frame means walking the point cloud again.
        entity.insert(cutaway_measure(piece));
    }
    true
}

/// A hull's height range and its centroid within its own cell.
fn cutaway_measure(piece: &HexStructurePiece) -> super::spectate::Cutaway {
    let rotation = Quat::from_array(piece.rotation);
    let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);
    // The *hull's own* centroid, not the collider's origin.
    //
    // `piece.center` is where the collider is placed, which for a convex hull
    // is the tile origin - so using it made every hull look like it sat at the
    // cell centre, `INTERIOR_RADIUS` matched them all, and no near wall was
    // ever cut. Only ceilings were, which is exactly why the cutaway read as
    // half-working here and correct in the studio: `detail::measure` averages
    // the points.
    let mut centroid = Vec3::ZERO;
    match &piece.shape {
        observed_traversal::ColliderShape::Cuboid { half } => {
            // A rotated box's vertical reach is each axis's contribution to Y,
            // which is exactly the AABB half-height. Its centroid is its centre.
            let reach = (rotation * Vec3::new(half.x, 0.0, 0.0)).y.abs()
                + (rotation * Vec3::new(0.0, half.y, 0.0)).y.abs()
                + (rotation * Vec3::new(0.0, 0.0, half.z)).y.abs();
            min_y = piece.center.y - reach;
            max_y = piece.center.y + reach;
            centroid = piece.center;
        }
        observed_traversal::ColliderShape::ConvexHull { points } => {
            let mut sum = Vec3::ZERO;
            for point in points {
                let placed = rotation * *point + piece.center;
                min_y = min_y.min(placed.y);
                max_y = max_y.max(placed.y);
                sum += placed;
            }
            if !points.is_empty() {
                #[allow(clippy::cast_precision_loss)]
                {
                    centroid = sum / points.len() as f32;
                }
            }
        }
    }
    let origin = Vec3::from_array(hex_origin(piece.source_cell));
    super::spectate::Cutaway {
        local: centroid - origin,
        min_y: min_y - origin.y,
        max_y: max_y - origin.y,
        origin_y: origin.y,
        cell_level: piece.source_cell.level,
        climb_wall: false,
    }
}

/// Local hull fingerprints already distinguish every opened/rebuilt recipe.
/// Cell coordinates would prevent identical recipes from sharing a mesh.
pub(super) fn cell_mesh_key(pieces: &[&HexStructurePiece]) -> Option<String> {
    pieces
        .first()
        .and_then(|piece| piece.tile.as_ref())
        .map(|tile| format!("{tile:?}"))
}
