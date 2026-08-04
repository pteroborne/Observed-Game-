//! The whole facility at a glance, in ground truth, following one body.
//!
//! Bot spectator mode already trails the body it drives (`sync_camera`'s chase
//! branch), which is fine for watching a fight and useless for watching a
//! *route*. You cannot see where the bot is going, what it just left, or why a
//! climb took it the long way round. This is the other view: pull back far
//! enough to read the facility, and keep the body in it.
//!
//! # Massing far, real geometry near
//!
//! A production facility is 28 x 20 x 10 - 5,600 cells - and presentation is
//! deliberately *bounded*: `residency` admits cells within 30 m of the runner at
//! eight per frame, so the whole facility is never resident and never should be.
//! Spawning it in authored geometry would be several hundred frames of hitching
//! for a view nobody plays in.
//!
//! So the two halves are drawn by two different things, which is what makes the
//! view affordable:
//!
//! - **Far**: one prism per cell, from `map`'s sketch vocabulary - height by
//!   archetype, colour by district. Cheap, and already the language the survivor
//!   map speaks, so the overview reads as the same facility rather than a second
//!   one.
//! - **Near**: nothing of ours at all. Residency has already spawned the real
//!   authored geometry around the body, so within `DETAIL_RADIUS` the massing
//!   simply gets out of the way and the actual tiles show through.
//!
//! The seam between them is the point rather than a compromise: massing is what
//! the facility *is*, geometry is what it is *made of*, and the body is always
//! standing in the second.
//!
//! # Ground truth, and why that is safe here
//!
//! This draws every cell, not the survivor's `HexPlayerMapKnowledge`. That would
//! be a fog-of-war violation in play, and it is not one here: spectator mode is
//! entered from the menu to watch bots, has no local survivor with knowledge to
//! respect, and cannot be reached from a live match. The gate is
//! `SpectatorBot`'s presence, checked on every system in this module.
//!
//! # One camera, moved - never a second one
//!
//! Bevy hands UI to the highest-order camera on the primary window and ignores
//! `is_active`, so a dormant second camera silently swallows every overlay. This
//! module therefore owns a *pose*, not a camera: `sync_camera` asks for it and
//! moves the one camera that already exists.

use bevy::prelude::*;
use observed_hex::hex_origin;

use crate::hex_wfc::sim::HexWfcRuntime;

/// Toggles the overview while spectating.
pub(in crate::hex_wfc) const TOGGLE_KEY: KeyCode = KeyCode::KeyO;

/// Cycles which body the overview follows.
pub(in crate::hex_wfc) const CYCLE_KEY: KeyCode = KeyCode::KeyF;

/// How far from the followed body the massing yields to real geometry.
///
/// A little past `STREAM_ENTER_RADIUS` (30 m), so a cell is always resident
/// before its prism disappears. The other order leaves a hole: massing gone,
/// geometry not yet spawned, and the body apparently standing on nothing.
pub(in crate::hex_wfc::view) const DETAIL_RADIUS: f32 = 34.0;

// Compile-time, not a test: these are relationships between constants, so a
// runtime assertion could only ever restate what the compiler already knows.
// The window must open outside the radius residency spawns at, or the prism
// vanishes before its geometry exists and the body stands on a hole; and it
// must close inside the radius residency retires at, or the geometry goes and
// no prism comes back.
const _: () = assert!(DETAIL_RADIUS > super::STREAM_ENTER_RADIUS);
const _: () = assert!(DETAIL_RADIUS < super::STREAM_EXIT_RADIUS);

/// Rotate the view by one detent.
pub(in crate::hex_wfc) const ROTATE_KEY: KeyCode = KeyCode::KeyR;

/// Widen and narrow the detail radius.
pub(in crate::hex_wfc) const WIDEN_KEY: KeyCode = KeyCode::BracketRight;
pub(in crate::hex_wfc) const NARROW_KEY: KeyCode = KeyCode::BracketLeft;

/// How many tiles around the followed body the view frames.
///
/// Framing the whole facility was the first pass's mistake: 28 x 20 cells in
/// one screen makes a doorway sub-pixel, so the geometry is there and too small
/// to read. Following the body at a few tiles is the studio's working distance.
pub(in crate::hex_wfc) const DEFAULT_TILE_RADIUS: u8 = 3;
const MIN_TILE_RADIUS: u8 = 2;
/// Re-exported for the sibling test module.
#[cfg(test)]
pub(in crate::hex_wfc::view) const MIN_TILE_RADIUS_FOR_TEST: u8 = MIN_TILE_RADIUS;
const MAX_TILE_RADIUS: u8 = 12;

/// A hex cell's plan width in metres; `hex_origin` steps by this.
const TILE_SPAN: f32 = 14.0;

/// The viewport the framing is fitted to.
///
/// The studio fits to its own window; the game fits to a nominal 16:10 and lets
/// the aspect fall out of the projection, so the two agree on framing without
/// the game having to read its window size before the camera exists.
pub(in crate::hex_wfc::view) const FRAME_WIDTH: f32 = 1600.0;
pub(in crate::hex_wfc::view) const FRAME_HEIGHT: f32 = 1000.0;

/// How fast the camera eases toward its framing, per second.
///
/// Slower than the chase cam's 6.0: at facility scale a snap reads as the whole
/// building lurching rather than the camera moving.
pub(in crate::hex_wfc::view) const RESPONSE: f32 = 2.5;

/// Whether the overview is up, and what it has built.
#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct SpectatorOverview {
    pub active: bool,
    /// Which of the six 60-degree bearings the view is read from.
    ///
    /// Detents rather than free orbit because the cutaway drops the walls
    /// facing the camera: orbit freely and walls pop in and out at arbitrary
    /// angles. Six steps, one per hex face, so each has one unambiguous set of
    /// near walls - the same reading the studio uses.
    pub detent: usize,
    /// How many tiles around the body are framed.
    pub tile_radius: u8,
    /// The facility generation the massing was built from. Relayout changes
    /// placements, so massing built before it is a picture of a facility that
    /// no longer exists.
    pub(in crate::hex_wfc) built_generation: Option<u32>,
}

/// What the cutaway needs to judge one hull, measured once at spawn.
///
/// The studio recomputes this from its own cached hulls. The game already has
/// the authored geometry resident, so the shared predicate is applied to what
/// is there rather than meshing it a second time.
#[derive(Component)]
pub(in crate::hex_wfc) struct Cutaway {
    /// Centroid relative to its cell's origin - what the near-wall test reads.
    pub(in crate::hex_wfc) local: Vec3,
    pub(in crate::hex_wfc) min_y: f32,
    pub(in crate::hex_wfc) max_y: f32,
}

/// The overview's own key light.
///
/// Play stages a tight pool over the runner's cell, which is right for a body
/// inside a corridor and useless from outside: the cutaway opens a dozen tiles
/// and one of them is lit. This is the studio's answer instead - a directional
/// key aimed off the view bearing, so a wall face and the floor it stands on
/// return different values. Head-on light flattens exactly the thing a cutaway
/// exists to show.
#[derive(Component)]
pub(in crate::hex_wfc) struct OverviewKey;

/// The rhombus shell around the facility.
///
/// In play it is the far wall you never reach. From outside, looking in, it is
/// a translucent lid over the whole building - it was the grey slab across the
/// first overview capture. The overview stands outside the facility, so the
/// shell is always between the camera and everything worth seeing.
#[derive(Component)]
pub(in crate::hex_wfc) struct BoundaryShell;

/// Flip the overview, and cycle the followed body.
///
/// Both gated on `SpectatorBot`: in play these keys must keep whatever meaning
/// play gives them.
pub(in crate::hex_wfc) fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    spectating: Option<Res<crate::sim::state::SpectatorBot>>,
    mut overview: ResMut<SpectatorOverview>,
    mut runtime: ResMut<HexWfcRuntime>,
) {
    if spectating.is_none() {
        return;
    }
    if keys.just_pressed(TOGGLE_KEY) {
        overview.active = !overview.active;
    }
    if keys.just_pressed(CYCLE_KEY) {
        cycle_focus(&mut runtime);
    }
    if keys.just_pressed(ROTATE_KEY) {
        overview.detent = (overview.detent + 1) % observed_style::iso::AZIMUTH_DETENTS;
    }
    if keys.just_pressed(WIDEN_KEY) {
        overview.tile_radius = (overview.tile_radius + 1).min(MAX_TILE_RADIUS);
    }
    if keys.just_pressed(NARROW_KEY) {
        overview.tile_radius = overview.tile_radius.saturating_sub(1).max(MIN_TILE_RADIUS);
    }
}

/// Follow the next body, in a stable order.
///
/// `players` is a `BTreeMap`, so its order is the same every run and on every
/// machine - which matters because a capture script drives this key.
fn cycle_focus(runtime: &mut HexWfcRuntime) {
    let bodies: Vec<_> = runtime.match_state.players.keys().copied().collect();
    if bodies.len() < 2 {
        return;
    }
    let next = bodies
        .iter()
        .position(|id| *id == runtime.local_player)
        .map_or(0, |index| (index + 1) % bodies.len());
    runtime.local_player = bodies[next];
}

/// Ask residency for the reach the overview needs, and hide the shell.
///
/// There is deliberately no massing any more. The first pass drew a prism per
/// cell for everything outside the detail radius, on the theory that "the
/// entire map" wanted some representation of the parts you are not looking at.
/// It read as a field of blocks and buried the authored geometry it was meant
/// to surround. The view is now only what the studio shows: real tiles, cut
/// away, out to the radius - and nothing at all beyond it.
pub(in crate::hex_wfc) fn sync_detail_window(
    overview: Res<SpectatorOverview>,
    residency: Option<ResMut<super::HexPresentationResidency>>,
    mut shell: Query<&mut Visibility, With<BoundaryShell>>,
) {
    // Residency's reach *is* the extent of the view now, so the two cannot
    // disagree: what is framed is exactly what is resident.
    if let Some(mut residency) = residency {
        residency.set_reach(if overview.active {
            super::residency::Reach::out_to(detail_reach(overview.tile_radius))
        } else {
            super::residency::Reach::play()
        });
    }

    // In play the shell is the far wall you never reach; from outside looking
    // in it is a lid over everything worth seeing.
    let wanted = if overview.active {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut shell {
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Light what the overview is looking at.
///
/// Spawned and despawned with the overview, and re-aimed when the detent turns,
/// so contrast is equivalent at all six stops rather than one stop happening to
/// look flat. Play's rig is left entirely alone: it is still there underneath,
/// lighting the body's cell, and this is added over it.
pub(in crate::hex_wfc) fn sync_key_light(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    overview: Res<SpectatorOverview>,
    existing: Query<(Entity, &mut Transform), With<OverviewKey>>,
) {
    let Some(bearing) = overview
        .active
        .then(|| observed_style::iso::detent_bearing(overview.detent))
    else {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        return;
    };

    // Off the view axis, and above. Aimed at the body so the lit region tracks
    // whatever the camera is framing.
    let offset = Quat::from_rotation_y(observed_style::iso::light::KEY_OFFSET)
        * Vec3::new(bearing.x, 0.0, bearing.y);
    let body = runtime.local().position;
    let from = body + (offset + Vec3::Y * 1.15).normalize_or_zero() * 40.0;
    let aimed = Transform::from_translation(from).looking_at(body, Vec3::Y);

    let mut placed = false;
    for (_, mut transform) in existing {
        *transform = aimed;
        placed = true;
    }
    if !placed {
        commands.spawn((
            DirectionalLight {
                illuminance: observed_style::iso::light::KEY_ILLUMINANCE,
                shadows_enabled: false,
                ..default()
            },
            aimed,
            OverviewKey,
            DespawnOnExit(crate::GameState::HexWfc),
        ));
    }
}

/// Cut away the resident authored geometry so the overview can see inside it.
///
/// `observed_style::iso::survives` - the studio's own rule, from the shared
/// crate - applied to the geometry residency has already spawned. Until this
/// existed the overview shared the studio's *camera* and none of its
/// *drawing*, which is why one read as a building and the other as a field of
/// blocks.
///
/// Everything is restored when the overview goes down: these are the same
/// entities the first-person view walks through.
pub(in crate::hex_wfc) fn sync_cutaway(
    overview: Res<SpectatorOverview>,
    mut hulls: Query<(&Cutaway, &mut Visibility)>,
) {
    let bearing = overview
        .active
        .then(|| observed_style::iso::detent_bearing(overview.detent));
    for (hull, mut visibility) in &mut hulls {
        let cut = bearing.is_some_and(|bearing| {
            !observed_style::iso::survives(hull.min_y, hull.max_y, hull.local, bearing, true)
        });
        let wanted = if cut {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Count why each hull survived or did not, once a second under the trace.
///
/// `survives` answers yes or no; when nearly everything says no, the useful
/// question is *which test* is saying it. Floors are supposed to survive
/// unconditionally, so a large floor-culled count is the whole answer.
pub(in crate::hex_wfc) fn trace_cutaway(
    time: Res<Time>,
    overview: Res<SpectatorOverview>,
    hulls: Query<&Cutaway>,
) {
    if !overview.active || std::env::var("OBSERVED2_SPECTATE_TRACE").is_err() {
        return;
    }
    if time.elapsed_secs() as u32 == (time.elapsed_secs() - time.delta_secs()) as u32 {
        return;
    }
    // The same thresholds `iso::survives` uses, restated here only to attribute
    // its answer; it is the authority, this is the microscope.
    const FLOOR_TOP: f32 = 0.75;
    const HEAD_CLEARANCE: f32 = 2.2;
    const INTERIOR_RADIUS: f32 = 6.5;
    let bearing = observed_style::iso::detent_bearing(overview.detent);
    let (mut floor, mut ceiling, mut interior, mut near, mut far) = (0, 0, 0, 0, 0);
    let (mut lowest, mut highest) = (f32::INFINITY, f32::NEG_INFINITY);
    for hull in &hulls {
        lowest = lowest.min(hull.min_y);
        highest = highest.max(hull.max_y);
        if hull.max_y <= FLOOR_TOP {
            floor += 1;
        } else if hull.min_y >= HEAD_CLEARANCE {
            ceiling += 1;
        } else if Vec2::new(hull.local.x, hull.local.z).length() < INTERIOR_RADIUS {
            interior += 1;
        } else if Vec2::new(hull.local.x, hull.local.z)
            .normalize_or_zero()
            .dot(bearing)
            > 0.0
        {
            near += 1;
        } else {
            far += 1;
        }
    }
    info!(
        "cutaway: floor={floor} ceiling={ceiling} interior={interior} near={near} far={far}          min_y={lowest:.2} max_y={highest:.2}"
    );
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

/// Where the overview camera stands, and how wide it sees.
///
/// The whole facility framed orthographically from one of six detents - the
/// same reading `composition_studio` uses, from the same shared code, so the
/// tool and the game do not teach two different buildings.
#[must_use]
pub(in crate::hex_wfc) fn framing(
    world: &observed_facility::hex_wfc::HexWfcWorld,
    detent: usize,
) -> Option<observed_style::iso::IsoFraming> {
    framing_fitted(world, detent, FRAME_WIDTH, FRAME_HEIGHT)
}

/// [`framing`] fitted to a specific viewport.
///
/// Only `units_per_pixel` depends on the viewport - the camera's position,
/// rotation and far plane do not - so callers that just need the pose can use
/// [`framing`] and ignore the size.
#[must_use]
pub(in crate::hex_wfc) fn framing_fitted(
    world: &observed_facility::hex_wfc::HexWfcWorld,
    detent: usize,
    width: f32,
    height: f32,
) -> Option<observed_style::iso::IsoFraming> {
    let (min, max) = bounds(world)?;
    Some(observed_style::iso::frame(min, max, detent, width, height))
}

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
    Some(observed_style::iso::IsoFraming {
        // Centred on the body itself, not on a box clamped to the facility -
        // clamping kept the box the same size but slid its centre to the edge,
        // which is why the body sat off in a corner of the frame.
        translation: body + tight.rotation * Vec3::Z * reach_all,
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

// There is no layer focus here any more. It existed to stop ten levels of
// *massing* stacking into a thicket; with only the radius drawn there is
// nothing to thin, and hiding the floor above would cut the top off the very
// stair the spectator is climbing. `observed_style::iso::Layer` is still the
// studio's, where a plan view of a whole floor does need it.

/// How fast the camera should ease toward [`framing`].
#[must_use]
pub(in crate::hex_wfc) fn response() -> f32 {
    RESPONSE
}

/// Drop everything this module owns. Called on leaving the state.
pub(in crate::hex_wfc) fn clear(mut overview: ResMut<SpectatorOverview>) {
    overview.active = false;
    overview.built_generation = None;
}
