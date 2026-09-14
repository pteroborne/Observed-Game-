//! The Empty Audience: a ceremonial axis, twin ascents and one raised reveal.
//! All nine designs are ordinary Facet Monument WFC candidates. Their shared
//! stepped bands and tapered buttresses carry the building across cell seams.

use super::entities::{
    Meta, lateral_port, stair_node, tile_cell, tile_light, vertical_port, worldspawn,
};
use super::geometry::{
    FLOOR_TOP, LEVEL, P2, WALL, band, boxed, centroid, corners, edge, hex_slab, lerp,
    offset_inward, plane3, prism, sloped_prism, wall,
};
use super::{Builder, GENERATED_NOTE};

const BASE: i32 = 130;

/// Extrude a convex (y,z) section along x. The same operation builds tapered
/// buttresses and the four solid pieces of a pointed portal reveal.
fn section(profile: &[P2], x0: f64, x1: f64) -> String {
    let middle = centroid(profile);
    let inside = ((x0 + x1) * 0.5, middle.0, middle.1);
    let mut out = String::from("{\n");
    for x in [x0, x1] {
        out.push_str(&plane3(
            (x, profile[0].0, profile[0].1),
            (x, profile[1].0, profile[1].1),
            (x, profile[2].0, profile[2].1),
            inside,
        ));
    }
    for i in 0..profile.len() {
        let a = profile[i];
        let b = profile[(i + 1) % profile.len()];
        out.push_str(&plane3(
            (x0, a.0, a.1),
            (x1, a.0, a.1),
            (x1, b.0, b.1),
            inside,
        ));
    }
    out.push_str("}\n");
    out
}

fn threshold(face: usize, level: f64) -> String {
    let (a, b) = edge(face);
    let length = (b.0 - a.0).hypot(b.1 - a.1);
    let left = lerp(a, b, 0.5 - 36.0 / length);
    let right = lerp(a, b, 0.5 + 36.0 / length);
    let strip = |a, b, bottom, top| {
        let (ia, ib) = offset_inward(a, b, WALL);
        prism(&[a, b, ib, ia], bottom, top, None, 0.0, 0.0)
    };
    // Slender full-height piers preserve the catalogue's boundary envelope.
    let mut out = strip(a, left, 0.0, level + 32.0);
    out.push_str(&strip(right, b, 0.0, level + 32.0));
    out.push_str(&strip(
        lerp(a, b, 0.5 - 44.0 / length),
        left,
        level + 32.0,
        level + LEVEL,
    ));
    out.push_str(&strip(
        right,
        lerp(a, b, 0.5 + 44.0 / length),
        level + 32.0,
        level + LEVEL,
    ));
    out.push_str(&strip(left, right, level + 72.0, level + 80.0));
    out
}

fn stepped_wall(face: usize, height: f64) -> String {
    let mut out = String::new();
    for (depth, bottom, top) in [
        (24.0, 0.0, height * 0.34),
        (16.0, height * 0.34, height * 0.67),
        (8.0, height * 0.67, height),
    ] {
        out.push_str(&band(face, 0.0, depth, bottom, top));
    }
    out
}

fn buttresses(height: f64) -> (String, String) {
    let mut brushes = String::new();
    let mut lights = String::new();
    for sign in [-1.0, 1.0] {
        let profile = [
            (sign * 66.0, 8.0),
            (sign * 98.0, 8.0),
            (sign * 98.0, height),
            (sign * 85.0, height - 6.0),
            (sign * 66.0, 30.0),
        ];
        brushes.push_str(&section(&profile, -12.0, 12.0));
        // Recess the shared practical beneath the upper ledge, not on a pole.
        brushes.push_str(&boxed(
            (-18.0, sign * 81.0 - 5.0, height - 26.0),
            (18.0, sign * 81.0 + 5.0, height - 22.0),
        ));
        lights.push_str(&tile_light(0.0, sign * 78.0, height - 28.0));
    }
    (brushes, lights)
}

#[derive(Clone, Copy)]
enum Form {
    Procession,
    Crossing,
    Foundation,
    Branch,
    Landing,
    Dais,
}

fn horizontal(name: &str, archetype: &str, variant: i32, doors: &[usize], form: Form) -> String {
    let crossing = matches!(form, Form::Crossing);
    let dais = matches!(form, Form::Dais);
    let foundation = matches!(form, Form::Foundation);
    let mut brushes = if crossing {
        let hex = corners();
        let mut deck = String::new();
        for face in 0..6 {
            let a = hex[face];
            let b = hex[(face + 1) % 6];
            deck.push_str(&prism(
                &[a, b, (b.0 * 0.55, b.1 * 0.55), (a.0 * 0.55, a.1 * 0.55)],
                0.0,
                FLOOR_TOP,
                None,
                0.0,
                0.0,
            ));
        }
        deck.push_str(&prism(
            &[(-65.0, -22.0), (65.0, -22.0), (65.0, 22.0), (-65.0, 22.0)],
            0.0,
            FLOOR_TOP,
            None,
            0.0,
            0.0,
        ));
        deck
    } else {
        hex_slab(0.0, FLOOR_TOP, 0.0, 0.0)
    };
    for face in 0..6 {
        brushes.push_str(&if doors.contains(&face) {
            threshold(face, 0.0)
        } else if dais {
            wall(face, 0.0, 32.0)
        } else {
            let height = if foundation {
                LEVEL
            } else if matches!(form, Form::Procession) {
                76.0
            } else {
                44.0
            };
            stepped_wall(face, height)
        });
    }
    if foundation {
        brushes.push_str(&hex_slab(120.0, LEVEL, 0.0, 0.0));
    }
    let mut lights = String::new();
    if dais {
        // Three pointed reveals with a decreasing aperture. The podium remains
        // empty; its broad low ramp is physical geometry, not a teleport cue.
        for (x, half, spring, peak) in [
            (-10.0, 68.0, 84.0, 126.0),
            (24.0, 56.0, 76.0, 116.0),
            (58.0, 44.0, 68.0, 106.0),
        ] {
            for sign in [-1.0, 1.0] {
                brushes.push_str(&section(
                    &[
                        (sign * half, 8.0),
                        (sign * (half + 16.0), 8.0),
                        (sign * (half + 16.0), spring),
                        (sign * half, spring),
                    ],
                    x - 4.0,
                    x + 4.0,
                ));
                brushes.push_str(&section(
                    &[
                        (sign * half, spring - 8.0),
                        (sign * (half + 16.0), spring),
                        (0.0, peak),
                        (0.0, peak - 14.0),
                    ],
                    x - 4.0,
                    x + 4.0,
                ));
            }
            brushes.push_str(&boxed(
                (x - 8.0, -17.0, peak - 11.0),
                (x + 3.0, 17.0, peak - 7.0),
            ));
            lights.push_str(&tile_light(x - 7.0, 0.0, peak - 13.0));
        }
        brushes.push_str(&prism(
            &[
                (6.0, -32.0),
                (20.0, -46.0),
                (80.0, -46.0),
                (94.0, -32.0),
                (94.0, 32.0),
                (80.0, 46.0),
                (20.0, 46.0),
                (6.0, 32.0),
            ],
            8.0,
            20.0,
            None,
            3.0,
            0.0,
        ));
        brushes.push_str(&sloped_prism(
            &[(-30.0, -28.0), (12.0, -28.0), (12.0, 28.0), (-30.0, 28.0)],
            0.0,
            [(-30.0, -28.0, 8.0), (-30.0, 28.0, 8.0), (12.0, -28.0, 20.0)],
            None,
        ));
    } else {
        let height = if matches!(form, Form::Landing | Form::Branch) {
            80.0
        } else {
            120.0
        };
        let (supports, practicals) = buttresses(height);
        brushes.push_str(&supports);
        lights.push_str(&practicals);
    }
    let mut out =
        format!("// Empty Audience: {name}. Architectural affordances only.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell(&format!("authored/{name}"), archetype, variant, 1, 3)
            .with_register_scope("facet_monument")
            .emit(),
    );
    out.push_str(&tile_cell(
        0,
        0,
        0,
        1,
        if crossing { "open" } else { "solid" },
    ));
    for &face in doors {
        out.push_str(&lateral_port(
            face,
            "door",
            &format!("audience_{face}"),
            0,
            0,
            0,
        ));
    }
    out.push_str(&lights);
    out
}

pub fn procession() -> String {
    horizontal(
        "audience_procession",
        "hall_straight",
        BASE,
        &[0, 3],
        Form::Procession,
    )
}
pub fn crossing() -> String {
    horizontal(
        "audience_crossing",
        "hall_straight",
        BASE + 1,
        &[0, 3],
        Form::Crossing,
    )
}
pub fn foundation() -> String {
    horizontal(
        "audience_foundation",
        "hall_straight",
        BASE + 2,
        &[0, 3],
        Form::Foundation,
    )
}
pub fn branch() -> String {
    horizontal(
        "audience_branch",
        "hall_junction_4way",
        BASE,
        &[0, 1, 3, 5],
        Form::Branch,
    )
}
pub fn landing() -> String {
    horizontal(
        "audience_landing",
        "hall_turn_120",
        BASE,
        &[0, 4],
        Form::Landing,
    )
}
pub fn elbow() -> String {
    horizontal(
        "audience_elbow",
        "hall_turn_60",
        BASE,
        &[0, 5],
        Form::Landing,
    )
}
pub fn gallery() -> String {
    horizontal(
        "audience_gallery",
        "hall_straight",
        BASE + 3,
        &[0, 3],
        Form::Landing,
    )
}

pub fn dais() -> String {
    horizontal(
        "audience_dais",
        "hall_turn_120",
        BASE + 1,
        &[2, 4],
        Form::Dais,
    )
}

pub fn ascent() -> String {
    let height = |x: f64| FLOOR_TOP + (x + 112.0) * LEVEL / 224.0;
    let mut brushes = hex_slab(0.0, FLOOR_TOP, 0.0, 0.0);
    let strip = |x: f64, a: f64, b: f64, rise: f64| {
        sloped_prism(
            &[(-x, a), (x, a), (x, b), (-x, b)],
            0.0,
            [
                (-112.0, a, height(-112.0) + rise),
                (-112.0, b, height(-112.0) + rise),
                (112.0, a, height(112.0) + rise),
            ],
            None,
        )
    };
    brushes.push_str(&strip(112.0, -36.0, 36.0, 0.0));
    for sign in [-1.0, 1.0] {
        // Three continuous courses rise with each flank, matching the language
        // of the processional walls without blocking the observer's eye line.
        brushes.push_str(&strip(104.0, sign * 38.0, sign * 46.0, 14.0));
        brushes.push_str(&strip(96.0, sign * 48.0, sign * 62.0, -2.0));
        brushes.push_str(&strip(80.0, sign * 64.0, sign * 78.0, -18.0));
    }
    for face in [1, 2, 4, 5] {
        brushes.push_str(&wall(face, 0.0, 28.0));
    }
    brushes.push_str(&threshold(3, 0.0));
    brushes.push_str(&threshold(0, LEVEL));
    let mut lights = String::new();
    for x in [-60.0, 60.0] {
        let z = height(x) + 18.0;
        brushes.push_str(&boxed((x - 16.0, 50.0, z), (x + 16.0, 60.0, z + 4.0)));
        lights.push_str(&tile_light(x, 49.0, z - 2.0));
    }
    let mut out =
        format!("// Empty Audience: full-storey ascent with stepped flanks.\n{GENERATED_NOTE}");
    out.push_str(&worldspawn(&brushes));
    out.push_str(
        &Meta::cell("authored/audience_ascent", "hall_ramp", BASE, 2, 3)
            .with_register_scope("facet_monument")
            .emit(),
    );
    out.push_str(&tile_cell(0, 0, 0, 2, "ramp"));
    out.push_str(&lateral_port(3, "door", "audience_entry", 0, 0, 0));
    out.push_str(&vertical_port("up", "ramp_open", "audience_ascent", 0));
    for index in 0..5u16 {
        let x = -112.0 + f64::from(index) * 56.0;
        out.push_str(&stair_node(index, x, 0.0, height(x)));
    }
    out.push_str(&lights);
    out
}

pub fn builders() -> Vec<Builder> {
    vec![
        ("audience_procession", procession),
        ("audience_crossing", crossing),
        ("audience_foundation", foundation),
        ("audience_branch", branch),
        ("audience_landing", landing),
        ("audience_dais", dais),
        ("audience_ascent", ascent),
        ("audience_elbow", elbow),
        ("audience_gallery", gallery),
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
    fn audience_sources_reproduce_and_serve_real_demands() {
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

    #[test]
    fn all_door_pairs_both_ascent_directions_and_the_dais_are_walkable() {
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
            let mut paths: Vec<_> = if name == "audience_ascent" {
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
            if name == "audience_dais" {
                paths.push((Vec3::new(-2.0, 0.5, 0.0), Vec3::new(4.0, 1.25, 0.0)));
            }
            for (start, goal) in paths {
                let mut body = FpsBody::spawned(start + Vec3::Y * (config.half_height + 0.15), 0.0);
                let mut reached = false;
                for _ in 0..1200 {
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
}
