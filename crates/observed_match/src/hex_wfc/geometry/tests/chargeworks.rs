use super::*;
use observed_facility::hex_wfc::{ChargeworksPart, authored_chargeworks};

#[test]
fn chargeworks_joins_and_service_gantry_are_walkable_at_all_six_rotations() {
    let mut world = showcase();
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
        level: 0,
    };
    let tiles = tiles();
    for rotation in 0..6 {
        let sectors = authored_chargeworks(world.config, anchor, rotation).unwrap();
        let mut fixture = world.clone();
        for p in sectors {
            fixture.placements.insert(p.coord, p);
        }
        let geometry =
            HexWfcGeometrySnapshot::project(&fixture, &tiles).expect("Chargeworks projects");
        assert!(
            geometry
                .pieces
                .iter()
                .filter(|p| sectors.iter().any(|s| s.coord == p.source_cell) && p.part.collides())
                .count()
                < 128
        );
        let scene = RapierTraversalScene::from_arena_spec(&geometry.arena);
        let centers = sectors.map(|p| Vec3::from_array(hex_origin(p.coord)) + Vec3::Y * 0.75);
        let route = [centers[0], centers[1], centers[2], centers[0]];
        for reversed in [false, true] {
            let mut path = route.to_vec();
            if reversed {
                path.reverse();
            }
            walk_room_route(&scene, &path).unwrap_or_else(|feet| {
                panic!("rotation {rotation} internal crossing blocked at {feet:?}")
            });
        }
        let HexArchetype::Chargeworks {
            part: ChargeworksPart::Transfer,
            heading,
        } = sectors[1].archetype
        else {
            panic!("middle role")
        };
        let origin = Vec3::from_array(hex_origin(sectors[1].coord));
        let h =
            glam::Quat::from_rotation_y(-(heading.index() as f32) * std::f32::consts::TAU / 6.0);
        let ramp = [
            Vec3::new(0.0, 0.75, 0.0),
            Vec3::new(0.7, 0.5, -4.08),
            Vec3::new(-5.0, 0.5, -4.08),
            Vec3::new(-5.0, 3.0, 2.5),
            Vec3::new(-2.0, 3.0, 2.5),
        ];
        let mut path: Vec<_> = ramp.into_iter().map(|p| origin + h * p).collect();
        walk_room_route(&scene, &path)
            .unwrap_or_else(|feet| panic!("rotation {rotation} gantry climb blocked at {feet:?}"));
        path.reverse();
        walk_room_route(&scene, &path).unwrap_or_else(|feet| {
            panic!("rotation {rotation} gantry descent blocked at {feet:?}")
        });
        for p in sectors {
            let HexArchetype::Chargeworks { heading, .. } = p.archetype else {
                unreachable!()
            };
            let h = glam::Quat::from_rotation_y(
                -(heading.index() as f32) * std::f32::consts::TAU / 6.0,
            );
            let origin = Vec3::from_array(hex_origin(p.coord));
            let mut entry = [
                origin + h * Vec3::new(-3.0, 0.75, -4.4),
                origin + Vec3::Y * 0.75,
            ];
            for _ in 0..2 {
                walk_room_route(&scene, &entry).unwrap_or_else(|feet| {
                    panic!("rotation {rotation} {heading:?} entrance blocked at {feet:?}")
                });
                entry.reverse();
            }
            assert!(
                geometry
                    .pieces
                    .iter()
                    .any(|piece| piece.source_cell == p.coord && piece.part.collides())
            );
            for face in HexFace::LATERAL {
                if p.ports().port(face) != PortClass::Span {
                    continue;
                }
                let [a, b] = observed_hex::face_edge(face);
                let middle = Vec3::new((a.0 + b.0) as f32 / 2.0, 0.0, (a.1 + b.1) as f32 / 2.0);
                let normal = middle.normalize();
                let tangent = Vec3::new(-normal.z, 0.0, normal.x);
                for side in [-2.0, 0.0, 2.0] {
                    let from = Vec3::from_array(hex_origin(p.coord)) + middle - normal * 0.4
                        + tangent * side
                        + Vec3::Y * 1.9;
                    assert_eq!(
                        scene.ray_distance(from, normal, 0.8),
                        None,
                        "blocked join at {rotation} {face:?}"
                    );
                }
            }
        }
    }
}
