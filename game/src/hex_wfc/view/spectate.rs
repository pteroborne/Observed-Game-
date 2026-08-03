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
use observed_facility::hex_wfc::HexCoord;
use observed_hex::hex_origin;

use super::map::cell::sketch;
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
    /// The facility generation the massing was built from. Relayout changes
    /// placements, so massing built before it is a picture of a facility that
    /// no longer exists.
    built_generation: Option<u32>,
}

/// One cell's massing prism.
///
/// `pub(in ...)` only because the systems that query it are named in the
/// schedule, which makes their parameter types part of that signature.
#[derive(Component)]
pub(in crate::hex_wfc) struct Massing {
    pub(in crate::hex_wfc) cell: HexCoord,
}

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

/// Build the massing when the overview comes up; drop it when it goes down.
pub(in crate::hex_wfc) fn sync_massing(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    spectating: Option<Res<crate::sim::state::SpectatorBot>>,
    mut overview: ResMut<SpectatorOverview>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<Massing>>,
) {
    let world = &runtime.match_state.facility;
    let wanted = overview.active && spectating.is_some();
    let current = overview.built_generation;
    if wanted && current == Some(world.generation) {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    if !wanted {
        overview.built_generation = None;
        return;
    }

    // One mesh per height and one material per district, so 5,600 prisms are a
    // handful of draw calls rather than 5,600. The map builder caches the same
    // way and for the same reason.
    let mut prisms: std::collections::BTreeMap<u32, Handle<Mesh>> =
        std::collections::BTreeMap::new();
    let mut tints: std::collections::BTreeMap<u8, Handle<StandardMaterial>> =
        std::collections::BTreeMap::new();

    for (&cell, placement) in &world.placements {
        let in_blueprint = world.room_id_at(cell).is_some();
        let drawn = sketch(placement.archetype, placement.space, in_blueprint);
        let Some(height) = drawn.height else {
            continue;
        };
        let Some(register) = world.architecture.get(&cell) else {
            continue;
        };

        let mesh = prisms
            .entry(height.to_bits())
            .or_insert_with(|| {
                meshes.add(Cuboid::new(drawn.inset * 12.0, height, drawn.inset * 12.0))
            })
            .clone();
        let material = tints
            .entry(*register as u8)
            .or_insert_with(|| {
                // The district's own accent, from the shared palette - the
                // same one the survivor map tints with. The Legibility
                // Contract forbids inventing a colour here, and a second
                // palette would make the overview and the map disagree about
                // which district you are looking at.
                let accent = observed_style::architecture(*register).accent;
                materials.add(StandardMaterial {
                    base_color: Color::LinearRgba(accent),
                    emissive: accent * 0.14,
                    perceptual_roughness: 0.95,
                    ..default()
                })
            })
            .clone();

        let origin = Vec3::from_array(hex_origin(cell));
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(origin + Vec3::Y * height * 0.5),
            // Spelled out rather than left to `Mesh3d`'s required components.
            // This module *drives* visibility, so it owns the component; and
            // required components come from the render plugins, which a
            // headless test does not load - relying on them made the window
            // silently do nothing there.
            Visibility::Inherited,
            Massing { cell },
        ));
    }
    overview.built_generation = Some(world.generation);
}

/// Decide what each prism shows, from the floor the followed body is on.
///
/// Three states, in order:
///
/// - **Off the layer entirely.** A ten-level facility seen all at once stacks
///   into a thicket, so only the body's floor and its immediate neighbours are
///   drawn at all - the same rule the studio reads a plan by.
/// - **On the focus floor, near the body.** Residency has already spawned the
///   real authored geometry there, so the prism gets out of its way.
/// - **Everything else drawn.** Massing, which is what carries the shape of the
///   building at this distance.
pub(in crate::hex_wfc) fn sync_detail_window(
    runtime: Res<HexWfcRuntime>,
    overview: Res<SpectatorOverview>,
    mut massing: Query<(&Massing, &mut Visibility)>,
) {
    if !overview.active {
        return;
    }
    let body = runtime.local();
    let layer = layer_for(body.cell.level);
    for (cell, mut visibility) in &mut massing {
        let origin = Vec3::from_array(hex_origin(cell.cell));
        let near = origin.distance(body.position) <= DETAIL_RADIUS;
        let wanted = if !layer.draws(cell.cell.level) || (layer.is_focus(cell.cell.level) && near) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
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
    let (min, max) = bounds(world)?;
    Some(observed_style::iso::frame(
        min,
        max,
        detent,
        FRAME_WIDTH,
        FRAME_HEIGHT,
    ))
}

/// Which floor is under inspection: the one the followed body is standing on.
///
/// This is what "follows the selected player" means at facility scale. The
/// camera frames the whole building and does not chase; what tracks the body is
/// the *floor*, so walking up a stair changes which storey is solid.
#[must_use]
pub(in crate::hex_wfc) fn layer_for(level: u8) -> observed_style::iso::Layer {
    observed_style::iso::Layer::Single(level)
}

/// How fast the camera should ease toward [`framing`].
#[must_use]
pub(in crate::hex_wfc) fn response() -> f32 {
    RESPONSE
}

/// Drop everything this module owns. Called on leaving the state.
pub(in crate::hex_wfc) fn clear(
    mut commands: Commands,
    mut overview: ResMut<SpectatorOverview>,
    massing: Query<Entity, With<Massing>>,
) {
    for entity in &massing {
        commands.entity(entity).despawn();
    }
    overview.active = false;
    overview.built_generation = None;
}
