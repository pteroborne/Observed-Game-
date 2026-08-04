//! Where the camera stands: the play eye, the spectator chase, and the
//! overview's isometric framing.
//!
//! Split out of `lighting`, which had accumulated the camera alongside the
//! light rig for no better reason than that both touch `GameCam`. They are
//! different questions - where you look from, and how the place is lit - and
//! keeping them together is what let three systems each derive the overview's
//! framing for themselves and drift apart.
//!
//! **One writer, three readers.** [`OverviewFrame`] is computed once per frame
//! by [`sync_frame`]; the camera pose, the orthographic scale, and the fog
//! range all read it. They used to derive it independently, and two of the
//! three were updated to frame the body's tile while the third still framed the
//! whole facility - so the camera and the zoom disagreed about what was being
//! looked at, and the view came out a thumbnail in an empty frame.

use bevy::prelude::*;
use observed_hex::hex_origin;

use super::spectate::SpectatorOverview;
use crate::hex_wfc::sim::{EYE_OFFSET, HexWfcRuntime};
use crate::view::components::GameCam;

/// Put the shared camera at the authoritative eye pose before shell entities are
/// enqueued. This removes the one-frame menu-camera vantage from state entry.
pub(super) fn prime_camera(
    transform: &mut Transform,
    player: &observed_match::hex_wfc::HexPlayerState,
) {
    let (eye, rotation) = player_eye_pose(player);
    transform.translation = eye;
    transform.rotation = rotation;
}

/// How far behind and above the bot the spectator camera trails, in metres, and
/// how far it looks down. Matches the over-the-shoulder framing already proven
/// by `hex_wfc_lab`'s bot-POV capture.
/// A hex cell's plan width in metres; `hex_origin` steps by this.
pub(in crate::hex_wfc::view) const TILE_SPAN: f32 = 14.0;

/// The viewport the framing falls back to when there is no window to measure.
const FRAME_WIDTH: f32 = 1600.0;
const FRAME_HEIGHT: f32 = 1000.0;

/// How fast the overview camera eases toward its framing, per second.
///
/// Slower than the chase: at facility scale a snap reads as the whole building
/// lurching rather than the camera moving.
const RESPONSE: f32 = 2.5;

/// Where the overview's fog begins and ends, as fractions of the framing's far
/// plane. Far enough back that the whole facility is legible, close enough that
/// depth still reads across it.
pub(in crate::hex_wfc::view) const OVERVIEW_FOG_START: f32 = 0.45;
pub(in crate::hex_wfc::view) const OVERVIEW_FOG_END: f32 = 1.05;

const CHASE_BACK: f32 = 2.6;
const CHASE_RISE: f32 = 1.6;
const CHASE_PITCH: f32 = -0.26;
/// Fraction of the remaining gap the chase camera closes per second. The bot's
/// yaw is a simulation value that can still swing quickly; easing toward it
/// keeps the view readable without altering anything the simulation sees.
const CHASE_RESPONSE: f32 = 6.0;

pub(in crate::hex_wfc) fn sync_camera(
    runtime: Res<HexWfcRuntime>,
    spectating: Option<Res<crate::sim::state::SpectatorBot>>,
    frame: Res<OverviewFrame>,
    time: Res<Time>,
    mut camera: Query<&mut Transform, With<GameCam>>,
    mut was_overviewing: Local<bool>,
) {
    let player = runtime.local();
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    if spectating.is_none() {
        // Human play is untouched: the eye pose, rigidly, every frame.
        let (eye, rotation) = player_eye_pose(player);
        transform.translation = eye;
        transform.rotation = rotation;
        return;
    }

    // Spectating: trail the bot instead of riding inside its head, and ease
    // rather than snap. A bot can still change heading faster than is
    // comfortable to watch from the first person, and this is presentation
    // only — no simulation state is read back or written.
    // Two spectator poses, one camera. The overview is further out and eases
    // more slowly; the chase is the close read. Which one is a question about
    // presentation, so it is answered here rather than by spawning a second
    // camera - Bevy hands UI to the highest-order camera on the window and
    // ignores `is_active`, so a dormant one would take every overlay with it.
    let overviewing = frame.0.is_some();
    let iso = frame.0;
    if let Some(iso) = &iso
        && std::env::var("OBSERVED2_SPECTATE_TRACE").is_ok()
    {
        // One line a second while the trace is on. This exists because the
        // last attempt at this was debugged by reasoning and got it wrong: the
        // numbers are cheap and the guesses were not.
        if time.elapsed_secs() as u32 != (time.elapsed_secs() - time.delta_secs()) as u32 {
            let to_body = iso.translation.distance(player.position);
            info!(
                "spectate: body={:?} cam={:?} dist={to_body:.1} upp={:.3} far={:.1} fog={:.1}..{:.1}",
                player.position,
                iso.translation,
                iso.units_per_pixel,
                iso.far,
                iso.far * OVERVIEW_FOG_START,
                iso.far * OVERVIEW_FOG_END,
            );
        }
    }
    let (target_translation, target_rotation, response) = if let Some(iso) = &iso {
        (iso.translation, iso.rotation, response())
    } else {
        let forward = Vec3::new(player.yaw.sin(), 0.0, -player.yaw.cos());
        let eye = player.position + Vec3::Y * EYE_OFFSET;
        (
            eye + Vec3::Y * CHASE_RISE - forward * CHASE_BACK,
            Quat::from_rotation_y(-player.yaw) * Quat::from_rotation_x(CHASE_PITCH),
            CHASE_RESPONSE,
        )
    };
    // Snap on the way in and out of the overview, ease within it.
    //
    // Easing is right for following a body and for turning a detent, and wrong
    // for the mode change: the play pose and the overview pose are hundreds of
    // metres apart, so at 2.5 per second the camera is still ~50 m short a
    // second later - which reads as the whole facility sitting off-centre.
    let switched = *was_overviewing != overviewing;
    *was_overviewing = overviewing;
    if switched || transform.translation == Vec3::ZERO {
        transform.translation = target_translation;
        transform.rotation = target_rotation;
        return;
    }
    let t = (response * time.delta_secs()).clamp(0.0, 1.0);
    transform.translation = transform.translation.lerp(target_translation, t);
    transform.rotation = transform.rotation.slerp(target_rotation, t);
}

/// Perspective for play, orthographic for the overview.
///
/// An isometric read has to be orthographic: under perspective the far side of
/// the facility shrinks, parallel corridors converge, and the cutaway's whole
/// premise - that a wall's plan azimuth tells you whether it is in your way -
/// stops holding.
pub(in crate::hex_wfc) fn sync_projection(
    settings: Res<crate::settings::Settings>,
    frame: Res<OverviewFrame>,
    mut projection: Query<&mut Projection, With<GameCam>>,
) {
    let Ok(mut projection) = projection.single_mut() else {
        return;
    };
    let iso = frame.0;
    if let Some(iso) = iso {
        *projection = Projection::Orthographic(OrthographicProjection {
            scale: iso.units_per_pixel,
            near: 0.1,
            far: iso.far,
            ..OrthographicProjection::default_3d()
        });
        return;
    }
    if let Projection::Perspective(perspective) = &mut *projection {
        let target = settings.fov_degrees.clamp(50.0, 80.0).to_radians();
        if (perspective.fov - target).abs() > f32::EPSILON {
            perspective.fov = target;
        }
    } else {
        // Coming back from the overview: restore the play projection at the
        // player's chosen field of view rather than leaving them orthographic.
        *projection = Projection::Perspective(PerspectiveProjection {
            fov: settings.fov_degrees.clamp(50.0, 80.0).to_radians(),
            ..default()
        });
    }
}

pub(in crate::hex_wfc::view) fn player_eye_pose(
    player: &observed_match::hex_wfc::HexPlayerState,
) -> (Vec3, Quat) {
    let eye = player.position + Vec3::Y * EYE_OFFSET;
    let rotation = Quat::from_rotation_y(-player.yaw) * Quat::from_rotation_x(player.pitch);
    (eye, rotation)
}

/// The overview's framing for this frame, computed once.
///
/// Three systems need it - the camera pose, the orthographic scale, and the
/// fog range - and each used to derive it for itself. They drifted, exactly as
/// duplicated derivations do: two were updated to frame the body's tile while
/// the third still framed the whole facility, so the camera and the zoom
/// disagreed about what was being looked at and the view came out a thumbnail
/// in an empty frame. One writer, three readers.
#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct OverviewFrame(
    pub(in crate::hex_wfc) Option<observed_style::iso::IsoFraming>,
);

/// Compute the framing. Must run before anything that reads it.
pub(in crate::hex_wfc) fn sync_frame(
    runtime: Res<HexWfcRuntime>,
    overview: Res<SpectatorOverview>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut frame: ResMut<OverviewFrame>,
) {
    // Fitted to the window that is actually there: `ScalingMode::WindowSize`
    // reads the scale as world units per *pixel*, so framing against a nominal
    // viewport just means a wider window shows more world.
    let (width, height) = windows
        .single()
        .map_or((FRAME_WIDTH, FRAME_HEIGHT), |window| {
            (window.width().max(1.0), window.height().max(1.0))
        });
    frame.0 = overview
        .active
        .then(|| {
            framing_around(
                &runtime.match_state.facility,
                tile_centre(runtime.local().cell),
                overview.detent,
                overview.tile_radius,
                width,
                height,
            )
        })
        .flatten();
}

/// The centre of a cell, in world space.
///
/// The overview frames *this* rather than the body's own position. Following a
/// walking body means the whole facility slides continuously under a fixed
/// camera, which is unreadable at this scale and unpleasant to watch; snapping
/// to the tile means the view holds still while the body crosses it and steps
/// once when the body does.
#[must_use]
pub(in crate::hex_wfc) fn tile_centre(cell: observed_facility::hex_wfc::HexCoord) -> Vec3 {
    Vec3::from_array(hex_origin(cell))
}

/// The world height band a storey occupies.
#[must_use]
pub(in crate::hex_wfc) fn storey(level: u8) -> (f32, f32) {
    let floor = f32::from(level) * observed_hex::TILE_LEVEL_HEIGHT;
    (floor, floor + observed_hex::TILE_LEVEL_HEIGHT)
}

/// The box the facility occupies, from its own placements.
///
/// Measured rather than derived from `config.cols/rows/levels` so a facility
/// with an irregular edge frames to what is actually there.
#[must_use]
pub(in crate::hex_wfc::view) fn bounds(
    world: &observed_facility::hex_wfc::HexWfcWorld,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for &cell in world.placements.keys() {
        let origin = Vec3::from_array(hex_origin(cell));
        // A cell is about 14 m across and one level tall; padding by that keeps
        // the rim of the outermost cells inside the frame.
        min = min.min(origin - Vec3::new(7.0, 0.0, 8.0));
        max = max.max(origin + Vec3::new(7.0, 8.0, 8.0));
    }
    (min.x <= max.x).then_some((min, max))
}

// The whole-facility framing variants are gone. They existed so a caller could
// ask "frame the building" and were the trap that caused the view to come out a
// thumbnail: two of three call sites moved to the radius framing and the third
// kept asking for the facility. With one writer there is no third call site,
// and with no function there is nothing to call by mistake.

/// Frame `radius` tiles around `body`, clamped to the facility.
///
/// This is what follows the spectator: the box is centred on the body, not on
/// the building, so walking moves the view.
#[must_use]
pub(in crate::hex_wfc) fn framing_around(
    world: &observed_facility::hex_wfc::HexWfcWorld,
    body: Vec3,
    detent: usize,
    radius: u8,
    width: f32,
    height: f32,
) -> Option<observed_style::iso::IsoFraming> {
    let (facility_min, facility_max) = bounds(world)?;
    let span = f32::from(radius) * TILE_SPAN;
    let reach = Vec3::new(span, TILE_SPAN, span);

    // Zoom from the radius box, position from the facility.
    //
    // These are two different questions and answering both from the small box
    // is what put a hard diagonal clip across the first attempt: the camera
    // stood one box-diagonal out (156 m) with a far plane of twice that, while
    // the facility runs 340 m - so the far plane sliced straight through the
    // massing, and anything behind the camera was cut by the near plane too.
    // Orthographic scale does not care how far away the camera is, so it can
    // stand right outside the whole building and still frame a few tiles.
    let tight = observed_style::iso::frame(body - reach, body + reach, detent, width, height);
    let reach_all = (facility_max - facility_min).length().max(1.0);
    // Aim a little above the body, not at its feet.
    //
    // Residency reaches upward as well as outward - the body's own level and
    // the ones above it - so the geometry around a body standing on a floor
    // sits mostly *above* that floor. Aiming at the feet puts all of it in the
    // top half of the frame. Half a cell up is where the mass actually is.
    let aim = body + Vec3::Y * TILE_SPAN * 0.5;
    Some(observed_style::iso::IsoFraming {
        // Centred on the body, not on a box clamped to the facility - clamping
        // kept the box the same size but slid its centre to the edge, which is
        // why the body sat off in a corner of the frame.
        translation: aim + tight.rotation * Vec3::Z * reach_all,
        rotation: tight.rotation,
        units_per_pixel: tight.units_per_pixel,
        far: reach_all * 2.0,
    })
}

/// The metric radius a tile radius stands for.
#[must_use]
pub(in crate::hex_wfc) fn detail_reach(radius: u8) -> f32 {
    f32::from(radius) * TILE_SPAN
}

/// How fast the camera should ease toward [`framing`].
#[must_use]
pub(in crate::hex_wfc) fn response() -> f32 {
    RESPONSE
}

#[cfg(test)]
mod tests {
    use observed_core::PlayerId;

    use super::*;

    #[test]
    fn camera_is_primed_to_the_authoritative_eye_pose() {
        let player = observed_match::hex_wfc::HexPlayerState {
            id: PlayerId(0),
            team: observed_core::TeamId(0),
            cell: observed_facility::hex_wfc::HexCoord {
                q: 2,
                r: 3,
                level: 1,
            },
            position: Vec3::new(4.0, 8.5, -2.0),
            yaw: 0.7,
            pitch: -0.2,
            escaped: false,
        };
        let mut camera = Transform::default();
        prime_camera(&mut camera, &player);
        let (eye, rotation) = player_eye_pose(&player);

        assert_eq!(camera.translation, eye);
        assert_eq!(camera.rotation, rotation);
    }
}
