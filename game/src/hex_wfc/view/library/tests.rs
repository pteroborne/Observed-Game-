use super::*;
use observed_facility::hex_wfc::{HexArchetype, HexWfcConfig};
use observed_match::hex_wfc::HexStructureRole;
use observed_traversal::{ColliderShape, StableColliderId};

fn fixture() -> (HexWfcWorld, HexCoord) {
    let mut facility = HexWfcWorld::generate(7, HexWfcConfig::default()).unwrap();
    let coord = *facility.placements.keys().next().unwrap();
    let placement = facility.placements.get_mut(&coord).unwrap();
    placement.archetype = HexArchetype::Room;
    placement.doors = 0;
    (facility, coord)
}

fn wall(coord: HexCoord, center: Vec3, size: Vec3) -> HexStructurePiece {
    HexStructurePiece {
        id: StableColliderId(1),
        anchor: coord,
        source_cell: coord,
        role: HexStructureRole::Hall,
        part: HexPiecePart::Authored,
        tile: None,
        center: Vec3::from_array(hex_origin(coord)),
        rotation: Quat::IDENTITY.to_array(),
        shape: ColliderShape::ConvexHull {
            points: cuboid(center, size, Vec3::X, HexFace::East),
        },
    }
}

#[test]
fn library_fits_do_not_bridge_window_openings_or_declared_doors() {
    let (mut facility, coord) = fixture();
    let solid = wall(coord, Vec3::new(6.75, 4.0, 0.0), Vec3::new(0.5, 8.0, 8.0));
    assert_eq!(bookcases(&facility, coord, &[&solid]).len(), 1);
    facility.placements.get_mut(&coord).unwrap().doors = 1 << HexFace::East.index();
    assert!(bookcases(&facility, coord, &[&solid]).is_empty());
    facility.placements.get_mut(&coord).unwrap().doors = 0;
    // A window's four convex frame pieces must remain four independent supports.
    let frames = [
        wall(coord, Vec3::new(6.75, 4.0, -2.9), Vec3::new(0.5, 8.0, 2.2)),
        wall(coord, Vec3::new(6.75, 4.0, 2.9), Vec3::new(0.5, 8.0, 2.2)),
        wall(coord, Vec3::new(6.75, 6.5, 0.0), Vec3::new(0.5, 3.0, 8.0)),
        wall(coord, Vec3::new(6.75, 0.6, 0.0), Vec3::new(0.5, 1.2, 8.0)),
    ];
    let fits = bookcases(&facility, coord, &frames.iter().collect::<Vec<_>>());
    assert!(
        !fits.is_empty(),
        "the lintel can still hold a small upper bookcase"
    );
    for fit in fits {
        for p in geometry(&fit).iter().flatten().flatten() {
            assert!(
                p.y >= 5.0 || p.z.abs() >= 1.8,
                "detail fills the window: {p:?}"
            );
        }
    }
}

#[test]
fn library_rows_follow_existing_shelves_and_shallow_inlays_clear_the_controller() {
    let (facility, coord) = fixture();
    let solid = wall(coord, Vec3::new(6.75, 4.0, 0.0), Vec3::new(0.5, 8.0, 8.0));
    let shallow = bookcases(&facility, coord, &[&solid]).remove(0);
    let radius = observed_traversal::FpsConfig::default().radius;
    for p in geometry(&shallow).iter().flatten().flatten() {
        assert!(p.dot(shallow.normal) - shallow.plane < radius);
    }
    let shelves = [
        wall(coord, Vec3::new(6.1, 2.0, 0.0), Vec3::new(0.8, 0.2, 8.0)),
        wall(coord, Vec3::new(6.1, 4.0, 0.0), Vec3::new(0.8, 0.2, 8.0)),
    ];
    let fitted = bookcases(&facility, coord, &[&solid, &shelves[0], &shelves[1]]).remove(0);
    let authored: Vec<_> = fitted
        .rows
        .iter()
        .filter(|r| (r.front + 5.745).abs() < 0.001)
        .collect();
    assert_eq!(authored.len(), 2);
    assert!((authored[0].height - 2.112).abs() < 0.001);
    assert!((authored[1].height - 4.112).abs() < 0.001);
    // Shallow extra rows must not put their bindings inside an authored board.
    for p in geometry(&fitted).iter().take(6).flatten().flatten() {
        assert!(!(p.x < 6.5 && p.y > 1.9 && p.y < 2.1));
        assert!(!(p.x < 6.5 && p.y > 3.9 && p.y < 4.1));
    }
}

#[test]
fn library_production_corner_has_fitted_books_on_its_closed_wall() {
    use crate::hex_wfc::launch::{HexLaunchSpec, HexSeedPolicy, prepare};
    let game = prepare(HexLaunchSpec {
        requested_seed: 1,
        config: observed_match::hex_wfc::HexMatchConfig {
            wfc: HexWfcConfig::arc_default(),
            ..default()
        },
        seed_policy: HexSeedPolicy::Nearby,
    })
    .unwrap()
    .match_state;
    let coord = HexCoord {
        q: 2,
        r: 9,
        level: 1,
    };
    let pieces: Vec<_> = game
        .geometry
        .pieces
        .iter()
        .filter(|p| p.source_cell == coord)
        .collect();
    let fits = bookcases(&game.facility, coord, &pieces);
    // Variant 848 has wall planes slightly rotated from the lattice: choosing
    // only exactly coplanar points against the grid normal left this tile bare.
    assert!(fits.iter().any(|c| c.face == HexFace::East));
}

#[test]
fn library_meshes_reuse_fits_and_despawn_with_the_owning_cell() {
    let (facility, coord) = fixture();
    let mut solid = wall(coord, Vec3::new(6.75, 4.0, 0.0), Vec3::new(0.5, 8.0, 8.0));
    let mut world = World::new();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = HexWfcVisualAssets::for_test(&mut materials);
    let mut mesh_count = None;
    for role in [HexStructureRole::Hall, HexStructureRole::Climb] {
        solid.role = role;
        let parent = world.spawn_empty().id();
        let count = spawn(
            &mut Commands::new(&mut queue, &world),
            &mut assets,
            &mut meshes,
            parent,
            coord,
            &facility,
            &[&solid],
        );
        queue.apply(&mut world);
        assert_eq!(count, 7);
        assert_eq!(
            world.query::<&LibraryBookcase>().iter(&world).count(),
            count
        );
        for (child, transform, cutaway) in world
            .query_filtered::<(&ChildOf, &Transform, &Cutaway), With<LibraryBookcase>>()
            .iter(&world)
        {
            assert_eq!(child.parent(), parent);
            assert_eq!(transform.translation, Vec3::from_array(hex_origin(coord)));
            assert_eq!(cutaway.cell_level, coord.level);
            assert_eq!(cutaway.climb_wall, role == HexStructureRole::Climb);
        }
        if let Some(first) = mesh_count {
            assert_eq!(meshes.len(), first, "a rebuilt fit should reuse its meshes");
        } else {
            mesh_count = Some(meshes.len());
        }
        world.entity_mut(parent).despawn();
        assert_eq!(world.query::<&LibraryBookcase>().iter(&world).count(), 0);
    }
}
