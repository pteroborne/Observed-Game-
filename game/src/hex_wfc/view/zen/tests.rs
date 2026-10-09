use super::*;
use observed_facility::hex_wfc::{HexArchetype, HexWfcConfig};
use observed_traversal::{ColliderShape, StableColliderId};

fn fixture() -> (HexWfcWorld, HexCoord) {
    let mut world = HexWfcWorld::generate(7, HexWfcConfig::default()).unwrap();
    let coord = *world.placements.keys().next().unwrap();
    let placement = world.placements.get_mut(&coord).unwrap();
    placement.archetype = HexArchetype::Room;
    placement.doors = 0;
    (world, coord)
}
fn piece(coord: HexCoord, center: Vec3, size: Vec3) -> HexStructurePiece {
    HexStructurePiece {
        surface: None,
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
fn zen_lattice_keeps_doors_and_split_windows_clear() {
    let (mut world, coord) = fixture();
    let solid = piece(coord, Vec3::new(6.75, 4.0, 0.0), Vec3::new(0.5, 8.0, 8.0));
    let fits = details(&world, coord, &[&solid]);
    assert_eq!(fits.len(), 1);
    // Details fit inside the character's contact clearance, rather than
    // creating visible barriers that have no physical collision.
    for p in fits[0].hulls.iter().flatten() {
        assert!(6.5 - p.x < observed_traversal::FpsConfig::default().radius);
    }
    world.placements.get_mut(&coord).unwrap().doors = 1 << HexFace::East.index();
    assert!(details(&world, coord, &[&solid]).is_empty());
    world.placements.get_mut(&coord).unwrap().doors = 0;
    let frames = [
        piece(coord, Vec3::new(6.75, 4.0, -2.9), Vec3::new(0.5, 8.0, 2.2)),
        piece(coord, Vec3::new(6.75, 4.0, 2.9), Vec3::new(0.5, 8.0, 2.2)),
        piece(coord, Vec3::new(6.75, 6.5, 0.0), Vec3::new(0.5, 3.0, 8.0)),
        piece(coord, Vec3::new(6.75, 0.6, 0.0), Vec3::new(0.5, 1.2, 8.0)),
    ];
    let fits = details(&world, coord, &frames.iter().collect::<Vec<_>>());
    assert!(fits.len() >= 3);
    for p in fits.iter().flat_map(|d| d.hulls.iter().flatten()) {
        assert!(
            p.y >= 5.0 || p.y <= 1.2 || p.z.abs() >= 1.8,
            "lattice crosses window: {p:?}"
        );
    }
}

#[test]
fn zen_slats_fit_actual_undersides_and_preserve_stairwell_gaps() {
    let (world, coord) = fixture();
    let slabs = [
        piece(coord, Vec3::new(-3.5, 7.75, 0.0), Vec3::new(5.0, 0.5, 12.0)),
        piece(coord, Vec3::new(3.5, 7.75, 0.0), Vec3::new(5.0, 0.5, 12.0)),
    ];
    let fits = details(&world, coord, &slabs.iter().collect::<Vec<_>>());
    assert_eq!(fits.len(), 2);
    for p in fits.iter().flat_map(|d| d.hulls.iter().flatten()) {
        assert!(p.x.abs() >= 1.0, "slats bridge a roof opening: {p:?}");
        assert!((7.43..=7.5).contains(&p.y));
    }
    // A raked slab's lowest two vertices cannot supply a flat ceiling.
    let mut slope = points(&slabs[0], Vec3::from_array(hex_origin(coord)));
    for p in &mut slope {
        p.y += p.x * 0.06;
    }
    assert!(ceiling_detail(&slope).is_none());
}

#[test]
fn zen_paper_is_reserved_for_broad_screens_not_posts_or_lintels() {
    let (_, coord) = fixture();
    for (center, size, expected) in [
        (Vec3::new(6.75, 4.0, 0.0), Vec3::new(0.5, 8.0, 8.0), 1),
        (Vec3::new(6.75, 4.0, 0.0), Vec3::new(0.5, 8.0, 0.5), 0),
        (Vec3::new(6.75, 6.5, 0.0), Vec3::new(0.5, 3.0, 8.0), 0),
        (Vec3::new(0.0, 0.8, 0.0), Vec3::new(2.0, 0.3, 1.0), 0),
    ] {
        let mut source = piece(coord, center, size);
        for angle in [0.0, 0.52, 1.04] {
            source.rotation = Quat::from_rotation_y(angle).to_array();
            assert_eq!(
                shell::finish(
                    &source,
                    &points(&source, Vec3::from_array(hex_origin(coord)))
                ),
                expected
            );
        }
    }
}

#[test]
fn zen_narrow_corridor_receives_lattice_and_slats_on_its_actual_supports() {
    let (world, coord) = fixture();
    let screen = piece(coord, Vec3::new(2.0, 4.0, 0.0), Vec3::new(0.5, 8.0, 8.0));
    let raft = piece(coord, Vec3::new(0.0, 5.25, 0.0), Vec3::new(3.0, 0.5, 8.0));
    let fits = details(&world, coord, &[&screen, &raft]);
    assert_eq!(fits.len(), 2);
    let vertical = fits.iter().find(|d| d.max_y - d.min_y > 1.0).unwrap();
    assert!(vertical.hulls.iter().flatten().all(|p| p.x > 1.6));
    let mut transverse = screen.clone();
    transverse.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array();
    assert_eq!(details(&world, coord, &[&transverse, &raft]).len(), 2);
    let overhead = fits.iter().find(|d| d.max_y - d.min_y < 0.1).unwrap();
    assert!(
        overhead
            .hulls
            .iter()
            .flatten()
            .all(|p| p.x.abs() <= 1.5 && p.y >= 4.93)
    );
}

#[test]
fn zen_details_follow_rotated_walls_and_despawn_with_climb_cells() {
    let (world, coord) = fixture();
    let mut solid = piece(coord, Vec3::new(6.75, 4.0, 0.0), Vec3::new(0.5, 8.0, 8.0));
    solid.rotation = Quat::from_rotation_y(0.055).to_array();
    solid.role = HexStructureRole::Climb;
    assert_eq!(details(&world, coord, &[&solid]).len(), 1);
    let mut ecs = World::new();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = HexWfcVisualAssets::for_test(&mut materials);
    for _ in 0..2 {
        let parent = ecs.spawn_empty().id();
        let count = spawn(
            &mut Commands::new(&mut queue, &ecs),
            &mut assets,
            &mut meshes,
            parent,
            coord,
            &world,
            &[&solid],
        );
        queue.apply(&mut ecs);
        assert_eq!(count, (1, true));
        assert_eq!(meshes.len(), 1, "same actual support should reuse its mesh");
        for (child, cutaway) in ecs
            .query_filtered::<(&ChildOf, &Cutaway), With<ZenDetail>>()
            .iter(&ecs)
        {
            assert_eq!(child.parent(), parent);
            assert!(cutaway.climb_wall);
            assert_eq!(cutaway.cell_level, coord.level);
        }
        ecs.entity_mut(parent).despawn();
        assert_eq!(ecs.query::<&ZenDetail>().iter(&ecs).count(), 0);
    }
}

#[test]
fn zen_production_halls_and_climbs_receive_supported_details() {
    use crate::hex_wfc::launch::{HexLaunchSpec, HexSeedPolicy, prepare};
    use observed_content::ArchitectureRegister;
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
    let mut hall = false;
    let mut climb = false;
    for placement in game.facility.placements.values().filter(|p| {
        game.facility.architecture.get(&p.coord) == Some(&ArchitectureRegister::ShadowScreen)
    }) {
        let pieces: Vec<_> = game
            .geometry
            .pieces
            .iter()
            .filter(|p| p.source_cell == placement.coord)
            .collect();
        if !details(&game.facility, placement.coord, &pieces).is_empty() {
            if matches!(placement.archetype, HexArchetype::Climb { .. }) {
                climb = true;
            } else {
                hall = true;
            }
        }
    }
    assert!(
        hall && climb,
        "both ordinary corridors and the Zen ascent need fitted detail"
    );
}

#[test]
fn cold_zen_wall_recipes_keep_the_base_shell_until_all_finishes_are_ready() {
    let (_, coord) = fixture();
    let screen = piece(coord, Vec3::new(6.75, 4.0, 0.0), Vec3::new(0.5, 8.0, 8.0));
    let post = piece(coord, Vec3::new(-6.75, 4.0, 0.0), Vec3::new(0.5, 8.0, 0.5));
    let mut world = World::default();
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let mut assets = HexWfcVisualAssets::for_test(&mut materials);
    let parent = world.spawn_empty().id();
    // One finish is cached; the other is deliberately cold. No partial paper/wood
    // wall should replace the complete base shell while the worker prepares it.
    {
        let mut commands = world.commands();
        assert!(
            shell::spawn(
                &mut commands,
                &mut assets,
                &mut meshes,
                parent,
                coord,
                &[&post]
            )
            .is_some()
        );
    }
    world.flush();
    let before = world.query::<&Mesh3d>().iter(&world).count();
    assets.preparing_cell = true;
    {
        let mut commands = world.commands();
        assert!(
            shell::spawn(
                &mut commands,
                &mut assets,
                &mut meshes,
                parent,
                coord,
                &[&post, &screen]
            )
            .is_none()
        );
    }
    world.flush();
    assert!(assets.missing_meshes);
    assert_eq!(world.query::<&Mesh3d>().iter(&world).count(), before);
    assets.preparing_cell = false;
    {
        let mut commands = world.commands();
        assert!(
            shell::spawn(
                &mut commands,
                &mut assets,
                &mut meshes,
                parent,
                coord,
                &[&post, &screen]
            )
            .is_some()
        );
    }
    world.flush();
    assert!(world.query::<&Mesh3d>().iter(&world).count() > before);
}
