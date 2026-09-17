//! The Borrowed View: staggered horizontal screens and two looping galleries.

use super::entities::{Meta, ceiling_fixture, deck_node, lateral_port, tile_cell, worldspawn};
use super::geometry::{
    DOOR_TOP, FLOOR_TOP, LEVEL, boxed, corners, door_wall, hex_slab, lerp, wall,
};
use super::{Builder, GENERATED_NOTE};

const BASE: i32 = 180;

#[derive(Clone, Copy)]
enum Screens {
    Pair,
    Single,
    Threshold,
}

fn screen(x: f64, left: f64, right: f64) -> String {
    let mut out = String::new();
    // Six separate horizontal members, with 0.375 m of real air between them.
    // The pale end panels carry both the slats and their dark head beam.
    for z in [12.0, 22.0, 32.0, 42.0, 52.0, 62.0] {
        out.push_str(&boxed((x - 3.0, left, z), (x + 3.0, right, z + 4.0)));
    }
    for y in [left, right] {
        out.push_str(&boxed(
            (x - 4.0, y - 6.0, FLOOR_TOP),
            (x + 4.0, y + 6.0, 70.0),
        ));
    }
    out.push_str(&boxed(
        (x - 5.0, left - 10.0, 66.0),
        (x + 5.0, right + 10.0, 74.0),
    ));
    out
}

fn cell(name: &str, archetype: &str, variant: i32, doors: &[usize], screens: Screens) -> String {
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 2.0, 0.0);
    brushes.push_str(&hex_slab(120.0, LEVEL, 0.0, 2.0));
    // The floor slab already spans every threshold; no duplicate sill brush.
    for face in 0..6 {
        brushes.push_str(&if doors.contains(&face) {
            door_wall(face, 0.0, LEVEL, 0.0, DOOR_TOP, 10.0, 0.0)
        } else {
            wall(face, 0.0, LEVEL)
        });
    }
    match screens {
        Screens::Pair => {
            brushes.push_str(&screen(-32.0, -64.0, 16.0));
            brushes.push_str(&screen(32.0, -16.0, 64.0));
        }
        Screens::Single => brushes.push_str(&screen(-32.0, -56.0, 16.0)),
        Screens::Threshold => {
            // Three heavy horizontals span the walkable axis overhead.
            // Their ends are carried by the sealed side walls.
            for x in [-64.0_f64, 0.0, 64.0] {
                let extent = 128.0 - x.abs() * 4.0 / 7.0;
                brushes.push_str(&boxed(
                    (x - 4.0, -extent + 8.0, 88.0),
                    (x + 4.0, extent - 8.0, 96.0),
                ));
            }
        }
    }
    let mut lights = String::new();
    // The fixtures hang from the outer lid. The paper's treatment is shared
    // style; sources carry placement and purpose, never ad-hoc energy/RGB.
    for y in [-80.0, 80.0] {
        let (fixture, source) = ceiling_fixture(0.0, y, 120.0, 16.0, 5.0);
        brushes.push_str(&fixture);
        lights.push_str(&source);
    }
    let mut out = format!("// The Borrowed View: {name}.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(&format!("authored/{name}"), archetype, variant, 1, 3)
            .with_register("shadow_screen")
            .with_register_scope("shadow_screen")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 1, "solid"));
    for &face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("borrowed_{face}"),
            0,
            0,
            0,
        ));
    }
    // The outer loop remains available around every screen. Its nodes are
    // behind the end panels and inside the threshold jambs.
    for index in 0..13u16 {
        let face = usize::from(index / 2) % 6;
        let a = corners()[face];
        let b = corners()[(face + 1) % 6];
        let p = if index % 2 == 0 { a } else { lerp(a, b, 0.5) };
        out.push_str(&deck_node(index, p.0 * 0.73, p.1 * 0.73, FLOOR_TOP));
    }
    out.push_str(&lights);
    out
}

pub fn layered() -> String {
    cell(
        "borrowed_screen",
        "hall_straight",
        BASE,
        &[0, 3],
        Screens::Pair,
    )
}
pub fn fork() -> String {
    cell(
        "borrowed_fork",
        "hall_junction_4way",
        BASE,
        &[0, 2, 3, 4],
        Screens::Pair,
    )
}
pub fn turn() -> String {
    cell(
        "borrowed_turn",
        "hall_turn_120",
        BASE,
        &[0, 2],
        Screens::Single,
    )
}
pub fn elbow() -> String {
    cell(
        "borrowed_elbow",
        "hall_turn_60",
        BASE,
        &[0, 1],
        Screens::Single,
    )
}
pub fn gallery() -> String {
    cell(
        "borrowed_gallery",
        "hall_straight",
        BASE + 1,
        &[0, 3],
        Screens::Single,
    )
}
pub fn threshold() -> String {
    cell(
        "borrowed_threshold",
        "hall_straight",
        BASE + 2,
        &[0, 3],
        Screens::Threshold,
    )
}

pub fn builders() -> Vec<Builder> {
    vec![
        ("borrowed_screen", layered),
        ("borrowed_fork", fork),
        ("borrowed_turn", turn),
        ("borrowed_elbow", elbow),
        ("borrowed_gallery", gallery),
        ("borrowed_threshold", threshold),
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
    fn sources_reproduce_and_match_real_shadow_screen_demands() {
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

    fn walk(tile: &crate::TilePrototype, start: Vec3, goal: Vec3, deck: bool) {
        let config = FpsConfig::default();
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        let mut body = FpsBody::spawned(start + Vec3::Y * (config.half_height + 0.1), 0.0);
        for _ in 0..2400 {
            let feet = body.position - Vec3::Y * config.half_height;
            assert!(
                feet.y >= 0.35,
                "fell on {:?}: {start:?} -> {goal:?}",
                tile.key
            );
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
            "{:?}: blocked at {:?}, {start:?} -> {goal:?}",
            tile.key, body.position
        );
    }

    #[test]
    fn every_door_pair_routes_around_the_staggered_screens() {
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
                        walk(&tile, threshold(a), threshold(b), true);
                    }
                }
            }
        }
    }

    #[test]
    fn screen_gaps_are_real_but_do_not_admit_a_player() {
        let tile = crate::parse_authored_module(&layered())
            .expect("valid source")
            .prototype;
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        let gap = Vec3::new(-2.0, 1.8, 1.5);
        assert!(
            scene.capsule_is_clear(gap, 0.03, 0.03),
            "the screen gap is filled"
        );
        assert!(
            !scene.capsule_is_clear(Vec3::new(-2.0, 1.5, 1.5), 0.03, 0.03),
            "the rail is missing"
        );
        let config = FpsConfig::default();
        assert!(
            !scene.capsule_is_clear(gap, config.radius, config.half_height),
            "the screen admits a player"
        );
    }
}
