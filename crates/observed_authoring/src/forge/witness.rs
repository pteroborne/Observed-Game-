//! The Witness Exchange: open galleries, an exposed bridge, and a rescue reveal.
//!
//! These are ordinary Wellshaft WFC candidates, not a new landmark archetype.
//! The photographed arrangement is authored separately in docs/compositions.
//! Low sealed parapets stop a body while allowing observation across the well;
//! all door sills retain the production floor height and aperture.

use super::entities::{
    Meta, deck_node, lateral_port, stair_node, tile_cell, tile_light, vertical_port, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, P2, WALL, boxed, corners, door_wall, edge, hex_slab, lerp, offset_inward,
    prism, sloped_prism, wall,
};
use super::{Builder, GENERATED_NOTE};

const BASE: i32 = 110;
const INNER: f64 = 0.58;

fn scaled(point: P2, scale: f64) -> P2 {
    (point.0 * scale, point.1 * scale)
}

/// Full-height jambs carry the next gallery; the lintel stops short of the
/// crown so observers can see the tier beyond. The aperture remains 72 x 64.
fn open_crown_threshold(face: usize) -> String {
    let (a, b) = edge(face);
    let (ia, ib) = offset_inward(a, b, WALL);
    let length = (b.0 - a.0).hypot(b.1 - a.1);
    let left = lerp(a, b, 0.5 - 36.0 / length);
    let right = lerp(a, b, 0.5 + 36.0 / length);
    let (il, ir) = offset_inward(left, right, WALL);
    let mut out = prism(&[a, left, il, ia], 0.0, LEVEL, None, 0.0, 0.0);
    out.push_str(&prism(&[right, b, ib, ir], 0.0, LEVEL, None, 0.0, 0.0));
    out.push_str(&prism(&[left, right, ir, il], 72.0, 88.0, None, 0.0, 2.0));
    out
}

/// A six-piece deck leaves a real shaft; the central bridge is one continuous
/// collider with flush landings, not a cosmetic strip over a hidden floor.
fn deck(bridge: bool) -> String {
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
        // Break the inner rail where the crossing actually meets the ring.
        let spans: &[(f64, f64)] = if bridge && (face == 0 || face == 3) {
            &[(0.0, 0.25), (0.75, 1.0)]
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
                    scaled(b, INNER + 0.04),
                    scaled(a, INNER + 0.04),
                ],
                FLOOR_TOP,
                22.0,
                None,
                1.0,
                0.0,
            ));
        }
    }
    if bridge {
        out.push_str(&prism(
            &[(-70.0, -18.0), (70.0, -18.0), (70.0, 18.0), (-70.0, 18.0)],
            0.0,
            FLOOR_TOP,
            None,
            0.0,
            2.0,
        ));
    }
    out
}

/// Paired fluted piers stay clear of every canonical door and the ring path.
/// The horizontal light housings match the shared renderer's practical mesh.
fn piers(top: f64) -> (String, String) {
    let mut brushes = String::new();
    let mut lights = String::new();
    for y in [-100.0, 100.0] {
        for x in [-22.0, 22.0] {
            brushes.push_str(&prism(
                &[
                    (x - 5.0, y - 7.0),
                    (x + 5.0, y - 7.0),
                    (x + 5.0, y + 7.0),
                    (x - 5.0, y + 7.0),
                ],
                FLOOR_TOP,
                top,
                None,
                2.0,
                0.0,
            ));
        }
        let back = y.signum() * 104.0;
        brushes.push_str(&boxed(
            (-17.0, back - 3.0, FLOOR_TOP),
            (17.0, back + 3.0, top - 5.0),
        ));
        let inside = y.signum() * (y.abs() - 9.0);
        brushes.push_str(&boxed(
            (-24.0, inside - 5.0, top - 29.0),
            (24.0, inside + 5.0, top - 25.0),
        ));
        lights.push_str(&tile_light(0.0, inside, top - 31.0));
    }
    (brushes, lights)
}

#[derive(Clone, Copy)]
enum FloorPlan {
    Gallery,
    Bridge,
    Terrace,
    Gateway,
}

fn gallery(name: &str, archetype: &str, variant: i32, doors: &[usize], floor: FloorPlan) -> String {
    let bridge = matches!(floor, FloorPlan::Bridge);
    let gate = matches!(floor, FloorPlan::Gateway);
    let solid = matches!(floor, FloorPlan::Terrace | FloorPlan::Gateway);
    let mut brushes = if solid {
        hex_slab(0.0, FLOOR_TOP, 0.0, 0.0)
    } else {
        deck(bridge)
    };
    for face in 0..6 {
        if doors.contains(&face) {
            // No extra sill brush: the deck already supplies the exact sill.
            brushes.push_str(&open_crown_threshold(face));
        } else {
            brushes.push_str(&wall(face, FLOOR_TOP, 36.0));
        }
    }
    let (supports, mut lights) = piers(LEVEL);
    brushes.push_str(&supports);
    if gate {
        // A nested reveal in the east approach: progressively taller toward
        // the observer. Every frame leaves at least the canonical 4.5 m lane.
        for (x, half, top) in [(92.0, 39.0, 80.0), (76.0, 44.0, 98.0), (60.0, 49.0, 116.0)] {
            for sign in [-1.0, 1.0] {
                let y = sign * half;
                brushes.push_str(&boxed(
                    (x - 4.0, y - 4.0, FLOOR_TOP),
                    (x + 4.0, y + 4.0, top),
                ));
            }
            brushes.push_str(&prism(
                &[
                    (x - 4.0, -half - 4.0),
                    (x + 4.0, -half - 4.0),
                    (x + 4.0, half + 4.0),
                    (x - 4.0, half + 4.0),
                ],
                top - 8.0,
                top,
                None,
                2.0,
                2.0,
            ));
        }
        brushes.push_str(&boxed((40.0, -20.0, 107.0), (52.0, 20.0, 111.0)));
        lights.push_str(&tile_light(46.0, 0.0, 105.0));
        // Light each successive reveal from its own header. The existing
        // district practical treatment supplies all color and intensity.
        lights.push_str(&tile_light(76.0, 0.0, 88.0));
        lights.push_str(&tile_light(92.0, 0.0, 70.0));
    }
    let mut out = format!(
        "// Witness Exchange: {name}. Structural geometry only; rescue is a gameplay affordance.\n{GENERATED_NOTE}"
    );
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(&format!("authored/{name}"), archetype, variant, 1, 3)
            .with_register_scope("wellshaft")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, if solid { "solid" } else { "open" }));
    for &face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("witness_{face}"),
            0,
            0,
            0,
        ));
    }
    if !solid && !bridge {
        // Route actors around the real hole. Corner nodes prevent a chord
        // between face midpoints from cutting through the inner parapet.
        for index in 0..13u16 {
            let face = usize::from(index / 2) % 6;
            let a = corners()[face];
            let b = corners()[(face + 1) % 6];
            let p = if index % 2 == 0 { a } else { lerp(a, b, 0.5) };
            let p = scaled(p, 0.73);
            out.push_str(&deck_node(index, p.0, p.1, FLOOR_TOP));
        }
    }
    out.push_str(&lights);
    out
}

pub fn bridge() -> String {
    gallery(
        "witness_bridge",
        "hall_straight",
        BASE,
        &[0, 3],
        FloorPlan::Bridge,
    )
}

pub fn balcony() -> String {
    gallery(
        "witness_balcony",
        "hall_turn_120",
        BASE,
        &[0, 4],
        FloorPlan::Gallery,
    )
}

pub fn junction() -> String {
    gallery(
        "witness_junction",
        "hall_junction_3way",
        BASE,
        &[0, 1, 2],
        FloorPlan::Gallery,
    )
}

pub fn crossroads() -> String {
    gallery(
        "witness_crossroads",
        "hall_junction_4way",
        BASE,
        &[0, 2, 3, 4],
        FloorPlan::Gallery,
    )
}

pub fn return_fan() -> String {
    gallery(
        "witness_return_fan",
        "hall_junction_4way",
        BASE + 1,
        &[0, 1, 2, 3],
        FloorPlan::Gallery,
    )
}

pub fn return_pairs() -> String {
    gallery(
        "witness_return_pairs",
        "hall_junction_4way",
        BASE + 2,
        &[0, 1, 4, 5],
        FloorPlan::Gallery,
    )
}

pub fn elbow() -> String {
    gallery(
        "witness_elbow",
        "hall_turn_60",
        BASE,
        &[0, 5],
        FloorPlan::Gallery,
    )
}

pub fn gate() -> String {
    gallery(
        "witness_gate",
        "hall_turn_120",
        BASE + 1,
        &[0, 2],
        FloorPlan::Gateway,
    )
}

pub fn straight() -> String {
    gallery(
        "witness_straight",
        "hall_straight",
        BASE + 1,
        &[0, 3],
        FloorPlan::Gallery,
    )
}

pub fn terrace_corner() -> String {
    gallery(
        "witness_terrace_corner",
        "hall_turn_120",
        BASE + 2,
        &[0, 4],
        FloorPlan::Terrace,
    )
}

pub fn terrace_junction() -> String {
    gallery(
        "witness_terrace_junction",
        "hall_junction_3way",
        BASE + 1,
        &[0, 1, 2],
        FloorPlan::Terrace,
    )
}

pub fn terrace_crossroads() -> String {
    gallery(
        "witness_terrace_crossroads",
        "hall_junction_4way",
        BASE + 3,
        &[0, 2, 3, 4],
        FloorPlan::Terrace,
    )
}

pub fn terrace_fan() -> String {
    gallery(
        "witness_terrace_fan",
        "hall_junction_4way",
        BASE + 4,
        &[0, 1, 2, 3],
        FloorPlan::Terrace,
    )
}

pub fn terrace_pairs() -> String {
    gallery(
        "witness_terrace_pairs",
        "hall_junction_4way",
        BASE + 5,
        &[0, 1, 4, 5],
        FloorPlan::Terrace,
    )
}

/// A roofless ascent beside a void, with the same straight spine and port
/// convention as the production ramp. Higher-floor content may expose edges;
/// it must still carry the actor across both seams without a hidden step.
pub fn ascent() -> String {
    let height = |x: f64| FLOOR_TOP + (x + 112.0) * LEVEL / 224.0;
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 0.0, 2.0);
    let walk = [
        (-112.0, -36.0),
        (112.0, -36.0),
        (112.0, 36.0),
        (-112.0, 36.0),
    ];
    brushes.push_str(&sloped_prism(
        &walk,
        0.0,
        [
            (-112.0, -36.0, height(-112.0)),
            (-112.0, 36.0, height(-112.0)),
            (112.0, -36.0, height(112.0)),
        ],
        None,
    ));
    // A parapet follows the climb. The sealed edge remains above the body,
    // while the roofless ramp can be seen from the observation galleries.
    for face in [1, 2, 4, 5] {
        let (a, b) = edge(face);
        let (ia, ib) = offset_inward(a, b, WALL);
        brushes.push_str(&sloped_prism(
            &[a, b, ib, ia],
            0.0,
            [
                (-112.0, -64.0, height(-112.0) + 36.0),
                (-112.0, 64.0, height(-112.0) + 36.0),
                (112.0, -64.0, height(112.0) + 36.0),
            ],
            None,
        ));
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
    let (supports, lights) = piers(2.0 * LEVEL);
    brushes.push_str(&supports);
    let mut out =
        format!("// Witness Exchange ascent: exposed central climb, two levels.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/witness_ascent", "hall_ramp", BASE, 2, 3)
            .with_register_scope("wellshaft")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 2, "ramp"));
    out.push_str(&lateral_port(3, "door", "witness_entry", 0, 0, 0));
    out.push_str(&vertical_port("up", "ramp_open", "witness_ascent", 0));
    for index in 0..5u16 {
        let x = -112.0 + f64::from(index) * 56.0;
        out.push_str(&stair_node(index, x, 0.0, height(x)));
    }
    out.push_str(&lights);
    out
}

pub fn builders() -> Vec<Builder> {
    vec![
        ("witness_bridge", bridge),
        ("witness_balcony", balcony),
        ("witness_junction", junction),
        ("witness_crossroads", crossroads),
        ("witness_return_fan", return_fan),
        ("witness_return_pairs", return_pairs),
        ("witness_elbow", elbow),
        ("witness_gate", gate),
        ("witness_straight", straight),
        ("witness_ascent", ascent),
        ("witness_terrace_corner", terrace_corner),
        ("witness_terrace_junction", terrace_junction),
        ("witness_terrace_crossroads", terrace_crossroads),
        ("witness_terrace_fan", terrace_fan),
        ("witness_terrace_pairs", terrace_pairs),
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

    fn threshold(face: HexFace) -> Vec3 {
        let [a, b] = face_edge(face);
        #[allow(clippy::cast_precision_loss)]
        Vec3::new((a.0 + b.0) as f32 * 0.45, 0.5, (a.1 + b.1) as f32 * 0.45)
    }

    #[test]
    fn every_gallery_door_pair_is_walkable_without_falling_into_the_well() {
        let config = FpsConfig::default();
        for (name, build) in builders()
            .into_iter()
            .filter(|(name, _)| *name != "witness_ascent")
        {
            let tile = crate::parse_authored_module(&build())
                .expect("valid tile")
                .prototype;
            let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
            let doors: Vec<_> = HexFace::LATERAL
                .into_iter()
                .filter(|&face| tile.signature.port(face) == PortClass::Door)
                .collect();
            for &entry in &doors {
                for &exit in &doors {
                    if entry == exit {
                        continue;
                    }
                    let goal = threshold(exit);
                    let mut body =
                        FpsBody::spawned(threshold(entry) + Vec3::Y * config.half_height, 0.0);
                    let mut reached = false;
                    for _ in 0..2400 {
                        let feet = body.position - Vec3::Y * config.half_height;
                        assert!(
                            feet.y >= 0.35,
                            "{name}: fell on {entry:?} -> {exit:?}: {feet:?}"
                        );
                        if (feet - goal).length() < 0.3 {
                            reached = true;
                            break;
                        }
                        let target = tile.deck.step_toward(feet, goal).unwrap_or(goal);
                        let delta = target - feet;
                        body.yaw = delta.x.atan2(-delta.z);
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
                        reached,
                        "{name}: blocked {entry:?} -> {exit:?} at {:?}",
                        body.position
                    );
                }
            }
        }
    }

    #[test]
    fn ascent_gains_one_storey_without_jumping() {
        let tile = crate::parse_authored_module(&ascent())
            .expect("valid ramp")
            .prototype;
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        let config = FpsConfig::default();
        let mut body = FpsBody::spawned(
            Vec3::new(-6.8, 0.7 + config.half_height, 0.0),
            std::f32::consts::FRAC_PI_2,
        );
        let mut highest = 0.0f32;
        for _ in 0..300 {
            let report = step_character(
                &scene,
                &mut body,
                PlayerIntent {
                    movement: Vec2::Y,
                    ..PlayerIntent::default()
                },
                &config,
                1.0 / 60.0,
            );
            assert!(!report.jumped);
            highest = highest.max(body.position.y - config.half_height);
        }
        assert!(highest > 8.35, "climb stopped at {highest}");
    }

    #[test]
    fn witness_sources_reproduce_the_committed_maps() {
        super::super::assert_reproduces(&builders());
    }

    #[test]
    fn every_witness_module_validates_and_serves_a_real_wfc_demand() {
        let demands = observed_facility::hex_wfc::geometry_demands();
        for (name, build) in builders() {
            let module = crate::parse_authored_module(&build())
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert!(
                demands
                    .iter()
                    .any(|demand| demand.archetype == module.prototype.key.archetype
                        && demand.signature == module.prototype.signature),
                "{name} is unreachable by WFC"
            );
        }
    }
}
