//! The Same Door Twice: repeated coved rooms and floating ceiling rafts.
//! Four ordinary Overlit Grid candidates form two loops and an enclosed ascent.

use super::entities::{
    Meta, lateral_port, stair_node, tile_cell, tile_light, vertical_port, wall_fixture, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, P2, band, boxed, centroid, corners, custom_plane, door_wall, edge,
    flat_plane, hex_slab, offset_inward, prism, side_plane, sloped_prism, wall,
};
use super::{Builder, GENERATED_NOTE};

const BASE: i32 = 150;

/// Four chords of a quarter-circle form a faceted floor-to-wall cove.
/// Each chord is an actual sloped collider, rather than a painted bevel.
fn cove(face: usize) -> String {
    let mut out = String::new();
    for i in 0..4 {
        let angle0 = f64::from(i) * std::f64::consts::FRAC_PI_2 / 4.0;
        let angle1 = f64::from(i + 1) * std::f64::consts::FRAC_PI_2 / 4.0;
        let (t0, z0) = (32.0 - 24.0 * angle0.cos(), 32.0 - 24.0 * angle0.sin());
        let (t1, z1) = (32.0 - 24.0 * angle1.cos(), 32.0 - 24.0 * angle1.sin());
        let (a, b) = edge(face);
        let (oa, ob) = offset_inward(a, b, t0);
        let (ia, _) = offset_inward(a, b, t1);
        out.push_str(&band(face, t0, t1, 0.0, z0).replace(
            &flat_plane(z0, true),
            &custom_plane((oa.0, oa.1, z0), (ob.0, ob.1, z0), (ia.0, ia.1, z1), true),
        ));
    }
    out
}

fn room(name: &str, archetype: &str, doors: &[usize]) -> String {
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 0.0, 0.0);
    brushes.push_str(&hex_slab(120.0, LEVEL, 0.0, 0.0));
    let raft: Vec<_> = corners()
        .into_iter()
        .map(|p| (p.0 * 0.80, p.1 * 0.80))
        .collect();
    brushes.push_str(&prism(&raft, 80.0, 88.0, None, 3.0, 3.0));
    let mut lights = String::new();
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, 0.0, 72.0, 12.0, 8.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
            brushes.push_str(&cove(face));
            // Practicals sit above the raft, behind its edge when viewed from
            // the walking floor. Their housing remains authored geometry.
            let (housing, source) = wall_fixture(face, 0.5, 104.0, 20.0);
            brushes.push_str(&housing);
            lights.push_str(&source);
        }
    }
    let mut out = format!("// The Same Door Twice: {name}.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(&format!("authored/{name}"), archetype, BASE, 1, 3)
            .with_register_scope("overlit_grid")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, "solid"));
    for &face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("noon_{face}"),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

/// A thin sloping ceiling, kept distinct from the solid ramp beneath it.
fn shell(plan: &[P2], height: impl Fn(P2) -> f64) -> String {
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
    let height = |x: f64| FLOOR_TOP + (x + 112.0) * LEVEL / 224.0;
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 0.0, 0.0);
    brushes.push_str(&sloped_prism(
        &[
            (-112.0, -36.0),
            (112.0, -36.0),
            (112.0, 36.0),
            (-112.0, 36.0),
        ],
        0.0,
        [
            (-112.0, -36.0, height(-112.0)),
            (-112.0, 36.0, height(-112.0)),
            (112.0, -36.0, height(112.0)),
        ],
        None,
    ));
    for sign in [-1.0, 1.0] {
        for i in 0..4 {
            let a = f64::from(i) * std::f64::consts::FRAC_PI_2 / 4.0;
            let b = f64::from(i + 1) * std::f64::consts::FRAC_PI_2 / 4.0;
            let (y0, z0) = (sign * (36.0 + 24.0 * a.sin()), 24.0 * (1.0 - a.cos()));
            let (y1, z1) = (sign * (36.0 + 24.0 * b.sin()), 24.0 * (1.0 - b.cos()));
            brushes.push_str(&sloped_prism(
                &[(-76.0, y0), (76.0, y0), (76.0, y1), (-76.0, y1)],
                0.0,
                [
                    (-76.0, y0, height(-76.0) + z0),
                    (76.0, y0, height(76.0) + z0),
                    (-76.0, y1, height(-76.0) + z1),
                ],
                None,
            ));
        }
    }
    for face in [1, 2, 4, 5] {
        brushes.push_str(&wall(face, 0.0, 2.0 * LEVEL));
    }
    brushes.push_str(&door_wall(3, 0.0, LEVEL, 0.0, 72.0, 12.0, 8.0));
    brushes.push_str(&door_wall(
        0,
        0.0,
        2.0 * LEVEL,
        LEVEL + FLOOR_TOP,
        LEVEL + 72.0,
        12.0,
        8.0,
    ));
    brushes.push_str(&hex_slab(248.0, 256.0, 0.0, 0.0));
    brushes.push_str(&shell(
        &[(-88.0, -44.0), (88.0, -44.0), (88.0, 44.0), (-88.0, 44.0)],
        |p| height(p.0) + 80.0,
    ));
    let mut lights = String::new();
    for x in [-64.0, 64.0] {
        let z = height(x) + 96.0;
        brushes.push_str(&boxed((x - 12.0, 46.0, z - 4.0), (x + 12.0, 54.0, z + 4.0)));
        lights.push_str(&tile_light(x, 45.0, z));
    }
    let mut out = format!("// The Same Door Twice: enclosed coved ascent.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/noon_ascent", "hall_ramp", BASE, 2, 3)
            .with_register_scope("overlit_grid")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 2, "ramp"));
    out.push_str(&lateral_port(3, "door", "noon_entry", 0, 0, 0));
    out.push_str(&vertical_port("up", "ramp_open", "noon_ascent", 0));
    for index in 0..5u16 {
        let x = -112.0 + f64::from(index) * 56.0;
        out.push_str(&stair_node(index, x, 0.0, height(x)));
    }
    out.push_str(&lights);
    out
}

pub fn straight() -> String {
    room("noon_straight", "hall_straight", &[0, 3])
}
pub fn bend() -> String {
    room("noon_bend", "hall_turn_120", &[0, 2])
}
pub fn fork() -> String {
    room("noon_fork", "hall_junction_3way", &[0, 2, 4])
}
pub fn builders() -> Vec<Builder> {
    vec![
        ("noon_straight", straight),
        ("noon_bend", bend),
        ("noon_fork", fork),
        ("noon_ascent", ascent),
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
    fn noon_sources_reproduce_and_serve_real_demands() {
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
                "{name} cannot be selected"
            );
        }
    }

    #[test]
    fn every_door_pair_and_both_ascent_directions_are_walkable() {
        let config = FpsConfig::default();
        for (name, build) in builders() {
            let tile = crate::parse_authored_module(&build())
                .expect("valid tile")
                .prototype;
            let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
            let threshold = |face| {
                let [a, b] = face_edge(face);
                Vec3::new(
                    f32::from(i16::try_from(a.0 + b.0).expect("local coordinate")) * 0.45,
                    0.5,
                    f32::from(i16::try_from(a.1 + b.1).expect("local coordinate")) * 0.45,
                )
            };
            let doors: Vec<_> = HexFace::LATERAL
                .into_iter()
                .filter(|&f| tile.signature.port(f) == PortClass::Door)
                .collect();
            let paths: Vec<_> = if name == "noon_ascent" {
                vec![
                    (Vec3::new(-6.7, 0.5, 0.0), Vec3::new(6.7, 8.5, 0.0)),
                    (Vec3::new(6.7, 8.5, 0.0), Vec3::new(-6.7, 0.5, 0.0)),
                ]
            } else {
                doors
                    .iter()
                    .flat_map(|&a| {
                        doors
                            .iter()
                            .filter(move |&&b| a != b)
                            .map(move |&b| (threshold(a), threshold(b)))
                    })
                    .collect()
            };
            for (start, goal) in paths {
                let mut body = FpsBody::spawned(start + Vec3::Y * (config.half_height + 0.1), 0.0);
                let mut reached = false;
                for _ in 0..1200 {
                    let feet = body.position - Vec3::Y * config.half_height;
                    assert!(feet.y >= 0.35, "{name}: fell from {start:?} to {goal:?}");
                    if (feet - goal).length() < 0.35 {
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
                assert!(reached, "{name}: blocked at {:?}", body.position);
            }
        }
    }
}
