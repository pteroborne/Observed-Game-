//! The major Guardian, drawn as the Tumbler (`observed_guardian`), and heard.
//!
//! The simulation owns where it is and whether it is hunting, frozen by someone who
//! sees it, or frozen by an anchor lantern. This module only draws and sounds those
//! states, and never writes anything back.
//!
//! - **Hunting:** the tiers turn, the eye scans its target, and movement scrapes like stone.
//! - **Seen:** the tiers snap into line and the latch drops home. Stone friction stops.
//! - **Anchored:** the same, with the anchor's purple clamped round its base and the
//!   clamp in the lantern's dark glass voice.
//! - **Let go:** the mass takes up its weight and stone friction follows its motion.
//! - **A catch:** the simulation sends the Guardian home in the same tick as the catch,
//!   so the catch plays as its own short-lived Tumbler where the Guardian was last
//!   drawn. Its sound is the event cue, played from the catch's cell (`audio`).
//!
//! The simulation moves the Guardian a whole cell at a time. The drawn Guardian glides
//! there instead, and snaps only after a catch, when the simulation has sent it home.
use bevy::audio::{PlaybackMode, SpatialScale, Volume};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use observed_guardian::form::{self, CATCH_SECONDS, Form, Look, Pose, Stage, State};
use observed_guardian::mesh::mesh;
use observed_match::hex_wfc::{HexGuardianStatus, HexMatchEventKind};
use observed_style::guardian::{self as style, Part as Finish};
use observed_style::{MarkerRole, marker};

use super::sim::{EYE_OFFSET, HexWfcRuntime};
use crate::GameState;

pub(super) mod released;

/// The major Guardian: the Tumbler at four tiers.
pub(super) const FORM: Form = Form::Tumbler { tiers: 4 };
/// A jump longer than this is a teleport (a catch sending it home), drawn as one.
pub(super) const SNAP: f32 = 30.0;
/// The hum's share of the effects volume, and how fast it follows the state.
const HUM_VOLUME: f32 = 0.7778;
const HUM_FADE: f32 = 14.0;
const ONE_SHOT_VOLUME: f32 = 0.9899;
/// The same distance shrink the event cues use (`audio::HEX_SPATIAL_SCALE`).
const SPATIAL_SCALE: f32 = 0.15;
/// The red light the eye throws: hunting, and held still.
const LIGHT_HUNTING: f32 = 1_000.0;
const LIGHT_FROZEN: f32 = 180.0;

#[derive(Resource)]
pub(super) struct GuardianArt {
    parts: Vec<(Handle<Mesh>, Look)>,
    shell: Handle<StandardMaterial>,
    trim: Handle<StandardMaterial>,
    pupil: Handle<StandardMaterial>,
    eye: Handle<StandardMaterial>,
    eye_flare: Handle<StandardMaterial>,
    seam: Handle<StandardMaterial>,
    seam_dark: Handle<StandardMaterial>,
    seam_flare: Handle<StandardMaterial>,
    beam: Handle<StandardMaterial>,
    clamp: Handle<StandardMaterial>,
    stamp: Handle<StandardMaterial>,
    hum: Handle<AudioSource>,
    latch: Handle<AudioSource>,
    release: Handle<AudioSource>,
    clamp_sound: Handle<AudioSource>,
}

/// What is being drawn, which may lag the simulation by a glide.
#[derive(Resource)]
pub(super) struct GuardianPresentation {
    state: State,
    /// Clock time the state began.
    since: f32,
    /// Where the Guardian is drawn standing, on the floor.
    at: Option<Vec3>,
    last_event_tick: u64,
    hum: f32,
}

/// The live Guardian's root; its parts are children.
#[derive(Component)]
pub(super) struct HexGuardianVisual;

/// A catch playing out where the Guardian was, then gone.
#[derive(Component)]
pub(super) struct CatchEffect {
    started: f32,
    at: Vec3,
    toward: Vec3,
}

#[derive(Component)]
pub(super) struct GuardianPart {
    pub(super) index: usize,
    pub(super) look: Look,
}

#[derive(Component)]
pub(super) struct GuardianHum;

#[derive(Component)]
pub(super) struct GuardianEyeLight;

/// A Tumbler's parts, posed each frame.
pub(super) type Parts<'w, 's> = Query<
    'w,
    's,
    (
        &'static GuardianPart,
        &'static mut Transform,
        &'static mut Visibility,
        &'static mut MeshMaterial3d<StandardMaterial>,
    ),
>;
/// The light the eye throws.
type EyeLight<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut PointLight),
    (With<GuardianEyeLight>, Without<GuardianPart>),
>;
/// The hum, which goes where the eye goes.
type Hum<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        Option<&'static mut SpatialAudioSink>,
    ),
    (
        With<GuardianHum>,
        Without<GuardianPart>,
        Without<GuardianEyeLight>,
    ),
>;

fn finish(part: Finish) -> StandardMaterial {
    let f = style::finish(part);
    StandardMaterial {
        base_color: f.base_color,
        emissive: f.emissive,
        metallic: f.metallic,
        perceptual_roughness: f.roughness,
        ..default()
    }
}

fn glow(emissive: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.02, 0.01, 0.01),
        emissive,
        ..default()
    }
}

fn haze(color: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::LinearRgba(color),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        ..default()
    }
}

pub(super) fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut eye_flare = finish(Finish::Eye);
    eye_flare.emissive = style::catch_flare();
    let control = marker(MarkerRole::Control);
    let art = GuardianArt {
        parts: form::parts(FORM)
            .into_iter()
            .map(|part| (meshes.add(mesh(part.shape)), part.look))
            .collect(),
        shell: materials.add(finish(Finish::Shell)),
        trim: materials.add(finish(Finish::Trim)),
        pupil: materials.add(finish(Finish::Pupil)),
        eye: materials.add(finish(Finish::Eye)),
        eye_flare: materials.add(eye_flare),
        seam: materials.add(glow(style::seam(false))),
        seam_dark: materials.add(glow(style::seam(true))),
        seam_flare: materials.add(glow(style::catch_flare())),
        beam: materials.add(haze(style::beam())),
        clamp: materials.add(StandardMaterial {
            base_color: control.base_color,
            emissive: style::anchor_clamp(),
            ..default()
        }),
        stamp: materials.add(haze(style::catch_flare() * 0.12)),
        hum: asset_server.load(observed_assets::GUARDIAN_HUM.path),
        latch: asset_server.load(observed_assets::GUARDIAN_LATCH.path),
        release: asset_server.load(observed_assets::GUARDIAN_RELEASE.path),
        clamp_sound: asset_server.load(observed_assets::GUARDIAN_CLAMP.path),
    };
    let root = spawn_tumbler(&mut commands, &art);
    commands
        .entity(root)
        .insert((HexGuardianVisual, Name::new("Major Guardian (Tumbler)")));
    commands.entity(root).with_children(|root| {
        root.spawn((
            GuardianEyeLight,
            PointLight {
                color: marker(MarkerRole::Collapse).base_color,
                intensity: LIGHT_HUNTING,
                range: 7.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::IDENTITY,
        ));
        // Always playing, and faded to silence while frozen, so being let go does not
        // restart it from the top.
        root.spawn((
            GuardianHum,
            AudioPlayer::<AudioSource>(art.hum.clone()),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::Linear(0.0),
                spatial: true,
                spatial_scale: Some(SpatialScale::new(SPATIAL_SCALE)),
                ..default()
            },
            Transform::IDENTITY,
        ));
    });
    commands.insert_resource(released::ReleasedArt::new(&mut meshes, &asset_server));
    commands.insert_resource(art);
    commands.insert_resource(GuardianPresentation {
        state: State::Hunting,
        since: 0.0,
        at: None,
        last_event_tick: 0,
        hum: 0.0,
    });
}

/// A Tumbler's root with every part as a child, hidden until posed.
pub(super) fn spawn_tumbler(commands: &mut Commands, art: &GuardianArt) -> Entity {
    commands
        .spawn((
            DespawnOnExit(GameState::HexWfc),
            Transform::IDENTITY,
            Visibility::Visible,
        ))
        .with_children(|root| {
            for (index, (mesh, look)) in art.parts.iter().enumerate() {
                let mut part = root.spawn((
                    GuardianPart { index, look: *look },
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material(art, *look, false, 0.0)),
                    Transform::IDENTITY,
                    Visibility::Hidden,
                ));
                if matches!(look, Look::Beam | Look::Stamp) {
                    part.insert((NotShadowCaster, NotShadowReceiver));
                }
            }
        })
        .id()
}

pub(super) fn material(
    art: &GuardianArt,
    look: Look,
    seams_dark: bool,
    flare: f32,
) -> Handle<StandardMaterial> {
    match look {
        Look::Shell => art.shell.clone(),
        Look::Trim => art.trim.clone(),
        Look::Pupil => art.pupil.clone(),
        Look::Beam => art.beam.clone(),
        Look::Clamp => art.clamp.clone(),
        Look::Stamp => art.stamp.clone(),
        Look::Seam if flare > 0.05 => art.seam_flare.clone(),
        Look::Seam if seams_dark => art.seam_dark.clone(),
        Look::Seam => art.seam.clone(),
        Look::Eye if flare > 0.05 => art.eye_flare.clone(),
        Look::Eye => art.eye.clone(),
    }
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<GuardianArt>();
    commands.remove_resource::<GuardianPresentation>();
    commands.remove_resource::<released::ReleasedArt>();
}

/// Every Guardian drawn each frame: the match's own, its catches, and those released
/// since.
pub(super) fn systems() -> impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, ()> {
    (sync, play_catches, released::sync).chain()
}

/// The drawn state for the simulation's status.
#[must_use]
pub(super) const fn state_for(status: HexGuardianStatus) -> State {
    match status {
        HexGuardianStatus::Active => State::Hunting,
        HexGuardianStatus::FrozenByPlayer => State::FrozenBySight,
        HexGuardianStatus::FrozenByAnchor => State::FrozenByAnchor,
    }
}

/// The sound of going from one state to another, if it has one.
#[must_use]
pub(super) const fn transition(from: State, to: State) -> Option<Transition> {
    match (from.frozen(), to) {
        (false, State::FrozenBySight) => Some(Transition::Latch),
        (_, State::FrozenByAnchor) if !matches!(from, State::FrozenByAnchor) => {
            Some(Transition::Clamp)
        }
        (true, State::Hunting) => Some(Transition::Release),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Transition {
    Latch,
    Clamp,
    Release,
}

/// Follow the simulation's Guardian: its state, its sounds, where it is drawn.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync(
    mut commands: Commands,
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    settings: Res<crate::settings::Settings>,
    art: Res<GuardianArt>,
    mut shown: ResMut<GuardianPresentation>,
    roots: Query<&Children, With<HexGuardianVisual>>,
    mut parts: Parts,
    mut light: EyeLight,
    mut hum: Hum,
) {
    let clock = time.elapsed_secs();
    let guardian = &runtime.match_state.guardian;
    let state = state_for(guardian.status);
    let floor = guardian.feet();
    let volume = settings.effective_sfx_volume();

    // A catch plays where the Guardian was drawn; then it is drawn at home.
    let tick = runtime.match_state.tick;
    if tick != shown.last_event_tick {
        shown.last_event_tick = tick;
        for event in &runtime.match_state.recent_events {
            if event.kind == HexMatchEventKind::GuardianCatch
                && let Some(at) = shown.at
            {
                let root = spawn_tumbler(&mut commands, &art);
                let toward = at + Vec3::Z + Vec3::Y * EYE_OFFSET;
                commands.entity(root).insert((
                    CatchEffect {
                        started: clock,
                        at,
                        toward,
                    },
                    Name::new("Major Guardian catch"),
                ));
                shown.at = Some(floor);
            }
        }
    }

    if state != shown.state {
        if let Some(at) = shown.at {
            let source = match transition(shown.state, state) {
                Some(Transition::Latch) => Some(art.latch.clone()),
                Some(Transition::Clamp) => Some(art.clamp_sound.clone()),
                Some(Transition::Release) => Some(art.release.clone()),
                None => None,
            };
            if let Some(source) = source {
                one_shot(
                    &mut commands,
                    source,
                    ONE_SHOT_VOLUME * volume,
                    at + Vec3::Y * 2.5,
                );
            }
        }
        shown.state = state;
        shown.since = clock;
    }

    let before = shown.at;
    let at = drawn_position(before, floor, state, time.delta_secs());
    shown.at = Some(at);

    // It looks at whoever it is after, or at whoever is watching.
    let toward = guardian
        .target
        .and_then(|id| runtime.match_state.players.get(&id))
        .unwrap_or_else(|| runtime.viewed())
        .position
        + Vec3::Y * EYE_OFFSET;
    let pose = major_pose(state, clock - shown.since, clock, Stage { at, toward });
    if let Ok(children) = roots.single() {
        apply(&art, &pose, children, &mut parts);
    }

    let eye = eye_position(&pose).unwrap_or(at + Vec3::Y * 2.5);
    if let Ok((mut transform, mut light)) = light.single_mut() {
        transform.translation = eye;
        light.intensity = if state == State::Hunting {
            LIGHT_HUNTING
        } else {
            LIGHT_FROZEN
        };
    }
    let wanted = slide_gain(state, before, at, time.delta_secs());
    shown.hum += (wanted - shown.hum) * (HUM_FADE * time.delta_secs()).min(1.0);
    if let Ok((mut transform, sink)) = hum.single_mut() {
        transform.translation = eye;
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(shown.hum * HUM_VOLUME * volume));
        }
    }
}

/// Never invent a second threat position; frozen and hunting models share sight's pose.
pub(super) fn drawn_position(
    _previous: Option<Vec3>,
    floor: Vec3,
    _state: State,
    _dt: f32,
) -> Vec3 {
    floor
}

/// Stone friction follows visible movement; idle, frozen and teleported bodies are quiet.
pub(super) fn slide_gain(state: State, before: Option<Vec3>, at: Vec3, dt: f32) -> f32 {
    let Some(before) = before else {
        return 0.0;
    };
    let distance = before.distance(at);
    if state != State::Hunting || dt <= 0.0 || !(0.001..SNAP).contains(&distance) {
        return 0.0;
    }
    0.4 + 0.6 * (distance / (dt * 7.0)).min(1.0)
}

/// Play the catches out, and clear them away.
pub(super) fn play_catches(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<GuardianArt>,
    effects: Query<(Entity, &CatchEffect, &Children)>,
    mut parts: Parts,
) {
    let clock = time.elapsed_secs();
    for (entity, effect, children) in &effects {
        let t = clock - effect.started;
        if t > CATCH_SECONDS {
            commands.entity(entity).despawn();
            continue;
        }
        let pose = major_pose(
            State::Catch,
            t,
            clock,
            Stage {
                at: effect.at,
                toward: effect.toward,
            },
        );
        apply(&art, &pose, children, &mut parts);
    }
}

pub(super) fn apply(art: &GuardianArt, pose: &Pose, children: &Children, parts: &mut Parts) {
    for child in children.iter() {
        let Ok((part, mut transform, mut visibility, mut current)) = parts.get_mut(child) else {
            continue;
        };
        match pose.parts.get(part.index).copied().flatten() {
            Some(at) => {
                *transform = at;
                *visibility = Visibility::Inherited;
            }
            None => *visibility = Visibility::Hidden,
        }
        if matches!(part.look, Look::Seam | Look::Eye) {
            let wanted = material(art, part.look, pose.seams_dark, pose.flare);
            if current.0 != wanted {
                current.0 = wanted;
            }
        }
    }
}

pub(super) fn major_pose(state: State, t: f32, clock: f32, stage: Stage) -> Pose {
    let mut pose = form::pose(FORM, state, t, clock, stage);
    for transform in pose.parts.iter_mut().flatten() {
        transform.translation = stage.at
            + (transform.translation - stage.at) * observed_match::hex_wfc::MAJOR_MODEL_SCALE;
        transform.scale *= observed_match::hex_wfc::MAJOR_MODEL_SCALE;
    }
    pose
}

/// Where the eye is in `pose`: the first part drawn as one.
fn eye_position(pose: &Pose) -> Option<Vec3> {
    let parts = form::parts(FORM);
    parts
        .iter()
        .zip(&pose.parts)
        .find(|(part, _)| part.look == Look::Eye)
        .and_then(|(_, at)| at.map(|at| at.translation))
}

fn one_shot(commands: &mut Commands, source: Handle<AudioSource>, volume: f32, at: Vec3) {
    if volume <= 0.0 {
        return;
    }
    commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        AudioPlayer(source),
        PlaybackSettings {
            mode: PlaybackMode::Despawn,
            volume: Volume::Linear(volume),
            spatial: true,
            spatial_scale: Some(SpatialScale::new(SPATIAL_SCALE)),
            ..default()
        },
        Transform::from_translation(at),
        Name::new("Major Guardian cue"),
    ));
}

#[cfg(test)]
mod tests;
