use super::*;
use observed_facility::hex_wfc::authored_switching_concourse;
fn turn(mut p: Vec3, rotation: usize) -> Vec3 {
    for _ in 0..rotation {
        p = Vec3::new(0.5 * p.x - 0.875 * p.z, p.y, 6.0 / 7.0 * p.x + 0.5 * p.z);
    }
    p
}
#[test]
fn concourse_three_entries_crossings_and_platform_circuit_at_all_six_headings() {
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
        level: 2,
    };
    for rotation in 0..6 {
        let sectors = authored_switching_concourse(world.config, anchor, rotation).unwrap();
        let mut fixture = world.clone();
        for p in sectors {
            fixture.placements.insert(p.coord, p);
            fixture
                .architecture
                .insert(p.coord, ArchitectureRegister::OverlitGrid);
        }
        let geometry =
            HexWfcGeometrySnapshot::project(&fixture, &tiles()).expect("Lumen concourse projects");
        let hulls = geometry
            .pieces
            .iter()
            .filter(|p| sectors.iter().any(|s| s.coord == p.source_cell) && p.part.collides())
            .count();
        assert!(hulls <= 128);
        let scene = RapierTraversalScene::from_arena_spec(&geometry.arena);
        let at = |sector: usize, p| {
            let HexArchetype::SwitchingConcourse { heading } = sectors[sector].archetype else {
                unreachable!()
            };
            Vec3::from_array(hex_origin(sectors[sector].coord)) + turn(p, heading.index())
        };
        let mut circuit = Vec::new();
        for sector in [0, 2, 1, 0] {
            for (x, z) in [(0.0, -2.8), (0.0, 0.0), (2.0, 1.8), (3.0, 4.7), (1.0, 7.0)] {
                circuit.push(at(sector, Vec3::new(x, 0.5, z)));
            }
        }
        for _ in 0..2 {
            walk_room_route(&scene, &circuit)
                .unwrap_or_else(|p| panic!("platform circuit {rotation}: {p:?}"));
            circuit.reverse();
        }
        for sector in 0..3 {
            let mut entry = [
                at(sector, Vec3::new(-3.0, 0.5, -4.4)),
                at(sector, Vec3::new(0.0, 0.5, -2.8)),
                at(sector, Vec3::new(0.0, 0.5, 0.0)),
                at(sector, Vec3::new(7.0, 0.5, 4.0)),
            ];
            for _ in 0..2 {
                walk_room_route(&scene, &entry)
                    .unwrap_or_else(|p| panic!("entry/crossing {rotation}: {p:?}"));
                entry.reverse();
            }
            let from = at(sector, Vec3::new(-4.6, 2.0, 3.4));
            let axis = (at(sector, Vec3::new(-5.4, 2.0, 3.4)) - from).normalize();
            assert!(
                scene.ray_distance(from, axis, 1.0).is_some(),
                "service fin is opaque"
            );
            let ceiling = at(sector, Vec3::new(1.0, 2.0, 1.0));
            assert!(
                scene.ray_distance(ceiling, Vec3::Y, 6.0).is_some(),
                "sealed storey canopy"
            );
        }
    }
}
