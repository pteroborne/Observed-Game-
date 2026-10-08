//! The first initial-room construction reference: existing Backrooms arrival and
//! decision footprints. These fittings change neither ports nor room placement.
use super::super::entities::{tile_light, worldspawn};
use super::super::geometry::{FLOOR_TOP, band, boxed, cell_origin, translate};

/// Dress after height conversion: human-scale fittings must not be compressed
/// by the ordinary eight-to-three-metre enclosure conversion.
pub(super) fn dress(map: &mut quake_map::QuakeMap, world: usize, id: &str) {
    let (brushes, lights) = match id {
        "authored/room_start" => arrival(),
        "authored/room_decision" => decision(),
        _ => return,
    };
    let extra = quake_map::parse(&mut std::io::Cursor::new(format!(
        "{}{}",
        worldspawn(&brushes),
        lights
    )))
    .expect("reference fittings emit valid brushes");
    map.entities[world]
        .brushes
        .extend(extra.entities[0].brushes.clone());
    map.entities.extend(extra.entities.into_iter().skip(1));
}

fn trim(brush: String) -> String {
    brush.replace("__TB_empty", "observed_trim")
}

fn practical(x: f64, y: f64, half: f64) -> (String, String) {
    // Shallow, ceiling-backed housing: even a major keeps its body clearance.
    (
        trim(boxed((x - half, y - 5.0, 55.5), (x + half, y + 5.0, 57.75))),
        tile_light(x, y, 52.0),
    )
}

fn arrival() -> (String, String) {
    let mut brushes = String::new();
    // Nested jambs frame the only in-grid departure. Their lane is at least
    // 4.5 m wide. Shallow downstands retain 2.96 m of interior body clearance;
    // the boundary's full 3 m doorway aperture remains unchanged.
    for (x, half) in [(62.0, 48.0), (88.0, 40.0)] {
        for sign in [-1.0, 1.0] {
            brushes.push_str(&trim(boxed(
                (x - 3.0, sign * half - 3.0, FLOOR_TOP),
                (x + 3.0, sign * half + 3.0, 56.0),
            )));
        }
        brushes.push_str(&trim(boxed(
            // Keep exposed faces off the cap's 56/58 planes: render materials
            // differ, so a coplanar beam would reintroduce ceiling shimmer.
            (x - 3.0, -half - 3.0, 55.5),
            (x + 3.0, half + 3.0, 57.75),
        )));
    }
    // An empty check-in counter establishes body scale and an asymmetric
    // return landmark. It stays against the sealed south-west perimeter.
    brushes.push_str(&band(2, 8.0, 20.0, FLOOR_TOP, 21.6));
    brushes.push_str(&trim(band(2, 7.0, 21.0, 21.6, 23.2)));
    let (fixture, lights) = practical(72.0, 0.0, 24.0);
    brushes.push_str(&fixture);
    (brushes, lights)
}

fn decision() -> (String, String) {
    let mut brushes = String::new();
    // Seen from the west arrival, the straight eastern exit stays exposed,
    // while this off-axis screen hides the side threshold until approached.
    // It leaves all three cell centres and their connecting lines clear.
    brushes.push_str(&boxed((-32.0, -86.0, FLOOR_TOP), (19.2, -80.0, 56.0)));
    brushes.push_str(&trim(boxed(
        (-33.0, -87.0, FLOOR_TOP),
        (-30.0, -79.0, 56.0),
    )));
    brushes.push_str(&trim(boxed((17.2, -87.0, FLOOR_TOP), (20.2, -79.0, 56.0))));
    // The side room has an unmistakable low waiting ledge; the long eastern
    // room has paired full-height service piers instead. These are places a
    // player can recognise on the way back, rather than different door colours.
    let (cx, cy) = cell_origin(0, 1);
    for face in [2, 3] {
        brushes.push_str(&translate(
            &band(face, 8.0, 18.0, FLOOR_TOP, 15.2),
            cx,
            cy,
            0.0,
        ));
        brushes.push_str(&translate(
            &trim(band(face, 7.0, 19.0, 15.2, 17.2)),
            cx,
            cy,
            0.0,
        ));
    }
    let (bx, by) = cell_origin(1, 0);
    for y in [-48.0, 48.0] {
        brushes.push_str(&translate(
            &trim(boxed((68.0, y - 4.0, FLOOR_TOP), (76.0, y + 4.0, 56.0))),
            bx,
            by,
            0.0,
        ));
    }
    let (fixture, lights) = practical(112.0, 0.0, 32.0);
    brushes.push_str(&fixture);
    // The side bay already has its own centred authored source.
    brushes.push_str(&practical(cx, cy, 16.0).0);
    (brushes, lights)
}

#[cfg(test)]
mod tests {
    use glam::{Vec2, Vec3, Vec3Swizzles};
    use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
    use observed_traversal::{FpsBody, FpsConfig};
    use player_input::PlayerIntent;

    fn module(role: &str) -> crate::AuthoredModule {
        let source = match role {
            "Start" => super::super::super::rooms::room_start(),
            "Decision" => super::super::super::rooms::room_decision(),
            _ => unreachable!(),
        };
        crate::parse_authored_module(&super::super::lower_source(&source).expect("low edition"))
            .expect("valid reference")
    }

    fn scene(module: &crate::AuthoredModule) -> RapierTraversalScene {
        let mut arena = module.prototype.arena_spec();
        arena.safety_center = Vec3::new(7.0, 4.0, 6.0);
        arena.safety_half = Vec3::splat(40.0);
        RapierTraversalScene::from_arena_spec(&arena)
    }

    fn walk(scene: &RapierTraversalScene, path: &[Vec3]) {
        let config = FpsConfig::deliberate_rapier();
        let mut body = FpsBody::spawned(path[0] + Vec3::Y * (config.half_height + 0.05), 0.0);
        for &goal in &path[1..] {
            let mut arrived = false;
            for _ in 0..900 {
                let feet = body.position - Vec3::Y * config.half_height;
                assert!(feet.y > 0.35, "fell at {feet:?}");
                if (feet - goal).length() < 0.25 {
                    arrived = true;
                    break;
                }
                let delta = goal - feet;
                body.yaw = delta.x.atan2(-delta.z);
                let report = step_character(
                    scene,
                    &mut body,
                    PlayerIntent {
                        movement: Vec2::Y * delta.xz().length().min(1.0),
                        ..PlayerIntent::default()
                    },
                    &config,
                    1.0 / 60.0,
                );
                assert!(!report.jumped);
            }
            assert!(arrived, "blocked at {:?}, goal {goal:?}", body.position);
        }
    }

    #[test]
    fn arrival_departure_and_all_six_decision_routes_are_walkable() {
        walk(
            &scene(&module("Start")),
            &[Vec3::new(-5.8, 0.5, 0.0), Vec3::new(5.8, 0.5, 0.0)],
        );
        let scene = scene(&module("Decision"));
        let centres = [
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(14.0, 0.5, 0.0),
            Vec3::new(7.0, 0.5, 12.0),
        ];
        let entrances = [
            Vec3::new(-5.8, 0.5, 0.0),
            Vec3::new(19.8, 0.5, 0.0),
            Vec3::new(9.9, 0.5, 17.0),
        ];
        for a in 0..3 {
            for b in 0..3 {
                if a != b {
                    walk(
                        &scene,
                        &[entrances[a], centres[a], centres[b], entrances[b]],
                    );
                }
            }
        }
    }

    #[test]
    fn decision_screen_requires_approach_but_keeps_the_long_exit_visible() {
        let scene = scene(&module("Decision"));
        let arrival = Vec3::new(-5.8, 2.1, 0.0);
        let side = Vec3::new(10.5, 2.1, 18.0);
        assert!(
            !scene.line_is_clear(arrival, side),
            "side threshold is exposed at arrival"
        );
        assert!(
            scene.line_is_clear(Vec3::new(0.0, 2.1, 0.0), side),
            "side threshold never reveals"
        );
        assert!(
            scene.line_is_clear(arrival, Vec3::new(20.5, 2.1, 0.0)),
            "long exit is hidden"
        );
    }

    #[test]
    fn reference_fittings_keep_ports_footprint_and_supported_lights() {
        for role in ["Start", "Decision"] {
            let module = module(role);
            let blueprint = observed_facility::hex_wfc::blueprint_for_role(match role {
                "Start" => observed_facility::map_spec::RoomRole::Start,
                _ => observed_facility::map_spec::RoomRole::Decision,
            });
            let footprint = module
                .footprint
                .iter()
                .map(|cell| (i32::from(cell.q), i32::from(cell.r), i32::from(cell.level)))
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(footprint, blueprint.cells.iter().copied().collect());
            assert_eq!(module.ports.len(), blueprint.named_ports.len());
            assert!(
                module
                    .prototype
                    .lights
                    .iter()
                    .all(|light| light.attachment.is_some())
            );
            assert!(module.prototype.hulls.len() <= 128);
            for (surface, hull) in module
                .prototype
                .surfaces
                .iter()
                .zip(&module.prototype.hulls)
            {
                let min_y = hull.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
                let max_y = hull.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
                if *surface == Some(crate::HullSurface::Trim) && min_y > 3.4 && max_y - min_y < 0.3
                {
                    assert!(
                        min_y < 3.5 && max_y < 3.625,
                        "trim shares an exposed ceiling-cap plane"
                    );
                }
            }
        }
    }

    #[test]
    fn major_sized_bodies_keep_clear_centres_and_departure_lanes() {
        use observed_traversal::rapier_controller::step_solid_character_with_settings;
        let profile = observed_traversal::TraversalRuntimeProfile::canonical_hex();
        let mut config = profile.controller();
        config.radius = 1.35;
        config.half_height = 1.4;
        config.walk_speed = 7.0;
        for (role, path) in [
            (
                "Start",
                vec![Vec3::new(-4.0, 0.5, 0.0), Vec3::new(5.8, 0.5, 0.0)],
            ),
            (
                "Decision",
                vec![
                    Vec3::new(-4.0, 0.5, 0.0),
                    Vec3::new(0.0, 0.5, 0.0),
                    Vec3::new(7.0, 0.5, 12.0),
                    Vec3::new(14.0, 0.5, 0.0),
                    Vec3::new(19.8, 0.5, 0.0),
                ],
            ),
        ] {
            let scene = scene(&module(role));
            let mut body = FpsBody::spawned(path[0] + Vec3::Y * (config.half_height + 0.02), 0.0);
            for goal in &path[1..] {
                let mut arrived = false;
                for _ in 0..900 {
                    let feet = body.position - Vec3::Y * config.half_height;
                    if (feet - *goal).length() < 0.25 {
                        arrived = true;
                        break;
                    }
                    let delta = *goal - feet;
                    body.yaw = delta.x.atan2(-delta.z);
                    let report = step_solid_character_with_settings(
                        &scene,
                        &mut body,
                        PlayerIntent {
                            movement: Vec2::Y * delta.xz().length().min(1.0),
                            ..PlayerIntent::default()
                        },
                        &config,
                        profile.rapier(),
                        1.0 / 60.0,
                    );
                    assert!(!report.recovered, "major recovered in {role}");
                    assert!(feet.y >= 0.49, "major lost support in {role}");
                }
                assert!(
                    arrived,
                    "major blocked in {role} at {:?}, goal {goal:?}",
                    body.position
                );
            }
        }
    }
}
