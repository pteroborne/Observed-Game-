//! Tests for the spectator overview, split out for the 600-line budget.

use super::camera::{bounds, framing_around};
use super::spectate::*;
use bevy::prelude::*;

/// A real solved facility, shared by the tests below.
fn solved_facility() -> observed_facility::hex_wfc::HexWfcWorld {
    use crate::hex_wfc::sim::load_prototypes;
    use observed_match::hex_wfc::{HexMatchConfig, HexWfcMatch};

    let protos = load_prototypes();
    (0..64u64)
        .find_map(|offset| {
            HexWfcMatch::new(
                crate::flow::MATCH_SEED.wrapping_add(offset),
                HexMatchConfig {
                    teams: 1,
                    members_per_team: 1,
                    ..Default::default()
                },
                &protos,
            )
            .ok()
        })
        .expect("a solvable nearby seed")
        .facility
}

/// The camera must frame the body from outside the building, looking down.
#[test]
fn the_overview_frames_the_body_from_outside_looking_down() {
    let world = solved_facility();
    let body = Vec3::new(10.0, 1.4, 0.0);
    let iso = framing_around(&world, body, 0, DEFAULT_TILE_RADIUS, 1600.0, 1000.0)
        .expect("a solved facility frames");

    let looking = iso.rotation * Vec3::NEG_Z;
    assert!(
        looking.y < 0.0,
        "the overview must look down, got {looking:?}"
    );
    assert!(
        iso.translation.y > body.y,
        "the camera must stand above the body"
    );
    // Far enough out that the far plane clears the whole facility. Deriving it
    // from the framed box instead put a hard diagonal clip across the view.
    let (min, max) = bounds(&world).expect("bounds");
    assert!(
        iso.far > (max - min).length(),
        "the far plane must clear the facility, not just the framed box"
    );
}
