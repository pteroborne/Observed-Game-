//! The Catalogue of Missing Rooms: enclosed shelves around a shared helix.
//! Six Infinite Gallery candidates compose with the existing, unrotated tower
//! family. The landmark layout lives in docs/compositions/missing_rooms.

use super::entities::{Meta, deck_node, lateral_port, tile_cell, wall_fixture, worldspawn};
use super::geometry::{
    DOOR_TOP, FLOOR_TOP, LEVEL, P2, WALL, band, boxed, corners, door_wall, hex_slab, lerp, prism,
    wall,
};
use super::{Builder, GENERATED_NOTE};

const BASE: i32 = 140;
const INNER: f64 = 0.48;

fn scaled(p: P2, scale: f64) -> P2 {
    (p.0 * scale, p.1 * scale)
}

#[derive(Clone, Copy)]
enum Form {
    Reading,
    Shaft,
    Bridge,
    Alcove,
}

fn ring(bridge: bool) -> String {
    let hex = corners();
    let mut out = String::new();
    for face in 0..6 {
        let a = hex[face];
        let b = hex[(face + 1) % 6];
        out.push_str(&prism(
            &[a, b, scaled(b, INNER), scaled(a, INNER)],
            0.0,
            FLOOR_TOP,
            None,
            0.0,
            0.0,
        ));
        let spans: &[(f64, f64)] = if bridge && (face == 0 || face == 3) {
            &[(0.0, 0.20), (0.80, 1.0)]
        } else {
            &[(0.0, 1.0)]
        };
        for &(start, end) in spans {
            let a = lerp(a, b, start);
            let b = lerp(hex[face], b, end);
            out.push_str(&prism(
                &[
                    scaled(a, INNER),
                    scaled(b, INNER),
                    scaled(b, INNER + 0.035),
                    scaled(a, INNER + 0.035),
                ],
                FLOOR_TOP,
                16.0,
                None,
                1.0,
                0.0,
            ));
        }
    }
    if bridge {
        out.push_str(&boxed((-60.0, -18.0, 0.0), (60.0, 18.0, FLOOR_TOP)));
    }
    out
}

fn room(name: &str, archetype: &str, variant: i32, doors: &[usize], form: Form) -> String {
    let open = matches!(form, Form::Shaft | Form::Bridge);
    let mut brushes = if open {
        ring(matches!(form, Form::Bridge))
    } else {
        hex_slab(0.0, FLOOR_TOP, 0.0, 0.0)
    };
    if !open {
        brushes.push_str(&hex_slab(120.0, LEVEL, 0.0, 2.0));
    }
    let sealed: Vec<_> = (0..6).filter(|face| !doors.contains(face)).collect();
    for face in 0..6 {
        brushes.push_str(&if doors.contains(&face) {
            // The floor supplies the sill, avoiding a duplicate collider.
            door_wall(face, 0.0, LEVEL, 0.0, DOOR_TOP, 8.0, 6.0)
        } else {
            wall(face, 0.0, LEVEL)
        });
    }
    for &face in &sealed {
        if matches!(form, Form::Alcove) && (face == 1 || face == 2) {
            continue;
        }
        let courses = if open { 2 } else { 5 };
        for course in 0..courses {
            let z = if open {
                36.0 + f64::from(course) * 48.0
            } else {
                24.0 + f64::from(course) * 20.0
            };
            brushes.push_str(&band(face, WALL, WALL + 18.0, z, z + 4.0));
        }
    }
    if matches!(form, Form::Alcove) {
        // One body-wide standing recess faces the aisle, clear of both doors.
        brushes.push_str(&boxed((-16.0, -102.0, FLOOR_TOP), (-11.0, -72.0, 54.0)));
        brushes.push_str(&boxed((11.0, -102.0, FLOOR_TOP), (16.0, -72.0, 54.0)));
        brushes.push_str(&boxed((-16.0, -102.0, 54.0), (16.0, -72.0, 60.0)));
    }
    let mut lights = String::new();
    for &face in sealed.iter().take(2) {
        let (fixture, source) = wall_fixture(face, 0.5, 76.0, 30.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let mut out = format!("// The Catalogue of Missing Rooms: {name}.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(&format!("authored/{name}"), archetype, variant, 1, 3)
            .with_register_scope("infinite_gallery")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, if open { "open" } else { "solid" }));
    for &face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("index_{face}"),
            0,
            0,
            0,
        ));
    }
    if open {
        // A closed ring of edge and corner nodes keeps paths off the shaft.
        for index in 0..13u16 {
            let face = usize::from(index / 2) % 6;
            let a = corners()[face];
            let b = corners()[(face + 1) % 6];
            let p = scaled(if index % 2 == 0 { a } else { lerp(a, b, 0.5) }, 0.70);
            out.push_str(&deck_node(index, p.0, p.1, FLOOR_TOP));
        }
    }
    out.push_str(&lights);
    out
}

pub fn reading() -> String {
    room(
        "index_reading",
        "hall_turn_120",
        BASE,
        &[0, 2],
        Form::Reading,
    )
}
pub fn fork() -> String {
    room(
        "index_fork",
        "hall_junction_3way",
        BASE,
        &[0, 1, 5],
        Form::Reading,
    )
}
pub fn junction() -> String {
    room(
        "index_junction",
        "hall_junction_4way",
        BASE,
        &[0, 1, 3, 5],
        Form::Reading,
    )
}
pub fn shaft() -> String {
    room(
        "index_shaft",
        "hall_turn_120",
        BASE + 1,
        &[0, 2],
        Form::Shaft,
    )
}
pub fn bridge() -> String {
    room(
        "index_bridge",
        "hall_junction_4way",
        BASE + 1,
        &[0, 1, 3, 5],
        Form::Bridge,
    )
}
pub fn alcove() -> String {
    room("index_alcove", "hall_straight", BASE, &[0, 3], Form::Alcove)
}

pub fn builders() -> Vec<Builder> {
    vec![
        ("index_reading", reading),
        ("index_fork", fork),
        ("index_junction", junction),
        ("index_shaft", shaft),
        ("index_bridge", bridge),
        ("index_alcove", alcove),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Vec2, Vec3};
    use observed_hex::{HexFace, PortClass, face_edge};
    use observed_traversal::rapier_controller::{RapierTraversalScene, step_character};
    use observed_traversal::{FpsBody, FpsConfig};
    use player_input::PlayerIntent;

    #[test]
    fn index_sources_reproduce_and_serve_real_demands() {
        super::super::assert_reproduces(&builders());
        let demands = observed_facility::hex_wfc::geometry_demands();
        for (name, build) in builders() {
            let tile = crate::parse_authored_module(&build())
                .expect("valid tile")
                .prototype;
            assert!(
                demands
                    .iter()
                    .any(|d| d.archetype == tile.key.archetype && d.signature == tile.signature),
                "{name} is not demanded by WFC"
            );
        }
    }

    #[test]
    fn every_door_pair_is_walkable_around_shelves_and_shafts() {
        let config = FpsConfig::default();
        for (name, build) in builders() {
            let tile = crate::parse_authored_module(&build())
                .expect("valid tile")
                .prototype;
            let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
            let doors: Vec<_> = HexFace::LATERAL
                .into_iter()
                .filter(|&f| tile.signature.port(f) == PortClass::Door)
                .collect();
            let threshold = |face| {
                let [a, b] = face_edge(face);
                Vec3::new(
                    f32::from(i16::try_from(a.0 + b.0).expect("local coordinate")) * 0.45,
                    0.5,
                    f32::from(i16::try_from(a.1 + b.1).expect("local coordinate")) * 0.45,
                )
            };
            for &entry in &doors {
                for &exit in &doors {
                    if entry == exit {
                        continue;
                    }
                    let goal = threshold(exit);
                    let mut body = FpsBody::spawned(
                        threshold(entry) + Vec3::Y * (config.half_height + 0.1),
                        0.0,
                    );
                    let mut reached = false;
                    for _ in 0..2400 {
                        let feet = body.position - Vec3::Y * config.half_height;
                        assert!(feet.y >= 0.35, "{name}: fell from {entry:?} to {exit:?}");
                        if (feet - goal).length() < 0.3 {
                            reached = true;
                            break;
                        }
                        let target = tile.deck.step_toward(feet, goal).unwrap_or(goal);
                        let delta = target - feet;
                        body.yaw = delta.x.atan2(-delta.z);
                        let report = step_character(
                            &scene,
                            &mut body,
                            PlayerIntent {
                                movement: Vec2::Y * Vec2::new(delta.x, delta.z).length().min(1.0),
                                ..PlayerIntent::default()
                            },
                            &config,
                            1.0 / 60.0,
                        );
                        assert!(!report.jumped);
                    }
                    assert!(
                        reached,
                        "{name}: blocked from {entry:?} to {exit:?} at {:?}",
                        body.position
                    );
                }
            }
        }
    }
    #[test]
    fn the_bridge_crossing_and_standing_alcove_are_physically_accessible() {
        let config = FpsConfig::default();
        for (source, start, goal) in [
            (
                bridge(),
                Vec3::new(-6.3, 0.5, 0.0),
                Vec3::new(6.3, 0.5, 0.0),
            ),
            (alcove(), Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, 0.5, 5.4)),
        ] {
            let tile = crate::parse_authored_module(&source)
                .expect("valid module")
                .prototype;
            let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
            let mut body = FpsBody::spawned(start + Vec3::Y * (config.half_height + 0.1), 0.0);
            let mut reached = false;
            for _ in 0..900 {
                let feet = body.position - Vec3::Y * config.half_height;
                assert!(feet.y >= 0.35, "fell during direct feature approach");
                if (feet - goal).length() < 0.3 {
                    reached = true;
                    break;
                }
                let delta = goal - feet;
                body.yaw = delta.x.atan2(-delta.z);
                let report = step_character(
                    &scene,
                    &mut body,
                    PlayerIntent {
                        movement: Vec2::Y * Vec2::new(delta.x, delta.z).length().min(1.0),
                        ..PlayerIntent::default()
                    },
                    &config,
                    1.0 / 60.0,
                );
                assert!(!report.jumped);
            }
            assert!(reached, "feature approach blocked at {:?}", body.position);
        }
    }
}
