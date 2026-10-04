use super::*;
use observed_facility::hex_wfc::authored_rain_court;
fn turn(mut p: Vec3, rotation: usize) -> Vec3 {
    for _ in 0..rotation {
        p = Vec3::new(0.5 * p.x - 0.875 * p.z, p.y, 6.0 / 7.0 * p.x + 0.5 * p.z);
    }
    p
}
#[test]
fn rain_all_entries_dry_circuit_crossings_and_opaque_screen_gaps_at_six_headings() {
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
    let anchor = HexCoord {
        q: 4,
        r: 4,
        level: 3,
    };
    for rotation in 0..6 {
        let sectors = authored_rain_court(world.config, anchor, rotation).unwrap();
        let mut fixture = world.clone();
        for p in sectors {
            fixture.placements.insert(p.coord, p);
            fixture
                .architecture
                .insert(p.coord, ArchitectureRegister::ShadowScreen);
        }
        let geometry = HexWfcGeometrySnapshot::project(&fixture, &tiles())
            .expect("Rain Court projects in Zen");
        let hulls = geometry
            .pieces
            .iter()
            .filter(|p| sectors.iter().any(|s| s.coord == p.source_cell) && p.part.collides())
            .count();
        assert!(hulls <= 128, "hull budget {hulls}");
        let scene = RapierTraversalScene::from_arena_spec(&geometry.arena);
        let mut dry = Vec::new();
        let mut exposed_routes = Vec::new();
        for sector in [sectors[0], sectors[2], sectors[1], sectors[0]] {
            let HexArchetype::RainCourt { heading } = sector.archetype else {
                unreachable!()
            };
            let origin = Vec3::from_array(hex_origin(sector.coord));
            let at = |p| origin + turn(p, heading.index());
            for (x, z) in [
                (0.0, -3.0),
                (-3.0, -1.0),
                (-3.0, 3.0),
                (0.0, 5.5),
                (1.0, 7.2),
            ] {
                dry.push(at(Vec3::new(x, 0.5, z)));
            }
            let mut exposed = Vec::new();
            for (x, z) in [
                (0.0, 0.0),
                (3.0, 0.0),
                (3.0, 3.5),
                (7.0, 4.0),
                (3.0, 3.5),
                (3.0, 0.0),
                (0.0, 0.0),
            ] {
                exposed.push(at(Vec3::new(x, 0.5, z)));
            }
            exposed_routes.push(exposed);
            let mut entry = [at(Vec3::new(-3.0, 0.5, -4.4)), at(Vec3::new(0.0, 0.5, 0.0))];
            for _ in 0..2 {
                walk_room_route(&scene, &entry)
                    .unwrap_or_else(|p| panic!("entry rotation {rotation}: {p:?}"));
                entry.reverse();
            }
            let from = at(Vec3::new(-0.5, 2.0, 1.5));
            let normal = turn(Vec3::Z, heading.index()).normalize();
            assert!(
                scene.ray_distance(from, normal, 1.0).is_some(),
                "paper must be opaque"
            );
            assert_eq!(
                scene.ray_distance(at(Vec3::new(-3.0, 2.0, 1.5)), normal, 1.0),
                None,
                "real west screen gap"
            );
            assert_eq!(
                scene.ray_distance(at(Vec3::new(2.0, 2.0, 1.5)), normal, 1.0),
                None,
                "real east screen gap"
            );
        }
        for _ in 0..2 {
            walk_room_route(&scene, &dry)
                .unwrap_or_else(|p| panic!("dry circuit rotation {rotation}: {p:?}"));
            for exposed in &mut exposed_routes {
                walk_room_route(&scene, exposed)
                    .unwrap_or_else(|p| panic!("crossing rotation {rotation}: {p:?}"));
                exposed.reverse();
            }
            dry.reverse();
        }
    }
}
