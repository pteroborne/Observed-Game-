use super::*;
use observed_facility::hex_wfc::authored_last_promenade;
fn turn(mut p: Vec3, rotation: usize) -> Vec3 {
    for _ in 0..rotation {
        p = Vec3::new(0.5 * p.x - 0.875 * p.z, p.y, 6.0 / 7.0 * p.x + 0.5 * p.z);
    }
    p
}
#[test]
fn promenade_bridges_entries_and_real_falls_work_at_every_rotation_on_sky() {
    let mut world = HexWfcWorld::generate(
        SHOWCASE_SEED,
        HexWfcConfig {
            levels: 8,
            ..showcase().config
        },
    )
    .expect("eight-floor Sky fixture with local revisions");
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
        level: 7,
    };
    for rotation in 0..6 {
        let sectors = authored_last_promenade(world.config, anchor, rotation).unwrap();
        let mut fixture = world.clone();
        for p in sectors {
            fixture.placements.insert(p.coord, p);
            fixture
                .architecture
                .insert(p.coord, ArchitectureRegister::Thinning);
        }
        let geometry = HexWfcGeometrySnapshot::project(&fixture, &tiles()).expect("Sky projects");
        let count = geometry
            .pieces
            .iter()
            .filter(|p| sectors.iter().any(|s| s.coord == p.source_cell) && p.part.collides())
            .count();
        assert!(count <= 128, "hull budget: {count}");
        let scene = RapierTraversalScene::from_arena_spec(&geometry.arena);
        let centers = sectors.map(|p| Vec3::from_array(hex_origin(p.coord)) + Vec3::Y * 2.5);
        let mut circuit = [centers[0], centers[1], centers[2], centers[0]];
        for _ in 0..2 {
            walk_room_route(&scene, &circuit)
                .unwrap_or_else(|feet| panic!("rotation {rotation}: bridge blocked {feet:?}"));
            circuit.reverse();
        }
        let mut surviving = fixture.clone();
        for sector in sectors {
            let mut lower = sector;
            lower.coord.level -= 1;
            lower.archetype = HexArchetype::Expanse;
            lower.doors = 0b11_1111;
            surviving.placements.insert(lower.coord, lower);
            surviving
                .architecture
                .insert(lower.coord, ArchitectureRegister::Megastructure);
        }
        let supported = HexWfcGeometrySnapshot::project(&surviving, &tiles())
            .expect("surviving Reactor below Sky");
        let supported_scene = RapierTraversalScene::from_arena_spec(&supported.arena);
        for sector in sectors {
            let HexArchetype::LastPromenade { heading } = sector.archetype else {
                unreachable!()
            };
            let origin = Vec3::from_array(hex_origin(sector.coord));
            let at = |p| origin + turn(p, heading.index());
            let mut entry = [at(Vec3::new(-3.2, 0.5, -5.4)), at(Vec3::new(0.0, 2.5, 0.0))];
            for _ in 0..2 {
                walk_room_route(&scene, &entry)
                    .unwrap_or_else(|feet| panic!("rotation {rotation}: entry blocked {feet:?}"));
                entry.reverse();
            }
            assert!(
                scene
                    .ray_distance(at(Vec3::new(0.0, 4.0, 0.0)), Vec3::Y, 7.0)
                    .is_none(),
                "gathering landing opens to real sky"
            );
            assert!(
                scene
                    .ray_distance(at(Vec3::new(5.0, 1.0, 2.7)), Vec3::NEG_Y, 5.0)
                    .is_none(),
                "bridge edges expose real floor openings"
            );
            // Step off the real lip with the same character controller as the game.
            let config = FpsConfig::default();
            let mut body = FpsBody::spawned(at(Vec3::new(4.0, 2.5 + config.half_height, 0.0)), 0.0);
            let direction = turn(Vec3::Z, heading.index()).normalize();
            body.yaw = direction.x.atan2(-direction.z);
            for _ in 0..80 {
                step_character(
                    &scene,
                    &mut body,
                    PlayerIntent {
                        movement: Vec2::Y,
                        ..PlayerIntent::default()
                    },
                    &config,
                    1.0 / 60.0,
                );
            }
            assert!(
                body.position.y < origin.y - 1.0,
                "lip must permit an actual fall, not a painted pit: {:?}",
                body.position
            );
            // A real surviving lower tile catches the same fall on its roof.
            let mut caught = FpsBody::spawned(
                at(Vec3::new(4.0, 2.5 + config.half_height, 0.0)),
                direction.x.atan2(-direction.z),
            );
            for frame in 0..150 {
                step_character(
                    &supported_scene,
                    &mut caught,
                    PlayerIntent {
                        movement: if frame < 42 { Vec2::Y } else { Vec2::ZERO },
                        ..PlayerIntent::default()
                    },
                    &config,
                    1.0 / 60.0,
                );
            }
            let feet = caught.position - Vec3::Y * config.half_height;
            assert!(
                (feet.y - origin.y).abs() < 0.06 && caught.grounded,
                "lower roof catches fall: {feet:?}"
            );
            // Recover through the wide threshold and its ramp using ordinary jumps
            // and movement; no teleport, implicit safety floor or special recovery.
            for target in [
                at(Vec3::new(4.0, 0.0, -2.7)),
                at(Vec3::new(1.5, 0.0, -6.0)),
                at(Vec3::new(-1.5, 0.65, -6.0)),
                at(turn(Vec3::new(6.6, 0.5, 0.0), 4)),
                at(Vec3::new(0.0, 2.5, 0.0)),
            ] {
                let mut reached = false;
                for _ in 0..600 {
                    let feet = caught.position - Vec3::Y * config.half_height;
                    let toward = (target - feet).with_y(0.0);
                    if toward.length() < 0.25 && (feet.y - target.y).abs() < 0.35 {
                        reached = true;
                        break;
                    }
                    caught.yaw = toward.x.atan2(-toward.z);
                    let jump_pressed = caught.grounded && target.y - feet.y > 0.3;
                    step_character(
                        &supported_scene,
                        &mut caught,
                        PlayerIntent {
                            movement: Vec2::Y,
                            jump_pressed,
                            ..PlayerIntent::default()
                        },
                        &config,
                        1.0 / 60.0,
                    );
                }
                assert!(
                    reached,
                    "rotation {rotation}: recovery blocked at {:?} toward {target:?}",
                    caught.position
                );
            }
            // The canopy side blade is real, opaque cover for a waiting teammate.
            let eye = at(turn(Vec3::new(4.0, 2.0, 1.0), 4));
            let target = at(turn(Vec3::new(4.0, 2.0, 3.0), 4));
            assert!(
                scene
                    .ray_distance(eye, (target - eye).normalize(), eye.distance(target))
                    .is_some(),
                "waiting bay blocks a sightline"
            );
        }
    }
}
