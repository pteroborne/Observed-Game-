use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::full_wfc::{
    CellCoord, FullWfcWorld, ModuleFace, ModulePlacement, ModuleSpace,
};
use observed_match::full_wfc::{CELL_SIZE, WALL_HEIGHT, WALL_THICKNESS};

use super::assets::FullWfcVisualAssets;

pub(super) fn spawn_register_dressing(
    parent: &mut ChildSpawnerCommands,
    assets: &mut FullWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    world: &FullWfcWorld,
    placement: &ModulePlacement,
) {
    let mats = assets.register(placement.architecture).clone();
    let closed_face = horizontal_faces()
        .into_iter()
        .find(|face| !placement.is_open(*face));
    let detail = if placement.architecture == ArchitectureRegister::Thinning {
        thinning_detail(world, placement.coord)
    } else {
        1.0
    };

    // A low continuous band gives cell-to-cell register changes a readable join while
    // remaining noncolliding presentation geometry.
    if let Some(face) = closed_face {
        spawn_wall_box(
            parent,
            assets,
            meshes,
            face,
            0.0,
            0.16,
            Vec3::new(7.2 * detail.max(0.35), 0.12, 0.08),
            mats.accent.clone(),
            "architecture baseboard",
        );
    }

    match placement.architecture {
        ArchitectureRegister::ShadowScreen => {
            if let Some(face) = closed_face {
                for along in [-3.6, -1.8, 0.0, 1.8, 3.6] {
                    spawn_wall_box(
                        parent,
                        assets,
                        meshes,
                        face,
                        along,
                        2.45,
                        Vec3::new(0.16, 4.5, 0.16),
                        if along == 0.0 {
                            mats.accent.clone()
                        } else {
                            mats.dark.clone()
                        },
                        "shadow-screen wall slat",
                    );
                }
            }
        }
        ArchitectureRegister::Monolith => {
            if let Some(face) = closed_face {
                for along in [-3.8, 3.8] {
                    spawn_wall_box(
                        parent,
                        assets,
                        meshes,
                        face,
                        along,
                        2.2,
                        Vec3::new(1.25, 4.4, 0.28),
                        mats.dark.clone(),
                        "monolith wall mass",
                    );
                }
                spawn_wall_box(
                    parent,
                    assets,
                    meshes,
                    face,
                    0.0,
                    2.65,
                    Vec3::new(0.10, 4.2, 0.10),
                    mats.accent.clone(),
                    "monolith narrow key",
                );
            }
        }
        ArchitectureRegister::OverlitGrid => {
            for x in [-3.2, 0.0, 3.2] {
                for z in [-3.2, 0.0, 3.2] {
                    if (placement.is_open(ModuleFace::Up) || placement.is_open(ModuleFace::Down))
                        && x == 0.0
                        && z == 0.0
                    {
                        continue;
                    }
                    spawn_box(
                        parent,
                        assets,
                        meshes,
                        Vec3::new(1.55, 0.08, 1.55),
                        Vec3::new(x, WALL_HEIGHT - 0.10, z),
                        mats.fixture.clone(),
                        "overlit ceiling grid",
                    );
                }
            }
        }
        ArchitectureRegister::Institutional => {
            if let Some(face) = closed_face {
                for along in [-3.0, 0.0, 3.0] {
                    spawn_wall_box(
                        parent,
                        assets,
                        meshes,
                        face,
                        along,
                        2.15,
                        Vec3::new(2.5, 3.7, 0.07),
                        mats.wall.clone(),
                        "institutional wall panel",
                    );
                }
            }
        }
        ArchitectureRegister::FacetMonument => {
            if let Some(face) = closed_face {
                for along in [-3.0, 0.0, 3.0] {
                    spawn_wall_box(
                        parent,
                        assets,
                        meshes,
                        face,
                        along,
                        2.5,
                        Vec3::new(0.08, 4.65, 0.07),
                        mats.accent.clone(),
                        "facet monument seam",
                    );
                }
            }
        }
        ArchitectureRegister::Megastructure => {
            if let Some(face) = closed_face {
                spawn_wall_box(
                    parent,
                    assets,
                    meshes,
                    face,
                    0.0,
                    2.1,
                    Vec3::new(7.0, 3.2, 0.34),
                    mats.dark.clone(),
                    "megastructure wall recess",
                );
            }
            for z in [-4.2, 0.0, 4.2] {
                spawn_box(
                    parent,
                    assets,
                    meshes,
                    Vec3::new(10.8, 0.24, 0.28),
                    Vec3::new(0.0, WALL_HEIGHT - 0.28, z),
                    mats.dark.clone(),
                    "megastructure ceiling rib",
                );
            }
        }
        ArchitectureRegister::Wellshaft => {
            for (x, z) in [(-4.4, -4.4), (4.4, 4.4)] {
                spawn_box(
                    parent,
                    assets,
                    meshes,
                    Vec3::new(0.42, 0.72, 0.42),
                    Vec3::new(x, 2.15, z),
                    mats.fixture.clone(),
                    "wellshaft warm caged practical",
                );
            }
        }
        ArchitectureRegister::InfiniteGallery => {
            if let Some(face) = closed_face {
                for y in [1.0, 2.4, 3.8] {
                    spawn_wall_box(
                        parent,
                        assets,
                        meshes,
                        face,
                        0.0,
                        y,
                        Vec3::new(9.0, 0.16, 0.34),
                        mats.accent.clone(),
                        "infinite gallery shelf band",
                    );
                }
            }
        }
        ArchitectureRegister::Thinning => {
            if let Some(face) = closed_face {
                for (index, along) in [-3.6, -1.8, 0.0, 1.8, 3.6].into_iter().enumerate() {
                    if index as f32 / 5.0 > detail {
                        continue;
                    }
                    spawn_wall_box(
                        parent,
                        assets,
                        meshes,
                        face,
                        along,
                        2.3,
                        Vec3::new(0.10, 3.8 * detail.max(0.25), 0.08),
                        mats.accent.clone(),
                        "thinning decayed practical",
                    );
                }
            }
        }
    }
}

fn horizontal_faces() -> [ModuleFace; 4] {
    [
        ModuleFace::East,
        ModuleFace::West,
        ModuleFace::South,
        ModuleFace::North,
    ]
}

#[allow(clippy::too_many_arguments)]
fn spawn_wall_box(
    parent: &mut ChildSpawnerCommands,
    assets: &mut FullWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    face: ModuleFace,
    along: f32,
    y: f32,
    nominal_size: Vec3,
    material: Handle<StandardMaterial>,
    name: &'static str,
) {
    let inset = CELL_SIZE * 0.5 - WALL_THICKNESS - nominal_size.z * 0.5 - 0.015;
    let (position, size) = match face {
        ModuleFace::East => (
            Vec3::new(inset, y, along),
            Vec3::new(nominal_size.z, nominal_size.y, nominal_size.x),
        ),
        ModuleFace::West => (
            Vec3::new(-inset, y, along),
            Vec3::new(nominal_size.z, nominal_size.y, nominal_size.x),
        ),
        ModuleFace::South => (Vec3::new(along, y, inset), nominal_size),
        ModuleFace::North => (Vec3::new(along, y, -inset), nominal_size),
        ModuleFace::Up | ModuleFace::Down => return,
    };
    spawn_box(parent, assets, meshes, size, position, material, name);
}

fn spawn_box(
    parent: &mut ChildSpawnerCommands,
    assets: &mut FullWfcVisualAssets,
    meshes: &mut Assets<Mesh>,
    size: Vec3,
    position: Vec3,
    material: Handle<StandardMaterial>,
    name: &'static str,
) {
    parent.spawn((
        Mesh3d(assets.mesh_for(meshes, size)),
        MeshMaterial3d(material),
        Transform::from_translation(position),
        Name::new(name),
    ));
}

/// Detail falls away toward the middle of a logical corridor and returns at its
/// room endpoints. This is deterministic presentation data derived from the stable
/// corridor cell set; it does not add another generation layer.
pub(super) fn thinning_detail(world: &FullWfcWorld, coord: CellCoord) -> f32 {
    let Some(corridor_id) = world.corridor_at(coord) else {
        return 1.0;
    };
    let cells = &world.corridors[&corridor_id].cells;
    let mut distances = BTreeMap::new();
    let mut queue = VecDeque::new();
    for &cell in cells {
        let touches_room = ModuleFace::ALL.into_iter().any(|face| {
            world
                .config
                .neighbor(cell, face)
                .and_then(|neighbor| world.placement(neighbor))
                .is_some_and(|neighbor| neighbor.space == ModuleSpace::Room)
        });
        if touches_room {
            distances.insert(cell, 0u16);
            queue.push_back(cell);
        }
    }
    while let Some(cell) = queue.pop_front() {
        let next_distance = distances[&cell].saturating_add(1);
        for face in ModuleFace::ALL {
            let Some(next) = world.config.neighbor(cell, face) else {
                continue;
            };
            if cells.contains(&next) && !distances.contains_key(&next) {
                distances.insert(next, next_distance);
                queue.push_back(next);
            }
        }
    }
    let distance = f32::from(*distances.get(&coord).unwrap_or(&0));
    let max = f32::from(*distances.values().max().unwrap_or(&0)).max(1.0);
    (1.0 - distance / max * 0.76).clamp(0.24, 1.0)
}

#[cfg(test)]
mod tests {
    use observed_facility::full_wfc::{FullWfcConfig, FullWfcWorld};

    use super::*;

    #[test]
    fn thinning_detail_is_bounded_and_deterministic() {
        let world = FullWfcWorld::catalog_fixture(0xC011_1DE3).expect("fixture");
        for corridor in world.corridors.values() {
            for &cell in &corridor.cells {
                let detail = thinning_detail(&world, cell);
                assert!((0.24..=1.0).contains(&detail));
                assert_eq!(detail, thinning_detail(&world, cell));
            }
        }
        let ordinary = FullWfcWorld::new(3, FullWfcConfig::default()).expect("world");
        assert_eq!(thinning_detail(&ordinary, ordinary.spawn()), 1.0);
    }
}
