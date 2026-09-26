//! How the desk answers the Architect: the board moving, and the desk's sounds.
//!
//! - **A tile builds in.** When a room's placement changes on the board, the room rises
//!   the last few metres into place under a glow that fades: amber for a tile this
//!   Architect built, with the facility's own reroute sound, and cyan for a room the team
//!   has found changed since it last saw it. A room drawn again only because the team
//!   now sees it, or sees it no longer, does not build in: nothing about it changed.
//! - **Trouble pulses.** A contradiction's ring breathes red, and the aim's amber ring
//!   breathes while the play waits to be confirmed.
//! - **The desk clicks.** A card picked up clicks, an aim ticks, a play the rules refuse
//!   lands with a dull knock, and a teammate's new request calls, low and long.

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use observed_facility::hex_wfc::HexPlacement;
use observed_style::architect::{Role, color};

use super::ArchitectDesk;
use super::pick::BOARD_ORIGIN;
use crate::GameState;

/// How far below its place a room starts, and how long it takes to build in.
const RISE: f32 = 2.4;
const BUILD_SECONDS: f32 = 1.3;
/// The glow's opacity as a room starts to build in.
const GLOW: f32 = 0.7;
/// Breaths a second, for a pulsing ring.
const PULSE_RATE: f32 = 1.1;

/// A room building in: its age, and the glow laid over it.
#[derive(Component)]
pub(in crate::hex_wfc) struct BuildIn {
    pub age: f32,
    pub glow: Handle<StandardMaterial>,
    pub glows: Vec<Entity>,
    /// Whether this Architect built it, which is what the sound answers.
    pub own: bool,
}

/// A mark that breathes, in `role`'s colour, swelling across from its own `scale`.
#[derive(Component)]
pub(super) struct Pulse {
    pub role: Role,
    pub scale: Vec3,
}

impl Pulse {
    /// A ring at its own size.
    pub(super) const fn ring(role: Role) -> Self {
        Self {
            role,
            scale: Vec3::ONE,
        }
    }
}

/// Whether a room drawn now as `now` builds in, and in what colour: amber for the tile
/// this Architect built (`own`, not yet `celebrated`), cyan for a room that was drawn
/// `before` as something else, and not at all otherwise.
#[must_use]
pub(super) fn build_glow(
    own: Option<HexPlacement>,
    celebrated: bool,
    before: Option<HexPlacement>,
    now: HexPlacement,
) -> Option<Role> {
    if own == Some(now) && !celebrated {
        Some(Role::Selected)
    } else if before.is_some_and(|before| before != now) {
        Some(Role::Observer)
    } else {
        None
    }
}

/// The glow's material, in `role`'s colour, at its brightest.
#[must_use]
pub(super) fn glow_material(role: Role) -> StandardMaterial {
    StandardMaterial {
        base_color: color(role).with_alpha(GLOW),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    }
}

/// Where a room building in stands, `t` (0..=1) of the way through: below its place,
/// easing out as it arrives.
#[must_use]
pub(super) fn rise(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    -RISE * (1.0 - t).powi(3)
}

/// The glow's opacity `t` (0..=1) of the way through.
#[must_use]
pub(super) fn glow_alpha(t: f32) -> f32 {
    GLOW * (1.0 - t.clamp(0.0, 1.0)).powi(2)
}

/// A breath, 0..=1, `seconds` in.
#[must_use]
pub(super) fn breath(seconds: f32) -> f32 {
    0.5 - 0.5 * (seconds * PULSE_RATE * std::f32::consts::TAU).cos()
}

/// Where a room building in starts.
#[must_use]
pub(super) fn start() -> Transform {
    Transform::from_translation(BOARD_ORIGIN + Vec3::Y * rise(0.0))
}

pub(super) fn build_in(
    mut commands: Commands,
    time: Res<Time>,
    mut rooms: Query<(Entity, &mut BuildIn, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut building, mut transform) in &mut rooms {
        building.age += time.delta_secs();
        let t = building.age / BUILD_SECONDS;
        transform.translation = BOARD_ORIGIN + Vec3::Y * rise(t);
        if let Some(mut glow) = materials.get_mut(&building.glow) {
            glow.base_color.set_alpha(glow_alpha(t));
        }
        if t >= 1.0 {
            for glow in building.glows.drain(..) {
                commands.entity(glow).despawn();
            }
            commands.entity(entity).remove::<BuildIn>();
        }
    }
}

pub(super) fn pulse(
    time: Res<Time>,
    mut rings: Query<(&Pulse, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let breath = breath(time.elapsed_secs());
    for (pulse, mut transform, material) in &mut rings {
        transform.scale = pulse.scale * Vec3::new(1.0 + 0.1 * breath, 1.0, 1.0 + 0.1 * breath);
        if let Some(mut material) = materials.get_mut(&material.0) {
            let base = color(pulse.role).to_linear();
            material.base_color = Color::LinearRgba(base * (0.6 + 0.6 * breath)).with_alpha(1.0);
        }
    }
}

/// The desk's sounds.
#[derive(Resource)]
pub(super) struct DeskSounds {
    click: Handle<AudioSource>,
    tick: Handle<AudioSource>,
    build: Handle<AudioSource>,
}

pub(super) fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    sounds: Option<Res<DeskSounds>>,
) {
    if sounds.is_none() {
        commands.insert_resource(DeskSounds {
            click: assets.load(observed_assets::UI_CLICK.path),
            tick: assets.load(observed_assets::UI_HOVER.path),
            build: assets.load(observed_assets::REROUTE.path),
        });
    }
}

/// What the desk was doing last frame, to hear what changed.
#[derive(Default)]
pub(super) struct Heard {
    selected: Option<usize>,
    aimed: Option<observed_hex::HexCoord>,
    refused: bool,
    /// The team's requests, by author and when made.
    requests: std::collections::BTreeSet<(observed_core::PlayerId, u64)>,
}

pub(super) fn sounds(
    mut commands: Commands,
    desk: Res<ArchitectDesk>,
    sounds: Res<DeskSounds>,
    settings: Res<crate::settings::Settings>,
    built: Query<&BuildIn, Added<BuildIn>>,
    runtime: Res<crate::hex_wfc::sim::HexWfcRuntime>,
    mut heard: Local<Heard>,
) {
    let volume = settings.effective_sfx_volume();
    let mut play = |source: &Handle<AudioSource>, gain: f32, speed: f32| {
        if volume * gain > 0.0 {
            commands.spawn((
                DespawnOnExit(GameState::HexWfc),
                AudioPlayer(source.clone()),
                PlaybackSettings {
                    mode: PlaybackMode::Despawn,
                    volume: Volume::Linear(volume * gain),
                    speed,
                    ..PlaybackSettings::DESPAWN
                },
                Name::new("Architect desk sound"),
            ));
        }
    };
    if desk.selected != heard.selected && desk.selected.is_some() {
        play(&sounds.click, 0.5, 1.0);
    }
    if desk.aimed != heard.aimed && desk.aimed.is_some() {
        play(&sounds.tick, 0.45, 1.2);
    }
    let refused = desk.last_refusal.is_some();
    if refused && !heard.refused {
        // Slowed, the click is a knock: the rules would not take it.
        play(&sounds.click, 0.7, 0.55);
    }
    for building in &built {
        play(&sounds.build, if building.own { 0.7 } else { 0.35 }, 1.0);
    }
    // A teammate asking: the tick, twice as long, so it is heard as a call.
    let requests: std::collections::BTreeSet<_> = runtime
        .ascent
        .as_ref()
        .map(|ascent| {
            super::requests::team_requests(ascent.session(), desk.team)
                .into_iter()
                .map(|request| (request.author, request.created_at))
                .collect()
        })
        .unwrap_or_default();
    if requests
        .iter()
        .any(|request| !heard.requests.contains(request))
    {
        play(&sounds.tick, 0.7, 0.6);
    }
    *heard = Heard {
        selected: desk.selected,
        aimed: desk.aimed,
        refused,
        requests,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_facility::hex_wfc::{HexCoord, authored_hall};

    fn hall(doors: u8) -> HexPlacement {
        authored_hall(
            HexCoord {
                q: 1,
                r: 1,
                level: 0,
            },
            doors,
        )
        .expect("a hall")
    }

    #[test]
    fn only_a_changed_placement_builds_in() {
        let (a, b) = (hall(0b00_1001), hall(0b01_0101));
        assert_eq!(
            build_glow(Some(a), false, None, a),
            Some(Role::Selected),
            "a tile this Architect built, drawn for the first time"
        );
        assert_eq!(
            build_glow(Some(a), false, Some(b), a),
            Some(Role::Selected),
            "a tile this Architect built over what was there"
        );
        assert_eq!(
            build_glow(Some(a), true, Some(a), a),
            None,
            "already celebrated, and drawn again only for its tone"
        );
        assert_eq!(
            build_glow(None, false, Some(b), a),
            Some(Role::Observer),
            "the team found it changed"
        );
        assert_eq!(build_glow(None, false, Some(a), a), None, "only its tone");
        assert_eq!(build_glow(None, false, None, a), None, "newly mapped");
        assert_eq!(
            build_glow(Some(b), false, Some(b), a),
            Some(Role::Observer),
            "the team has seen a rival rebuild this Architect's tile"
        );
    }

    #[test]
    fn a_room_rises_into_place_as_its_glow_fades() {
        assert!((rise(0.0) + RISE).abs() < 1e-5);
        assert!(rise(0.5) > rise(0.2));
        assert!(rise(1.0).abs() < 1e-6);
        assert!(rise(3.0).abs() < 1e-6, "and stays there");
        assert!((glow_alpha(0.0) - GLOW).abs() < 1e-6);
        assert!(glow_alpha(0.5) < glow_alpha(0.1));
        assert!(glow_alpha(1.0).abs() < 1e-6);
    }

    #[test]
    fn a_breath_goes_all_the_way_in_and_out() {
        assert!(breath(0.0).abs() < 1e-6);
        let top = breath(0.5 / PULSE_RATE);
        assert!((top - 1.0).abs() < 1e-5);
        assert!((0.0..=1.0).contains(&breath(0.37)));
    }
}
