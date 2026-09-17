//! The Third Light: a continuous low-ceiling field for the Liminal Grid.
//! Six candidates retain ordinary WFC ports beneath a suspended panel grid.

use super::entities::{Meta, ceiling_fixture, deck_node, lateral_port, tile_cell, worldspawn};
use super::geometry::{FLOOR_TOP, P2, band, boxed, corners, hex_slab, lerp, prism, wall};
use super::{Builder, GENERATED_NOTE};

const BASE: i32 = 170;
const CEILING: f64 = 56.0;

#[derive(Clone, Copy)]
enum Form {
    Grid,
    Baffle,
    Cut,
    Recess,
}

fn opened_floor() -> String {
    // Four convex pieces leave an exact 5 m square hole; a separate 1.5 m
    // strip crosses it. There is no hidden slab below the maintenance opening.
    let mut out = String::new();
    for sign in [-1.0, 1.0] {
        out.push_str(&prism(
            &[
                (sign * 40.0, -105.142857),
                (sign * 112.0, -64.0),
                (sign * 112.0, 64.0),
                (sign * 40.0, 105.142857),
            ],
            0.0,
            FLOOR_TOP,
            None,
            0.0,
            0.0,
        ));
        out.push_str(&prism(
            &[
                (-40.0, sign * 40.0),
                (40.0, sign * 40.0),
                (40.0, sign * 105.142857),
                (0.0, sign * 128.0),
                (-40.0, sign * 105.142857),
            ],
            0.0,
            FLOOR_TOP,
            None,
            0.0,
            0.0,
        ));
    }
    out.push_str(&boxed((-40.0, -12.0, 0.0), (40.0, 12.0, FLOOR_TOP)));
    out
}

/// Clip a convex ceiling panel to a rectangular grid cell inside the hex.
fn clip_panel(plan: &[P2], normal: P2, offset: f64) -> Vec<P2> {
    let mut out = Vec::new();
    for i in 0..plan.len() {
        let a = plan[i];
        let b = plan[(i + 1) % plan.len()];
        let da = a.0 * normal.0 + a.1 * normal.1 - offset;
        let db = b.0 * normal.0 + b.1 * normal.1 - offset;
        if da <= 0.0 {
            out.push(a);
        }
        if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
            out.push(lerp(a, b, da / (da - db)));
        }
    }
    out
}

fn cell(name: &str, archetype: &str, variant: i32, doors: &[usize], form: Form) -> String {
    let mut brushes = if matches!(form, Form::Cut) {
        opened_floor()
    } else {
        hex_slab(0.0, FLOOR_TOP, 0.0, 0.0)
    };
    // The threshold retains the shared 4 m clearance. Inside it, a suspended
    // ceiling drops to 3 m above the floor, with rails another 0.25 m lower.
    brushes.push_str(&hex_slab(120.0, 128.0, 0.0, 0.0));
    let ceiling: Vec<_> = corners()
        .into_iter()
        .map(|p| (p.0 * 0.80, p.1 * 0.80))
        .collect();
    for (left, right) in [(-104.0, -33.0), (-31.0, 31.0), (33.0, 104.0)] {
        for (bottom, top) in [(-104.0, -33.0), (-31.0, 31.0), (33.0, 104.0)] {
            let mut panel = ceiling.clone();
            for (normal, offset) in [
                ((1.0, 0.0), right),
                ((-1.0, 0.0), -left),
                ((0.0, 1.0), top),
                ((0.0, -1.0), -bottom),
            ] {
                panel = clip_panel(&panel, normal, offset);
            }
            brushes.push_str(&prism(&panel, CEILING, CEILING + 4.0, None, 0.0, 0.0));
        }
    }
    for offset in [-32.0, 32.0] {
        brushes.push_str(&boxed(
            (offset - 1.0, -80.0, 52.0),
            (offset + 1.0, 80.0, CEILING),
        ));
        brushes.push_str(&boxed(
            (-80.0, offset - 1.0, 52.0),
            (80.0, offset + 1.0, CEILING),
        ));
    }
    for face in 0..6 {
        brushes.push_str(&if doors.contains(&face) {
            band(face, 0.0, 8.0, 72.0, 128.0)
        } else {
            wall(face, 0.0, 80.0)
        });
    }
    let columns: &[(f64, f64)] = if matches!(form, Form::Cut) {
        &[(0.0, -80.0), (0.0, 80.0)]
    } else {
        &[(-32.0, -32.0), (-32.0, 32.0), (32.0, -32.0), (32.0, 32.0)]
    };
    for &(x, y) in columns {
        brushes.push_str(&prism(
            &[
                (x - 8.0, y - 8.0),
                (x + 8.0, y - 8.0),
                (x + 8.0, y + 8.0),
                (x - 8.0, y + 8.0),
            ],
            FLOOR_TOP,
            CEILING,
            None,
            0.5,
            0.0,
        ));
    }
    if matches!(form, Form::Baffle) {
        brushes.push_str(&boxed((16.0, -48.0, FLOOR_TOP), (24.0, 48.0, CEILING)));
    }
    if matches!(form, Form::Recess) {
        for x in [-16.0, 16.0] {
            brushes.push_str(&boxed((x - 2.0, 94.0, FLOOR_TOP), (x + 2.0, 100.0, 48.0)));
        }
    }
    let mut lights = String::new();
    // Three identical housings, one live source. The rhythm is construction,
    // not a navigation signal; colour and output remain owned by shared style.
    for x in [-64.0, 0.0, 64.0] {
        let (fixture, source) = ceiling_fixture(x, 0.0, CEILING, 12.0, 3.0);
        brushes.push_str(&fixture);
        if x == 0.0 {
            lights.push_str(&source);
        }
    }
    let mut out = format!("// The Third Light: {name}.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(&format!("authored/{name}"), archetype, variant, 1, 3)
            .with_register("liminal_grid")
            .with_register_scope("liminal_grid")
            .emit(),
    );
    out.push_str(&tile_cell(
        0,
        0,
        0,
        1,
        if matches!(form, Form::Cut) {
            "open"
        } else {
            "solid"
        },
    ));
    for &face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("back_{face}"),
            0,
            0,
            0,
        ));
    }
    // A perimeter route works for all door pairs around the columns, baffle
    // and maintenance hole. It also leaves the narrow centre crossing optional.
    for index in 0..13u16 {
        let face = usize::from(index / 2) % 6;
        let a = corners()[face];
        let b = corners()[(face + 1) % 6];
        let p = if index % 2 == 0 { a } else { lerp(a, b, 0.5) };
        let scale = if p.0.abs() < 0.1 { 0.64 } else { 0.70 };
        // The maintenance tile's two columns occupy the axial tips, so pass
        // their inside edge. The hole stops at y=40 and this route is y=64.
        let y = if matches!(form, Form::Cut) && p.0.abs() < 0.1 {
            p.1.signum() * 64.0
        } else {
            p.1 * scale
        };
        out.push_str(&deck_node(index, p.0 * scale, y, FLOOR_TOP));
    }
    out.push_str(&lights);
    out
}

pub fn grid() -> String {
    cell(
        "back_grid",
        "expanse",
        BASE,
        &[0, 1, 2, 3, 4, 5],
        Form::Grid,
    )
}
pub fn baffle() -> String {
    cell(
        "back_baffle",
        "expanse",
        BASE + 1,
        &[0, 1, 2, 3, 4, 5],
        Form::Baffle,
    )
}
pub fn cut() -> String {
    cell(
        "back_cut",
        "expanse",
        BASE + 2,
        &[0, 1, 2, 3, 4, 5],
        Form::Cut,
    )
}
pub fn corner() -> String {
    cell(
        "back_corner",
        "hall_junction_3way",
        BASE,
        &[0, 1, 2],
        Form::Grid,
    )
}
pub fn edge() -> String {
    cell(
        "back_edge",
        "hall_junction_4way",
        BASE,
        &[0, 1, 2, 3],
        Form::Grid,
    )
}
pub fn recess() -> String {
    cell(
        "back_recess",
        "hall_junction_4way",
        BASE + 1,
        &[0, 1, 2, 3],
        Form::Recess,
    )
}
pub fn builders() -> Vec<Builder> {
    vec![
        ("back_grid", grid),
        ("back_baffle", baffle),
        ("back_cut", cut),
        ("back_corner", corner),
        ("back_edge", edge),
        ("back_recess", recess),
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
    fn sources_reproduce_and_match_real_liminal_demands() {
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
    fn every_door_pair_clears_columns_baffle_and_low_ceiling() {
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
    fn narrow_crossing_is_walkable_both_ways_and_its_opening_is_real() {
        let tile = crate::parse_authored_module(&cut())
            .expect("valid source")
            .prototype;
        for sign in [-1.0, 1.0] {
            walk(
                &tile,
                Vec3::new(sign * 6.3, 0.5, 0.0),
                Vec3::new(-sign * 6.3, 0.5, 0.0),
                false,
            );
        }
        let config = FpsConfig::default();
        let scene = RapierTraversalScene::from_arena_spec(&tile.arena_spec());
        let mut body = FpsBody::spawned(Vec3::new(0.0, 0.6 + config.half_height, -1.6), 0.0);
        for _ in 0..180 {
            step_character(
                &scene,
                &mut body,
                PlayerIntent::default(),
                &config,
                1.0 / 60.0,
            );
            if body.position.y < -2.0 {
                return;
            }
        }
        panic!(
            "the maintenance opening has a hidden floor at {:?}",
            body.position
        );
    }
}
