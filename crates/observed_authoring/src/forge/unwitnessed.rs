//! The Unfinished Crossing: unrailed voids, recessed piers and suspended ascents.
//! Ordinary Megastructure candidates; the three-storey landmark is a lab layout.

use super::entities::{
    Meta, deck_node, lateral_port, stair_node, tile_cell, tile_light, vertical_port, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, P2, band, boxed, centroid, corners, custom_plane, door_wall, hex_slab, lerp,
    prism, side_plane, wall,
};
use super::{Builder, GENERATED_NOTE};

const BASE: i32 = 160;

#[derive(Clone, Copy)]
enum Form {
    Gallery,
    Crossing,
    Gate,
    Fork,
    Arrival,
    Break,
}

fn ring() -> String {
    let hex = corners();
    let mut out = String::new();
    for face in 0..6 {
        let a = hex[face];
        let b = hex[(face + 1) % 6];
        out.push_str(&prism(
            &[a, b, (b.0 * 0.45, b.1 * 0.45), (a.0 * 0.45, a.1 * 0.45)],
            0.0,
            FLOOR_TOP,
            None,
            0.0,
            0.0,
        ));
    }
    out
}

/// Split faces leave a deep vertical recess in each oversized pier.
/// The buttress and two flanges are independent convex solids, all in-cell.
fn piers(top: f64) -> String {
    let mut out = String::new();
    for sign in [-1.0, 1.0] {
        let y = sign * 83.0;
        out.push_str(&boxed((-22.0, y - 12.0, 0.0), (22.0, y + 12.0, 28.0)));
        for x in [-14.0, 14.0] {
            out.push_str(&prism(
                &[
                    (x - 7.0, y - 9.0),
                    (x + 7.0, y - 9.0),
                    (x + 7.0, y + 9.0),
                    (x - 7.0, y + 9.0),
                ],
                28.0,
                top,
                None,
                3.0,
                0.0,
            ));
        }
        out.push_str(&boxed((-21.0, y - 9.0, top - 10.0), (21.0, y + 9.0, top)));
    }
    out
}

fn room(name: &str, archetype: &str, doors: &[usize], form: Form) -> String {
    let open = matches!(form, Form::Gallery | Form::Crossing | Form::Break);
    let mut brushes = match form {
        Form::Gallery | Form::Crossing => ring(),
        Form::Break => prism(
            &[
                (112.0, 64.0),
                (112.0, -64.0),
                (0.0, -128.0),
                (-48.0, -100.5714),
                (-48.0, 100.5714),
                (0.0, 128.0),
            ],
            0.0,
            FLOOR_TOP,
            None,
            0.0,
            0.0,
        ),
        _ => hex_slab(0.0, FLOOR_TOP, 0.0, 0.0),
    };
    if matches!(form, Form::Crossing) {
        brushes.push_str(&boxed((-56.0, -18.0, 0.0), (56.0, 18.0, FLOOR_TOP)));
    }
    for face in 0..6 {
        brushes.push_str(&if doors.contains(&face) {
            door_wall(face, 0.0, LEVEL, 0.0, 72.0, 12.0, 6.0)
        } else {
            wall(face, 0.0, 72.0)
        });
        if !doors.contains(&face) {
            // Square-section service conduits above the closed thresholds.
            brushes.push_str(&band(face, 0.0, 20.0, 100.0, 116.0));
        }
    }
    brushes.push_str(&piers(LEVEL));
    if matches!(form, Form::Gate | Form::Arrival) {
        brushes.push_str(&prism(
            &[(-88.0, -52.0), (88.0, -52.0), (88.0, 52.0), (-88.0, 52.0)],
            112.0,
            LEVEL,
            None,
            2.0,
            2.0,
        ));
    }
    let mut lights = String::new();
    for sign in [-1.0, 1.0] {
        let y = sign * 73.0;
        brushes.push_str(&boxed((-5.0, y - 2.0, 42.0), (5.0, y + 2.0, 66.0)));
        lights.push_str(&tile_light(0.0, y - sign * 3.0, 54.0));
    }
    let variant = BASE + i32::from(matches!(form, Form::Crossing | Form::Arrival));
    let mut out = format!("// The Unfinished Crossing: {name}.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(&format!("authored/{name}"), archetype, variant, 1, 3)
            .with_register_scope("megastructure")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, if open { "open" } else { "solid" }));
    for &face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("unwitnessed_{face}"),
            0,
            0,
            0,
        ));
    }
    if matches!(form, Form::Gallery) {
        // Keep navigation on the unrailed ring, clear of its actual aperture.
        for index in 0..13u16 {
            let face = usize::from(index / 2) % 6;
            let a = corners()[face];
            let b = corners()[(face + 1) % 6];
            let p = if index % 2 == 0 { a } else { lerp(a, b, 0.5) };
            let y = if p.0.abs() < 0.1 {
                p.1.signum() * 62.0
            } else {
                p.1 * 0.70
            };
            out.push_str(&deck_node(index, p.0 * 0.70, y, FLOOR_TOP));
        }
    }
    out.push_str(&lights);
    out
}

fn suspended_slab(plan: &[P2], height: impl Fn(P2) -> f64) -> String {
    let hint = centroid(plan);
    let mut out = String::from("{\n");
    for i in 0..plan.len() {
        out.push_str(&side_plane(plan[i], plan[(i + 1) % plan.len()], 0.0, hint));
    }
    let p = [plan[0], plan[1], plan[2]].map(|p| (p.0, p.1, height(p)));
    out.push_str(&custom_plane(p[0], p[1], p[2], true));
    let p = p.map(|p| (p.0, p.1, p.2 - 8.0));
    out.push_str(&custom_plane(p[0], p[1], p[2], false));
    out.push_str("}\n");
    out
}

pub fn ascent() -> String {
    let height = |p: P2| FLOOR_TOP + (p.0 + 112.0) * LEVEL / 224.0;
    let mut brushes = suspended_slab(
        &[
            (-112.0, -36.0),
            (112.0, -36.0),
            (112.0, 36.0),
            (-112.0, 36.0),
        ],
        height,
    );
    brushes.push_str(&hex_slab(0.0, FLOOR_TOP, 0.0, 0.0));
    for face in [1, 2, 4, 5] {
        brushes.push_str(&wall(face, 0.0, 2.0 * LEVEL));
        brushes.push_str(&band(face, 8.0, 24.0, 216.0, 232.0));
    }
    brushes.push_str(&door_wall(3, 0.0, LEVEL, 0.0, 72.0, 12.0, 6.0));
    brushes.push_str(&wall(3, LEVEL, 2.0 * LEVEL));
    brushes.push_str(&door_wall(
        0,
        0.0,
        2.0 * LEVEL,
        LEVEL + FLOOR_TOP,
        LEVEL + 72.0,
        12.0,
        6.0,
    ));
    brushes.push_str(&piers(2.0 * LEVEL));
    let mut lights = String::new();
    for x in [-64.0, 64.0] {
        let z = height((x, 0.0)) + 44.0;
        brushes.push_str(&boxed((x - 5.0, 70.0, z - 12.0), (x + 5.0, 76.0, z + 12.0)));
        lights.push_str(&tile_light(x, 68.0, z));
    }
    let mut out =
        format!("// The Unfinished Crossing: suspended eight-metre ascent.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/unwitnessed_ascent", "hall_ramp", BASE, 2, 3)
            .with_register_scope("megastructure")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 2, "ramp"));
    out.push_str(&lateral_port(3, "door", "unwitnessed_entry", 0, 0, 0));
    out.push_str(&vertical_port("up", "ramp_open", "unwitnessed_ascent", 0));
    for index in 0..5u16 {
        let x = -112.0 + f64::from(index) * 56.0;
        out.push_str(&stair_node(index, x, 0.0, height((x, 0.0))));
    }
    out.push_str(&lights);
    out
}

pub fn gallery() -> String {
    room(
        "unwitnessed_gallery",
        "hall_turn_120",
        &[0, 2],
        Form::Gallery,
    )
}
pub fn crossing() -> String {
    room(
        "unwitnessed_crossing",
        "hall_straight",
        &[0, 3],
        Form::Crossing,
    )
}
pub fn gate() -> String {
    room("unwitnessed_gate", "hall_straight", &[0, 3], Form::Gate)
}
pub fn fork() -> String {
    room(
        "unwitnessed_fork",
        "hall_junction_3way",
        &[0, 2, 4],
        Form::Fork,
    )
}
pub fn arrival() -> String {
    room(
        "unwitnessed_arrival",
        "hall_turn_120",
        &[0, 2],
        Form::Arrival,
    )
}
pub fn broken() -> String {
    room("unwitnessed_break", "hall_turn_60", &[0, 1], Form::Break)
}

pub fn builders() -> Vec<Builder> {
    vec![
        ("unwitnessed_gallery", gallery),
        ("unwitnessed_crossing", crossing),
        ("unwitnessed_gate", gate),
        ("unwitnessed_fork", fork),
        ("unwitnessed_arrival", arrival),
        ("unwitnessed_break", broken),
        ("unwitnessed_ascent", ascent),
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
    fn sources_reproduce_and_serve_real_megastructure_demands() {
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

    fn walk(name: &str, tile: &crate::TilePrototype, start: Vec3, goal: Vec3, deck: bool) {
        let config = FpsConfig::default();
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        let mut body = FpsBody::spawned(start + Vec3::Y * (config.half_height + 0.1), 0.0);
        for _ in 0..2400 {
            let feet = body.position - Vec3::Y * config.half_height;
            assert!(feet.y >= 0.35, "{name}: fell en route to {goal:?}");
            if (feet - goal).length() < 0.3 {
                return;
            }
            let target = if deck {
                tile.deck.step_toward(feet, goal).unwrap_or(goal)
            } else {
                goal
            };
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
        panic!(
            "{name}: blocked at {:?} on {start:?} -> {goal:?}",
            body.position
        );
    }

    #[test]
    fn all_door_pairs_bridge_ring_and_both_ascent_directions_are_walkable() {
        let threshold = |face| {
            let [a, b] = face_edge(face);
            Vec3::new(
                f32::from(i16::try_from(a.0 + b.0).expect("local coordinate")) * 0.45,
                0.5,
                f32::from(i16::try_from(a.1 + b.1).expect("local coordinate")) * 0.45,
            )
        };
        for (name, build) in builders() {
            let tile = crate::parse_authored_module(&build())
                .expect("valid tile")
                .prototype;
            let doors: Vec<_> = HexFace::LATERAL
                .into_iter()
                .filter(|&f| tile.signature.port(f) == PortClass::Door)
                .collect();
            for &a in &doors {
                for &b in &doors {
                    if a != b {
                        walk(
                            name,
                            &tile,
                            threshold(a),
                            threshold(b),
                            name == "unwitnessed_gallery",
                        );
                    }
                }
            }
            if name == "unwitnessed_ascent" {
                walk(
                    name,
                    &tile,
                    Vec3::new(-6.7, 0.5, 0.0),
                    Vec3::new(6.7, 8.5, 0.0),
                    false,
                );
                walk(
                    name,
                    &tile,
                    Vec3::new(6.7, 8.5, 0.0),
                    Vec3::new(-6.7, 0.5, 0.0),
                    false,
                );
                // Real walkable air below the thin slab; a solid wedge fails this.
                walk(
                    name,
                    &tile,
                    Vec3::new(3.0, 0.5, 0.0),
                    Vec3::new(5.8, 0.5, 0.0),
                    false,
                );
            } else if doors.len() == 1 {
                walk(
                    name,
                    &tile,
                    threshold(doors[0]),
                    Vec3::new(-2.2, 0.5, 0.0),
                    false,
                );
            }
        }
    }

    #[test]
    fn unfinished_cantilever_has_a_real_fall_and_no_phantom_exit() {
        let tile = crate::parse_authored_module(&broken())
            .expect("valid tile")
            .prototype;
        assert_eq!(tile.signature.port(HexFace::West), PortClass::Sealed);
        let config = FpsConfig::default();
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        let mut body = FpsBody::spawned(
            Vec3::new(-2.0, 0.6 + config.half_height, 0.0),
            -std::f32::consts::FRAC_PI_2,
        );
        for _ in 0..180 {
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
            if body.position.y < -2.0 {
                return;
            }
        }
        panic!(
            "cantilever should end over a void, body at {:?}",
            body.position
        );
    }
}
