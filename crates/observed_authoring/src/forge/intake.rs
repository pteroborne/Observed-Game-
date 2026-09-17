//! The Uncalled Number: waiting rooms, service hatches and a staff bypass.

use super::entities::{Meta, ceiling_fixture, deck_node, lateral_port, tile_cell, worldspawn};
use super::geometry::{
    DOOR_TOP, FLOOR_TOP, LEVEL, band, boxed, corners, door_wall, hex_slab, lerp, prism, translate,
    wall,
};
use super::{Builder, GENERATED_NOTE};

const BASE: i32 = 200;
const CEILING: f64 = 80.0;

#[derive(Clone, Copy)]
enum Program {
    Arrival,
    Waiting,
    Service,
    Queue,
    Hatch,
    Corner,
}

fn bench(face: usize) -> String {
    let mut out = band(face, 8.0, 26.0, FLOOR_TOP, 12.0);
    out.push_str(&band(face, 8.0, 30.0, 12.0, 16.0));
    out.push_str(&band(face, 8.0, 12.0, 16.0, 28.0));
    out
}

fn hatch() -> String {
    let mut out = boxed((-8.0, -40.0, FLOOR_TOP), (8.0, 40.0, 24.0));
    for y in [-44.0, 44.0] {
        out.push_str(&boxed((-8.0, y - 4.0, FLOOR_TOP), (8.0, y + 4.0, 64.0)));
    }
    out.push_str(&boxed((-8.0, -48.0, 40.0), (8.0, 48.0, 64.0)));
    out.push_str(&boxed((-20.0, -40.0, 24.0), (20.0, 40.0, 28.0)));
    out
}

fn cell(name: &str, archetype: &str, variant: i32, doors: &[usize], program: Program) -> String {
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 2.0, 0.0);
    brushes.push_str(&hex_slab(120.0, LEVEL, 0.0, 2.0));
    // A lower continuous ceiling is carried by the envelope. Its 5 m soffit
    // preserves the canonical 4 m standing clearance at every threshold.
    brushes.push_str(&hex_slab(CEILING, CEILING + 8.0, 0.0, 1.0));
    for face in 0..6 {
        if doors.contains(&face) {
            brushes.push_str(&door_wall(face, 0.0, LEVEL, 0.0, DOOR_TOP, 8.0, 2.0));
        } else {
            brushes.push_str(&wall(face, 0.0, LEVEL));
            brushes.push_str(&band(face, 8.0, 10.0, FLOOR_TOP, 32.0));
        }
    }
    match program {
        Program::Arrival => {
            for y in [-36.0, 36.0] {
                brushes.push_str(&boxed((-56.0, y - 3.0, FLOOR_TOP), (56.0, y + 3.0, 28.0)));
            }
        }
        Program::Waiting => {
            brushes.push_str(&bench(1));
            brushes.push_str(&bench(4));
        }
        Program::Service => {
            // Built-in storage follows the sealed rear walls; the five thin
            // face divisions belong to the cabinet mass, not loose props.
            brushes.push_str(&prism(
                &[
                    (-72.0, 48.0),
                    (72.0, 48.0),
                    (72.0, 80.0),
                    (0.0, 120.0),
                    (-72.0, 80.0),
                ],
                FLOOR_TOP,
                48.0,
                None,
                2.0,
                0.0,
            ));
            for x in [-48.0, -24.0, 0.0, 24.0, 48.0] {
                brushes.push_str(&boxed((x - 1.0, 45.0, FLOOR_TOP), (x + 1.0, 49.0, 48.0)));
            }
            brushes.push_str(&boxed((-40.0, -56.0, FLOOR_TOP), (40.0, -48.0, 52.0)));
            brushes.push_str(&boxed((-40.0, -56.0, FLOOR_TOP), (-32.0, -16.0, 52.0)));
        }
        Program::Queue => {
            for (x, a, b) in [(-32.0, -44.0, 20.0), (32.0, -20.0, 44.0)] {
                brushes.push_str(&boxed((x - 4.0, a, FLOOR_TOP), (x + 4.0, b, 30.0)));
                brushes.push_str(&boxed((x - 5.0, a, 30.0), (x + 5.0, b, 32.0)));
            }
        }
        // Keep the canonical centre spawn clear of the counter. A hatch is
        // a viewing opening, not the cell's standing-clearance sample.
        Program::Hatch => brushes.push_str(&translate(&hatch(), 32.0, 0.0, 0.0)),
        Program::Corner => brushes.push_str(&bench(4)),
    }
    let mut lights = String::new();
    for y in [-64.0, 64.0] {
        let (fixture, source) = ceiling_fixture(0.0, y, CEILING, 28.0, 4.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let mut out = format!("// The Uncalled Number: {name}.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(&format!("authored/{name}"), archetype, variant, 1, 3)
            .with_register("institutional")
            .with_register_scope("institutional")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, "solid"));
    for &face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("intake_{face}"),
            0,
            0,
            0,
        ));
    }
    if matches!(program, Program::Queue | Program::Hatch | Program::Corner) {
        // The ring stays outside the ends of the counters and queue islands.
        // Main corridors need no detour; their centre line remains clear.
        for index in 0..13u16 {
            let face = usize::from(index / 2) % 6;
            let a = corners()[face];
            let b = corners()[(face + 1) % 6];
            let p = if index % 2 == 0 { a } else { lerp(a, b, 0.5) };
            out.push_str(&deck_node(index, p.0 * 0.67, p.1 * 0.67, FLOOR_TOP));
        }
    }
    out.push_str(&lights);
    out
}

pub fn arrival() -> String {
    cell(
        "intake_arrival",
        "hall_straight",
        BASE,
        &[0, 3],
        Program::Arrival,
    )
}
pub fn waiting() -> String {
    cell(
        "intake_waiting",
        "hall_straight",
        BASE + 1,
        &[0, 3],
        Program::Waiting,
    )
}
pub fn service() -> String {
    cell(
        "intake_service",
        "hall_straight",
        BASE + 2,
        &[0, 3],
        Program::Service,
    )
}
pub fn junction() -> String {
    cell(
        "intake_junction",
        "hall_junction_3way",
        BASE,
        &[0, 1, 3],
        Program::Queue,
    )
}
pub fn concourse() -> String {
    cell(
        "intake_concourse",
        "hall_junction_4way",
        BASE,
        &[0, 1, 3, 5],
        Program::Queue,
    )
}
pub fn reception() -> String {
    cell(
        "intake_reception",
        "hall_turn_120",
        BASE,
        &[0, 2],
        Program::Hatch,
    )
}
pub fn turn() -> String {
    cell(
        "intake_turn",
        "hall_turn_120",
        BASE + 1,
        &[0, 2],
        Program::Corner,
    )
}
pub fn elbow() -> String {
    cell(
        "intake_elbow",
        "hall_turn_60",
        BASE,
        &[0, 1],
        Program::Corner,
    )
}

pub fn builders() -> Vec<Builder> {
    vec![
        ("intake_arrival", arrival),
        ("intake_waiting", waiting),
        ("intake_service", service),
        ("intake_junction", junction),
        ("intake_concourse", concourse),
        ("intake_reception", reception),
        ("intake_turn", turn),
        ("intake_elbow", elbow),
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
    fn sources_reproduce_and_match_production_institutional_demands() {
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

    fn walk(tile: &crate::TilePrototype, start: Vec3, goal: Vec3) {
        let config = FpsConfig::default();
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        let mut body = FpsBody::spawned(start + Vec3::Y * (config.half_height + 0.1), 0.0);
        for _ in 0..2400 {
            let feet = body.position - Vec3::Y * config.half_height;
            assert!(feet.y >= 0.35, "fell on {:?}", tile.key);
            if (feet - goal).length() < 0.25 {
                return;
            }
            let delta = tile.deck.step_toward(feet, goal).unwrap_or(goal) - feet;
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
            "{:?}: blocked at {:?}, {start:?} -> {goal:?}",
            tile.key, body.position
        );
    }

    #[test]
    fn all_door_pairs_route_around_the_counters_and_queue_islands() {
        let threshold = |face| {
            let [a, b] = face_edge(face);
            Vec3::new(
                f32::from(i16::try_from(a.0 + b.0).expect("local")) * 0.45,
                0.5,
                f32::from(i16::try_from(a.1 + b.1).expect("local")) * 0.45,
            )
        };
        for (_, build) in builders() {
            let tile = crate::parse_authored_module(&build())
                .expect("valid source")
                .prototype;
            let doors: Vec<_> = HexFace::LATERAL
                .into_iter()
                .filter(|&f| tile.signature.port(f) == PortClass::Door)
                .collect();
            for &a in &doors {
                for &b in &doors {
                    if a != b {
                        walk(&tile, threshold(a), threshold(b));
                    }
                }
            }
        }
    }

    #[test]
    fn service_hatch_has_real_air_at_eye_height_but_rejects_a_player() {
        let tile = crate::parse_authored_module(&reception())
            .expect("valid source")
            .prototype;
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        // Sweep a tiny capsule through the opening; there is no hidden pane.
        for step in -16i16..=16 {
            assert!(scene.capsule_is_clear(
                Vec3::new(2.0 + f32::from(step) / 8.0, 2.125, 0.0),
                0.025,
                0.025
            ));
        }
        assert!(
            !scene.capsule_is_clear(Vec3::new(2.0, 2.8, 0.0), 0.025, 0.025),
            "missing header"
        );
        let config = FpsConfig::default();
        assert!(!scene.capsule_is_clear(
            Vec3::new(2.0, 2.125, 0.0),
            config.radius,
            config.half_height
        ));
    }
}
