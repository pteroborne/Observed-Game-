//! The Weight Between: deep bores, undivided piers and solid viewing terraces.

use super::entities::{Meta, ceiling_fixture, deck_node, lateral_port, tile_cell, worldspawn};
use super::geometry::{
    DOOR_TOP, FLOOR_TOP, LEVEL, WALL, band, boxed, corners, door_wall, hex_slab, lerp, prism,
    sloped_prism, wall,
};
use super::{Builder, GENERATED_NOTE};

const BASE: i32 = 190;

#[derive(Clone, Copy)]
enum Mass {
    Gate,
    Bore,
    Pier,
    Fork,
    Recess,
    Overlook,
}

fn block(x0: f64, y0: f64, x1: f64, y1: f64, top: f64) -> String {
    prism(
        &[(x0, y0), (x1, y0), (x1, y1), (x0, y1)],
        FLOOR_TOP,
        top,
        None,
        2.0,
        0.0,
    )
}

fn cell(name: &str, archetype: &str, variant: i32, doors: &[usize], mass: Mass) -> String {
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 2.0, 0.0);
    brushes.push_str(&hex_slab(120.0, LEVEL, 0.0, 2.0));
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, 0.0, DOOR_TOP, 16.0, 6.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
            // Thick backing reads as excavated mass. The canonical envelope
            // remains unchanged, so these sources mate with ordinary halls.
            if !matches!(mass, Mass::Overlook) || ![4, 5].contains(&face) {
                brushes.push_str(&band(face, WALL, 28.0, FLOOR_TOP, 120.0));
            }
        }
    }
    match mass {
        Mass::Gate => {
            brushes.push_str(&block(-20.0, 40.0, 20.0, 108.0, 120.0));
            brushes.push_str(&block(-20.0, -108.0, 20.0, -40.0, 120.0));
            brushes.push_str(&boxed((-20.0, -44.0, 80.0), (20.0, 44.0, 120.0)));
        }
        Mass::Bore => {
            for sign in [-1.0, 1.0] {
                let plan = [
                    (-104.0, 40.0),
                    (104.0, 40.0),
                    (104.0, 60.0),
                    (0.0, 120.0),
                    (-104.0, 60.0),
                ]
                .map(|(x, y)| (x, y * sign));
                brushes.push_str(&prism(&plan, FLOOR_TOP, 120.0, None, 2.0, 0.0));
            }
            brushes.push_str(&boxed((-64.0, -44.0, 80.0), (64.0, 44.0, 120.0)));
        }
        Mass::Pier => brushes.push_str(&block(-24.0, -32.0, 24.0, 32.0, 120.0)),
        Mass::Fork => brushes.push_str(&block(-16.0, -24.0, 16.0, 24.0, 120.0)),
        Mass::Recess => {
            // One heavy wall-backed shoulder, outside the circulation ring.
            brushes.push_str(&band(4, 28.0, 36.0, FLOOR_TOP, 120.0));
        }
        Mass::Overlook => {
            // A solid 5 m ramp rises 2.5 m. Its flat landing is part of the
            // same storey, not an ascent port or a route into a hidden floor.
            brushes.push_str(&sloped_prism(
                &[(-64.0, 48.0), (16.0, 48.0), (16.0, 80.0), (-64.0, 80.0)],
                0.0,
                [
                    (-64.0, 48.0, FLOOR_TOP),
                    (-64.0, 80.0, FLOOR_TOP),
                    (16.0, 48.0, 48.0),
                ],
                None,
            ));
            brushes.push_str(&boxed((16.0, 48.0, 0.0), (64.0, 80.0, 48.0)));
            // Thick parapets finish the exposed front and end of the ledge.
            brushes.push_str(&boxed((16.0, 40.0, 0.0), (64.0, 48.0, 64.0)));
            brushes.push_str(&boxed((64.0, 40.0, 0.0), (72.0, 80.0, 64.0)));
        }
    }

    let fixtures: &[(f64, f64, f64)] = match mass {
        Mass::Gate | Mass::Bore => &[(0.0, 0.0, 80.0)],
        Mass::Overlook => &[(40.0, 64.0, 120.0), (0.0, -48.0, 120.0)],
        _ => &[(-64.0, 0.0, 120.0), (64.0, 0.0, 120.0)],
    };
    let mut lights = String::new();
    for &(x, y, ceiling) in fixtures {
        let (fixture, source) = ceiling_fixture(x, y, ceiling, 6.0, 6.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }

    let mut out = format!("// The Weight Between: {name}.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(&format!("authored/{name}"), archetype, variant, 1, 3)
            .with_register("monolith")
            .with_register_scope("monolith")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, "solid"));
    for &face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("weight_{face}"),
            0,
            0,
            0,
        ));
    }
    if matches!(mass, Mass::Pier | Mass::Fork | Mass::Recess) {
        for index in 0..13u16 {
            let face = usize::from(index / 2) % 6;
            let a = corners()[face];
            let b = corners()[(face + 1) % 6];
            let p = if index % 2 == 0 { a } else { lerp(a, b, 0.5) };
            out.push_str(&deck_node(index, p.0 * 0.62, p.1 * 0.62, FLOOR_TOP));
        }
    }
    out.push_str(&lights);
    out
}

pub fn pier() -> String {
    cell("weight_pier", "hall_straight", BASE, &[0, 3], Mass::Pier)
}
pub fn gate() -> String {
    cell(
        "weight_gate",
        "hall_straight",
        BASE + 1,
        &[0, 3],
        Mass::Gate,
    )
}
pub fn bore() -> String {
    cell(
        "weight_bore",
        "hall_straight",
        BASE + 2,
        &[0, 3],
        Mass::Bore,
    )
}
pub fn overlook() -> String {
    cell(
        "weight_overlook",
        "hall_straight",
        BASE + 3,
        &[0, 3],
        Mass::Overlook,
    )
}
pub fn fork() -> String {
    cell(
        "weight_fork",
        "hall_junction_4way",
        BASE,
        &[0, 2, 3, 4],
        Mass::Fork,
    )
}
pub fn turn() -> String {
    cell("weight_turn", "hall_turn_120", BASE, &[0, 2], Mass::Recess)
}
pub fn elbow() -> String {
    cell("weight_elbow", "hall_turn_60", BASE, &[0, 1], Mass::Recess)
}

pub fn builders() -> Vec<Builder> {
    vec![
        ("weight_pier", pier),
        ("weight_gate", gate),
        ("weight_bore", bore),
        ("weight_overlook", overlook),
        ("weight_fork", fork),
        ("weight_turn", turn),
        ("weight_elbow", elbow),
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
    fn sources_reproduce_and_match_production_monolith_demands() {
        super::super::assert_reproduces(&builders());
        let demands = observed_facility::hex_wfc::geometry_demands();
        for (name, build) in builders() {
            let tile = crate::parse_authored_module(&build())
                .expect("valid source")
                .prototype;
            assert!(
                demands
                    .iter()
                    .any(|d| d.archetype == tile.key.archetype && d.signature == tile.signature),
                "{name} is not selectable"
            );
        }
    }

    fn walk(tile: &crate::TilePrototype, body: &mut FpsBody, goal: Vec3, deck: bool) {
        let config = FpsConfig::default();
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        for _ in 0..2400 {
            let feet = body.position - Vec3::Y * config.half_height;
            assert!(feet.y >= 0.35, "fell on {:?}", tile.key);
            if (feet - goal).length() < 0.25 {
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
                body,
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
            "{:?}: blocked at {:?}, goal {goal:?}",
            tile.key, body.position
        );
    }

    fn spawn(feet: Vec3) -> FpsBody {
        FpsBody::spawned(
            feet + Vec3::Y * (FpsConfig::default().half_height + 0.1),
            0.0,
        )
    }

    #[test]
    fn all_door_pairs_route_through_bores_and_around_solid_piers() {
        for (_, build) in builders() {
            let tile = crate::parse_authored_module(&build())
                .expect("valid source")
                .prototype;
            let doors: Vec<_> = HexFace::LATERAL
                .into_iter()
                .filter(|&f| tile.signature.port(f) == PortClass::Door)
                .collect();
            let threshold = |face| {
                let [a, b] = face_edge(face);
                Vec3::new(
                    f32::from(i16::try_from(a.0 + b.0).expect("local")) * 0.45,
                    0.5,
                    f32::from(i16::try_from(a.1 + b.1).expect("local")) * 0.45,
                )
            };
            for &a in &doors {
                for &b in &doors {
                    if a != b {
                        walk(&tile, &mut spawn(threshold(a)), threshold(b), true);
                    }
                }
            }
        }
    }

    #[test]
    fn overlook_is_reachable_and_returns_to_the_main_floor_without_jumping() {
        let tile = crate::parse_authored_module(&overlook())
            .expect("valid source")
            .prototype;
        let path = [
            Vec3::new(-4.5, 0.5, 0.0),
            Vec3::new(-4.5, 0.5, -4.0),
            Vec3::new(1.5, 3.0, -4.0),
            Vec3::new(3.0, 3.0, -4.0),
        ];
        let mut body = spawn(path[0]);
        for &goal in &path[1..] {
            walk(&tile, &mut body, goal, false);
        }
        assert!(body.position.y - FpsConfig::default().half_height > 2.85);
        for &goal in path[..path.len() - 1].iter().rev() {
            walk(&tile, &mut body, goal, false);
        }
    }
}
