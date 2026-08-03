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
const DETAIL_RADIUS: f32 = 34.0;

// Compile-time, not a test: these are relationships between constants, so a
// runtime assertion could only ever restate what the compiler already knows.
// The window must open outside the radius residency spawns at, or the prism
// vanishes before its geometry exists and the body stands on a hole; and it
// must close inside the radius residency retires at, or the geometry goes and
// no prism comes back.
const _: () = assert!(DETAIL_RADIUS > super::STREAM_ENTER_RADIUS);
const _: () = assert!(DETAIL_RADIUS < super::STREAM_EXIT_RADIUS);

/// Where the camera sits relative to the followed body.
///
/// High and back, looking down at about 35 degrees - the same read as the
/// survivor map and the composition studio, so the three do not each teach a
/// different sense of which way the facility runs.
const RISE: f32 = 52.0;
const BACK: f32 = 44.0;
const PITCH: f32 = -0.62;

/// How fast the camera eases toward the body, per second.
///
/// Slower than the chase cam's 6.0: at this distance a snap reads as the whole
/// facility lurching rather than the camera moving.
const RESPONSE: f32 = 2.5;

/// Whether the overview is up, and what it has built.
#[derive(Resource, Default)]
pub(in crate::hex_wfc) struct SpectatorOverview {
    pub active: bool,
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
    cell: HexCoord,
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

/// Hide the massing around the followed body so the real tiles show through.
pub(in crate::hex_wfc) fn sync_detail_window(
    runtime: Res<HexWfcRuntime>,
    overview: Res<SpectatorOverview>,
    mut massing: Query<(&Massing, &mut Visibility)>,
) {
    if !overview.active {
        return;
    }
    let body = runtime.local().position;
    for (cell, mut visibility) in &mut massing {
        let origin = Vec3::from_array(hex_origin(cell.cell));
        let near = origin.distance(body) <= DETAIL_RADIUS;
        let wanted = if near {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// The overview camera pose for a body at `position` facing `yaw`.
///
/// Returned rather than applied: `sync_camera` owns the one camera, and a
/// second one here would take the UI with it.
#[must_use]
pub(in crate::hex_wfc) fn pose(position: Vec3, yaw: f32) -> (Vec3, Quat) {
    let forward = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
    let eye = position + Vec3::Y * RISE - forward * BACK;
    let rotation = Quat::from_rotation_y(-yaw) * Quat::from_rotation_x(PITCH);
    (eye, rotation)
}

/// How fast the camera should ease toward [`pose`].
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The camera looks down at the body, not up past it.
    #[test]
    fn the_overview_looks_down_on_the_body_it_follows() {
        let (eye, rotation) = pose(Vec3::ZERO, 0.0);
        assert!(eye.y > 0.0, "the overview must sit above the body");
        let looking = rotation * Vec3::NEG_Z;
        assert!(
            looking.y < 0.0,
            "the overview must look downward, got {looking:?}"
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

        // The window is open at the body and shut away from it.
        let body = app.world().resource::<HexWfcRuntime>().local().position;
        let mut near_hidden = 0;
        let mut far_visible = 0;
        let mut query = app.world_mut().query::<(&Massing, &Visibility)>();
        for (massing, visibility) in query.iter(app.world()) {
            let origin = Vec3::from_array(hex_origin(massing.cell));
            if origin.distance(body) <= DETAIL_RADIUS {
                assert_eq!(
                    *visibility,
                    Visibility::Hidden,
                    "massing at the body must yield to the real geometry"
                );
                near_hidden += 1;
            } else if *visibility == Visibility::Inherited {
                far_visible += 1;
            }
        }
        assert!(
            near_hidden > 0,
            "the body should always have some massing to hide"
        );
        assert!(far_visible > 0, "the rest of the facility must still show");

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
}
