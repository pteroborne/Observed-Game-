//! The Last Courtyard: sparse pavilions enclosing one deliberately absent cell.
//! Roofs, rafters and ramp decks are real convex shells. Every threshold keeps
//! the catalogue's sill and lintel; the scene is assembled from ordinary WFC cards.

use super::entities::{
    Meta, lateral_port, stair_node, tile_cell, tile_light, vertical_port, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, P2, WALL, boxed, centroid, custom_plane, door_wall, edge, hex_slab, lerp,
    offset_inward, prism, side_plane, wall,
};
use super::{Builder, GENERATED_NOTE};

const BASE: i32 = 120;

/// Two parallel sloping faces give a thin roof or suspended walkway, without
/// filling all the space below it as a solid ramp wedge would.
fn shell(plan: &[P2], height: impl Fn(P2) -> f64, thickness: f64) -> String {
    let hint = centroid(plan);
    let mut out = String::from("{\n");
    for i in 0..plan.len() {
        out.push_str(&side_plane(plan[i], plan[(i + 1) % plan.len()], 0.0, hint));
    }
    let points = [plan[0], plan[1], plan[2]].map(|p| (p.0, p.1, height(p)));
    out.push_str(&custom_plane(points[0], points[1], points[2], true));
    let lower = points.map(|p| (p.0, p.1, p.2 - thickness));
    out.push_str(&custom_plane(lower[0], lower[1], lower[2], false));
    out.push_str("}\n");
    out
}

/// Low side bands and narrow posts leave the upper envelope open. The lintel
/// is still at 72, so an adjoining production door has identical headroom.
fn threshold(face: usize) -> String {
    let (a, b) = edge(face);
    let length = (b.0 - a.0).hypot(b.1 - a.1);
    let left = lerp(a, b, 0.5 - 36.0 / length);
    let right = lerp(a, b, 0.5 + 36.0 / length);
    let strip = |a, b, bottom, top| {
        let (ia, ib) = offset_inward(a, b, WALL);
        prism(&[a, b, ib, ia], bottom, top, None, 0.0, 0.0)
    };
    let mut out = strip(a, left, 0.0, 28.0);
    out.push_str(&strip(right, b, 0.0, 28.0));
    out.push_str(&strip(lerp(a, b, 0.5 - 41.0 / length), left, 28.0, LEVEL));
    out.push_str(&strip(right, lerp(a, b, 0.5 + 41.0 / length), 28.0, LEVEL));
    out.push_str(&strip(left, right, 72.0, 78.0));
    out
}

fn pavilion(name: &str, archetype: &str, variant: i32, doors: &[usize], roofed: bool) -> String {
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 0.0, 0.0);
    for face in 0..6 {
        brushes.push_str(&if doors.contains(&face) {
            threshold(face)
        } else {
            wall(face, 0.0, 28.0)
        });
    }
    let mut lights = String::new();
    if roofed {
        // All interior corners remain within radius 104 for sixfold rotation.
        for x in [-58.0, 58.0] {
            for y in [-42.0, 42.0] {
                brushes.push_str(&boxed(
                    (x - 3.0, y - 3.0, FLOOR_TOP),
                    (x + 3.0, y + 3.0, 101.0),
                ));
            }
        }
        for y in [-42.0, 42.0] {
            brushes.push_str(&boxed((-65.0, y - 4.0, 91.0), (65.0, y + 4.0, 99.0)));
        }
        for sign in [-1.0, 1.0] {
            let height = |p: P2| 122.0 - sign * p.0 * 0.28;
            brushes.push_str(&shell(
                &[
                    (0.0, -58.0),
                    (sign * 85.0, -58.0),
                    (sign * 85.0, 58.0),
                    (0.0, 58.0),
                ],
                height,
                5.0,
            ));
            // Two countable rafters per roof plane, carried by the cross beams.
            for y in [-28.0, 28.0] {
                brushes.push_str(&shell(
                    &[
                        (0.0, y - 2.0),
                        (sign * 83.0, y - 2.0),
                        (sign * 83.0, y + 2.0),
                        (0.0, y + 2.0),
                    ],
                    |p| height(p) - 5.0,
                    5.0,
                ));
            }
        }
        brushes.push_str(&boxed((-3.0, -59.0, 119.0), (3.0, 59.0, 125.0)));
        // One short outer screen. It stays behind all direct doorway chords.
        if doors.contains(&3) {
            brushes.push_str(&boxed((-61.0, 14.0, 8.0), (-55.0, 42.0, 84.0)));
        } else {
            brushes.push_str(&boxed((-84.0, -18.0, 8.0), (-80.0, 18.0, 84.0)));
        }
        for y in [-42.0, 42.0] {
            brushes.push_str(&boxed((-16.0, y - 5.0, 87.0), (16.0, y + 5.0, 91.0)));
            lights.push_str(&tile_light(0.0, y, 85.0));
        }
    } else {
        for y in [-78.0, 78.0] {
            brushes.push_str(&boxed((-4.0, y - 4.0, 8.0), (4.0, y + 4.0, 43.0)));
            brushes.push_str(&boxed((-12.0, y - 5.0, 39.0), (12.0, y + 5.0, 43.0)));
            lights.push_str(&tile_light(0.0, y, 37.0));
        }
    }
    let mut out = format!("// Last Courtyard: {name}. Architecture only.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(&format!("authored/{name}"), archetype, variant, 1, 3)
            .with_register_scope("thinning")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, "solid"));
    for &face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("courtyard_{face}"),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

pub fn corner() -> String {
    pavilion("courtyard_corner", "hall_turn_120", BASE, &[0, 4], true)
}
pub fn deck() -> String {
    pavilion("courtyard_deck", "hall_turn_120", BASE + 1, &[0, 4], false)
}
pub fn fork() -> String {
    pavilion(
        "courtyard_fork",
        "hall_junction_3way",
        BASE,
        &[0, 2, 4],
        true,
    )
}
pub fn fork_deck() -> String {
    pavilion(
        "courtyard_fork_deck",
        "hall_junction_3way",
        BASE + 1,
        &[0, 2, 4],
        false,
    )
}
pub fn lookout() -> String {
    pavilion("courtyard_lookout", "hall_straight", BASE, &[0, 3], true)
}

pub fn ascent() -> String {
    let height = |p: P2| FLOOR_TOP + (p.0 + 112.0) * LEVEL / 224.0;
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 0.0, 0.0);
    brushes.push_str(&shell(
        &[
            (-112.0, -36.0),
            (112.0, -36.0),
            (112.0, 36.0),
            (-112.0, 36.0),
        ],
        height,
        8.0,
    ));
    for side in [-1.0, 1.0] {
        let y = side * 44.0;
        for x in [-90.0, -30.0, 30.0, 90.0] {
            brushes.push_str(&boxed(
                (x - 3.0, y - 3.0, FLOOR_TOP),
                (x + 3.0, y + 3.0, height((x, y)) + 20.0),
            ));
        }
        brushes.push_str(&shell(
            &[
                (-104.0, y - 3.0),
                (104.0, y - 3.0),
                (104.0, y + 3.0),
                (-104.0, y + 3.0),
            ],
            |p| height(p) + 20.0,
            5.0,
        ));
    }
    for face in [1, 2, 4, 5] {
        brushes.push_str(&wall(face, 0.0, 28.0));
    }
    brushes.push_str(&door_wall(3, 0.0, LEVEL, FLOOR_TOP, 72.0, 8.0, 6.0));
    brushes.push_str(&door_wall(
        0,
        0.0,
        2.0 * LEVEL,
        LEVEL + FLOOR_TOP,
        LEVEL + 72.0,
        8.0,
        6.0,
    ));
    let mut lights = String::new();
    for x in [-70.0, 70.0] {
        let z = height((x, 0.0)) + 28.0;
        brushes.push_str(&boxed((x - 4.0, 57.0, 8.0), (x + 4.0, 65.0, z + 4.0)));
        lights.push_str(&tile_light(x, 55.0, z));
    }
    let mut out = format!("// Last Courtyard: suspended full-storey ascent.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/courtyard_ascent", "hall_ramp", BASE, 2, 3)
            .with_register_scope("thinning")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 2, "ramp"));
    out.push_str(&lateral_port(3, "door", "courtyard_entry", 0, 0, 0));
    out.push_str(&vertical_port("up", "ramp_open", "courtyard_ascent", 0));
    for index in 0..5u16 {
        let x = -112.0 + f64::from(index) * 56.0;
        out.push_str(&stair_node(index, x, 0.0, height((x, 0.0))));
    }
    out.push_str(&lights);
    out
}

pub fn builders() -> Vec<Builder> {
    vec![
        ("courtyard_corner", corner),
        ("courtyard_deck", deck),
        ("courtyard_fork", fork),
        ("courtyard_fork_deck", fork_deck),
        ("courtyard_lookout", lookout),
        ("courtyard_ascent", ascent),
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
    fn every_pavilion_door_pair_and_the_suspended_ascent_are_walkable() {
        let config = FpsConfig::default();
        for (name, build) in builders() {
            let tile = crate::parse_authored_module(&build())
                .expect("valid tile")
                .prototype;
            let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
            let doors: Vec<_> = HexFace::LATERAL
                .into_iter()
                .filter(|&face| tile.signature.port(face) == PortClass::Door)
                .collect();
            let threshold = |face| {
                let [a, b] = face_edge(face);
                #[allow(clippy::cast_precision_loss)]
                Vec3::new((a.0 + b.0) as f32 * 0.45, 0.5, (a.1 + b.1) as f32 * 0.45)
            };
            let paths: Vec<_> = if name == "courtyard_ascent" {
                vec![
                    (Vec3::new(-6.7, 0.5, 0.0), Vec3::new(6.7, 8.5, 0.0)),
                    (Vec3::new(6.7, 8.5, 0.0), Vec3::new(-6.7, 0.5, 0.0)),
                ]
            } else {
                doors
                    .iter()
                    .flat_map(|&entry| {
                        doors
                            .iter()
                            .filter(move |&&exit| entry != exit)
                            .map(move |&exit| (threshold(entry), threshold(exit)))
                    })
                    .collect()
            };
            for (start, goal) in paths {
                let mut body = FpsBody::spawned(start + Vec3::Y * (config.half_height + 0.15), 0.0);
                let mut reached = false;
                for _ in 0..900 {
                    let feet = body.position - Vec3::Y * config.half_height;
                    assert!(
                        feet.y >= 0.35,
                        "{name}: fell on {start:?} -> {goal:?}: {feet:?}"
                    );
                    if (feet - goal).length() < 0.4 {
                        reached = true;
                        break;
                    }
                    let delta = goal - feet;
                    body.yaw = delta.x.atan2(-delta.z);
                    let report = step_character(
                        &scene,
                        &mut body,
                        PlayerIntent {
                            // Brake at the landing instead of running off the
                            // isolated tile while the capsule settles downhill.
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
                    "{name}: blocked {start:?} -> {goal:?} at {:?}",
                    body.position
                );
            }
        }
    }

    #[test]
    fn courtyard_sources_reproduce_and_serve_real_demands() {
        super::super::assert_reproduces(&builders());
        let demands = observed_facility::hex_wfc::geometry_demands();
        for (name, build) in builders() {
            let module = crate::parse_authored_module(&build())
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert!(
                demands
                    .iter()
                    .any(|d| d.archetype == module.prototype.key.archetype
                        && d.signature == module.prototype.signature),
                "{name} cannot be selected by WFC"
            );
        }
    }
}
