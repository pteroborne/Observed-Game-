use super::*;
use observed_facility::hex_wfc::authored_jade_nave;
fn turn(mut p: Vec3, rotation: usize) -> Vec3 {
    for _ in 0..rotation {
        p = Vec3::new(0.5 * p.x - 0.875 * p.z, p.y, 6.0 / 7.0 * p.x + 0.5 * p.z);
    }
    p
}
#[test]
fn jade_lower_crossings_upper_circuit_and_all_ramps_are_walkable_at_every_rotation() {
    let mut world = showcase();
    world.config.levels = 8;
    world.blueprints.clear();
    for p in world.placements.values_mut() {
        p.space = HexSpace::Void;
        p.archetype = HexArchetype::Void;
        p.doors = 0;
        p.up = PortClass::Sealed;
        p.down = PortClass::Sealed;
    }
    for level in [4, 5] {
        let anchor = HexCoord { q: 4, r: 4, level };
        let tiles = tiles();
        for rotation in 0..6 {
            let sectors = authored_jade_nave(world.config, anchor, rotation).unwrap();
            let mut fixture = world.clone();
            for p in sectors {
                fixture.placements.insert(p.coord, p);
                fixture
                    .architecture
                    .insert(p.coord, ArchitectureRegister::FacetMonument);
            }
            let geometry = HexWfcGeometrySnapshot::project(&fixture, &tiles)
                .expect("Jade projects in Monument");
            let count = geometry
                .pieces
                .iter()
                .filter(|p| sectors.iter().any(|s| s.coord == p.source_cell) && p.part.collides())
                .count();
            assert!(count <= 128, "room hull budget: {count}");
            let scene = RapierTraversalScene::from_arena_spec(&geometry.arena);
            let centers = sectors.map(|p| Vec3::from_array(hex_origin(p.coord)) + Vec3::Y * 0.5);
            for rise in [0.0, 2.5] {
                let mut path: Vec<_> = [centers[0], centers[1], centers[2], centers[0]]
                    .into_iter()
                    .map(|p| p + Vec3::Y * rise)
                    .collect();
                for _ in 0..2 {
                    walk_room_route(&scene, &path).unwrap_or_else(|feet| {
                        panic!("rotation {rotation}, rise {rise}: circuit blocked at {feet:?}")
                    });
                    path.reverse();
                }
            }
            for sector in sectors {
                let HexArchetype::JadeNave { heading } = sector.archetype else {
                    unreachable!()
                };
                let origin = Vec3::from_array(hex_origin(sector.coord));
                let at = |p| origin + turn(p, heading.index());
                let mut ramp: Vec<_> = [
                    Vec3::new(0.0, 0.5, 0.0),
                    Vec3::new(-3.5, 0.5, -2.0),
                    Vec3::new(-4.9, 0.5, -3.7),
                    Vec3::new(-4.9, 3.0, 2.5),
                    Vec3::new(-3.5, 3.0, 2.5),
                    Vec3::new(-3.5, 3.0, 0.0),
                    Vec3::new(0.0, 3.0, 0.0),
                ]
                .into_iter()
                .map(at)
                .collect();
                for _ in 0..2 {
                    walk_room_route(&scene, &ramp).unwrap_or_else(|feet| {
                        panic!("rotation {rotation} {heading:?}: ramp blocked at {feet:?}")
                    });
                    ramp.reverse();
                }
                let eye = at(Vec3::new(-3.5, 4.5, 3.75));
                assert!(
                    scene
                        .ray_distance(eye, (at(Vec3::new(-1.5, 4.5, 3.75)) - eye).normalize(), 3.0)
                        .is_some(),
                    "jade pier must block Guardian sightlines"
                );
                let ceiling = at(Vec3::new(0.0, 4.5, 0.0));
                assert!(
                    scene.ray_distance(ceiling, Vec3::Y, 4.0).is_some(),
                    "sealed Monument storey"
                );
                let mut entry = [at(Vec3::new(-3.0, 0.5, -4.4)), at(Vec3::new(0.0, 0.5, 0.0))];
                for _ in 0..2 {
                    walk_room_route(&scene, &entry).unwrap_or_else(|feet| {
                        panic!("rotation {rotation}: entry blocked at {feet:?}")
                    });
                    entry.reverse();
                }
            }
        }
    }
}
