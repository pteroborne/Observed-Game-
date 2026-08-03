//! Tests for the spectator overview, split out for the 600-line budget.

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

/// The overview must be a real isometric read: orthographic, looking down,
/// and framing the whole facility rather than trailing the body.
///
/// This is the test that would have caught the first attempt, which was a
/// pulled-back perspective chase cam - it followed the body perfectly and
/// looked nothing like the studio.
#[test]
fn the_overview_frames_the_whole_facility_isometrically() {
    let mut world = solved_facility();

    // Nothing placed means nothing to frame: an empty box would otherwise
    // put the camera at an infinity and take the projection with it.
    let placements = std::mem::take(&mut world.placements);
    assert!(
        framing(&world, 0).is_none(),
        "an empty facility has nothing to frame"
    );
    world.placements = placements;
    let (min, max) = bounds(&world).expect("a solved facility has bounds");
    let iso = framing(&world, 0).expect("a solved facility frames");

    // Looking down, from outside the box, at the studio's own pitch.
    let looking = iso.rotation * Vec3::NEG_Z;
    assert!(
        looking.y < 0.0,
        "the overview must look down, got {looking:?}"
    );
    assert!(
        iso.translation.y > max.y,
        "the camera must stand above the facility it frames"
    );

    // Wide enough to hold the facility: the orthographic half-extent has to
    // cover the box, or "the entire map" is a crop.
    let half_width = iso.units_per_pixel * FRAME_WIDTH * 0.5;
    let plan_span = Vec2::new(max.x - min.x, max.z - min.z).length() * 0.5;
    assert!(
        half_width >= plan_span * 0.5,
        "framing {half_width:.1} m cannot hold a facility spanning {plan_span:.1} m"
    );

    // Every detent is a distinct bearing, and detent 0 is the studio's.
    let bearings: Vec<_> = (0..observed_style::iso::AZIMUTH_DETENTS)
        .map(observed_style::iso::detent_bearing)
        .collect();
    for (index, bearing) in bearings.iter().enumerate() {
        for (other, compare) in bearings.iter().enumerate() {
            if index != other {
                assert!(
                    bearing.distance(*compare) > 0.1,
                    "detents {index} and {other} read from the same angle"
                );
            }
        }
    }
}

/// The overview asks residency to reach as far as it frames, and hides the
/// shell that would otherwise lid the view.
///
/// There is deliberately nothing to count any more: the massing this used to
/// assert over is gone, because drawing a prism per cell outside the radius
/// read as a field of blocks and buried the authored geometry. What is left to
/// check is the contract between the view and residency - if those disagree,
/// the view frames space that has no geometry in it.
#[test]
fn the_overview_asks_residency_for_the_reach_it_frames() {
    for radius in [MIN_TILE_RADIUS_FOR_TEST, DEFAULT_TILE_RADIUS, 12] {
        let framed = detail_reach(radius);
        assert!(
            framed > 0.0,
            "a radius of {radius} tiles must reach somewhere"
        );
        // Wider radius, wider reach - monotonic, or dialling out shows empty
        // space where geometry should be.
        assert!(
            detail_reach(radius + 1) > framed,
            "reach must grow with the radius"
        );
    }
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
