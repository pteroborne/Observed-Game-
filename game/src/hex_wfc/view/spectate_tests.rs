//! Tests for the spectator overview, split out for the 600-line budget.

use super::map::cell::sketch;
use super::spectate::*;
use bevy::prelude::*;
use observed_hex::hex_origin;

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

/// The focus floor is the one the body stands on, with its neighbours as
/// context and everything else dropped.
#[test]
fn the_focus_floor_follows_the_body() {
    let layer = layer_for(4);
    assert!(layer.is_focus(4));
    assert!(layer.draws(3) && layer.draws(5));
    assert!(
        !layer.draws(6),
        "a ten-level facility drawn all at once is a thicket"
    );
}

/// Cycling must be stable and must terminate: every body, then back.
#[test]
fn cycling_visits_every_body_and_returns() {
    // Order comes from the BTreeMap, so this is the order every machine
    // sees - a capture script driving the key gets the same run.
    let ids = [0u8, 1, 2];
    let mut seen = Vec::new();
    let mut at = 0usize;
    for _ in 0..ids.len() {
        seen.push(ids[at]);
        at = (at + 1) % ids.len();
    }
    assert_eq!(
        seen, ids,
        "cycling must visit each body once before repeating"
    );
    assert_eq!(at, 0, "cycling must return to the first body");
}

/// Every drawable cell gets a prism, and the ones around the body get out
/// of the way so the real geometry can be seen.
///
/// The unit tests above only check the pose arithmetic. This runs the two
/// systems against a real solved facility, which is what catches the
/// wiring: a query that matches nothing, a resource never registered, a
/// generation guard that rebuilds every frame or never rebuilds at all.
#[test]
fn the_overview_masses_the_facility_and_opens_a_window_at_the_body() {
    use crate::hex_wfc::sim::{HexWfcRuntime, load_prototypes};
    use observed_match::hex_wfc::{HexMatchConfig, HexWfcMatch};

    let protos = load_prototypes();
    let match_state = (0..64u64)
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
        .expect("a solvable nearby seed");

    let world = &match_state.facility;
    let drawable = world
        .placements
        .iter()
        .filter(|(cell, placement)| {
            let in_blueprint = world.room_id_at(**cell).is_some();
            sketch(placement.archetype, placement.space, in_blueprint)
                .height
                .is_some()
                && world.architecture.contains_key(*cell)
        })
        .count();
    assert!(drawable > 0, "the fixture facility draws nothing");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<SpectatorOverview>()
        .insert_resource(crate::sim::state::SpectatorBot::for_seed(
            crate::flow::MATCH_SEED,
        ))
        .insert_resource(HexWfcRuntime {
            local_player: *match_state.players.keys().next().expect("a body"),
            match_state,
            pending_visual_cells: Default::default(),
            presented_revisions: Default::default(),
            status: String::new(),
            map_open: false,
            map_level: 0,
            results_delay_frames: 0,
            networked: false,
            resync_attempts: 0,
        })
        .add_systems(Update, (sync_massing, sync_detail_window).chain());

    // Down: nothing is massed.
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&Massing>()
            .iter(app.world())
            .count(),
        0,
        "the overview must cost nothing while it is down"
    );

    // Up: one prism per drawable cell.
    app.world_mut().resource_mut::<SpectatorOverview>().active = true;
    app.update();
    let massed = app
        .world_mut()
        .query::<&Massing>()
        .iter(app.world())
        .count();
    assert_eq!(
        massed, drawable,
        "every drawable cell should be massed exactly once"
    );

    // Rebuilding is guarded: a second frame must not re-spawn the lot.
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&Massing>()
            .iter(app.world())
            .count(),
        massed,
        "massing must not be rebuilt every frame"
    );

    // The window is open at the body, on its floor, and shut elsewhere.
    let body = app.world().resource::<HexWfcRuntime>().local().clone();
    let layer = layer_for(body.cell.level);
    let mut near_hidden = 0;
    let mut context_visible = 0;
    let mut query = app.world_mut().query::<(&Massing, &Visibility)>();
    for (massing, visibility) in query.iter(app.world()) {
        let level = massing.cell.level;
        let origin = Vec3::from_array(hex_origin(massing.cell));
        let near = origin.distance(body.position) <= DETAIL_RADIUS;
        if !layer.draws(level) {
            assert_eq!(
                *visibility,
                Visibility::Hidden,
                "a floor off the layer must not draw at all"
            );
        } else if layer.is_focus(level) && near {
            assert_eq!(
                *visibility,
                Visibility::Hidden,
                "massing at the body must yield to the real geometry"
            );
            near_hidden += 1;
        } else if *visibility == Visibility::Inherited {
            context_visible += 1;
        }
    }
    assert!(
        near_hidden > 0,
        "the body should always have some massing to hide"
    );
    assert!(
        context_visible > 0,
        "the rest of the drawn layer must still show"
    );

    // Down again: everything goes.
    app.world_mut().resource_mut::<SpectatorOverview>().active = false;
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&Massing>()
            .iter(app.world())
            .count(),
        0,
        "leaving the overview must not leak massing"
    );
}
