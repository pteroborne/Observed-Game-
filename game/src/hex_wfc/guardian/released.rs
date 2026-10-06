//! Guardians released after the match began, drawn where the simulation has them.
//!
//! A requisition's major is the Tumbler again, drawn as the match's own is. A minor is a
//! small Roller: an octahedral cage that walks by tipping face to face, one tip for each
//! stride of ground its body actually covers, so it rolls where it goes rather than on
//! the spot. Minors are never frozen by sight, so a minor is only ever drawn hunting. Its
//! eye is self-lit, which keeps its silhouette legible on an unpowered floor.
//!
//! Presentation only: the simulation owns where each is, and nothing here writes back.
//! Their arrival and their loss are heard through the match's event cues (`cues`).
use bevy::audio::{PlaybackMode, SpatialScale, Volume};
use bevy::prelude::*;
use observed_guardian::form::{self, Look, Stage, State};
use observed_guardian::mesh::mesh;
use observed_guardian::roll::{Rest, Roll};
use observed_hex::{FLOOR_SLAB_TOP, hex_origin};
use observed_match::hex_wfc::{HexReleasedGuardian, HexReleasedKind};

use super::super::sim::{EYE_OFFSET, HexWfcRuntime};
use super::{self as guardian, FORM, GuardianArt, GuardianPart, Parts};
use crate::GameState;

/// A minor's size against the major Roller the form was drawn at: about a metre tall,
/// knee to chest on a body.
const MINOR_SCALE: f32 = 0.7;

/// The Roller's meshes, and how far one tip carries it.
#[derive(Resource)]
pub(super) struct ReleasedArt {
    roller: Vec<(Handle<Mesh>, Look)>,
    /// Plan distance one tip carries the Roller's centre, at the form's own scale.
    stride: f32,
    land: Handle<AudioSource>,
}

/// A released Guardian's persistent voice, faded with its simulation state.
#[derive(Component)]
pub(super) struct ReleasedHum {
    id: u16,
    gain: f32,
}

pub(super) type ReleasedVoices<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut ReleasedHum,
        &'static mut Transform,
        Option<&'static mut SpatialAudioSink>,
    ),
    (Without<ReleasedVisual>, Without<GuardianPart>),
>;

/// One released Guardian as drawn.
#[derive(Component)]
pub(in crate::hex_wfc) struct ReleasedVisual {
    id: u16,
    kind: HexReleasedKind,
    /// Where it is drawn standing, on the floor.
    at: Vec3,
    /// A major's state, and the clock time it began.
    state: State,
    since: f32,
    /// A minor's last landed rest, centred on its own feet, and how far into the next tip
    /// it is, in strides.
    rest: Rest,
    progress: f32,
    heading: Vec3,
}

impl ReleasedArt {
    pub(super) fn new(meshes: &mut Assets<Mesh>, server: &AssetServer) -> Self {
        let start = Rest::on_a_face(Vec3::ZERO);
        Self {
            land: server.load(observed_assets::MINOR_GUARDIAN_STEP.path),
            roller: form::parts(form::Form::Roller)
                .into_iter()
                .map(|part| (meshes.add(mesh(part.shape)), part.look))
                .collect(),
            stride: Roll::toward(start, Vec3::X)
                .at(1.0)
                .centre
                .with_y(0.0)
                .length(),
        }
    }
}

/// Spawn what has been released, draw it where it is, and clear away what is gone.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync(
    mut commands: Commands,
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    guardian_art: Res<GuardianArt>,
    art: Res<ReleasedArt>,
    settings: Res<crate::settings::Settings>,
    mut hums: ReleasedVoices,
    mut visuals: Query<
        (Entity, &mut ReleasedVisual, &mut Transform, &Children),
        Without<GuardianPart>,
    >,
    mut parts: Parts,
) {
    let clock = time.elapsed_secs();
    let released = &runtime.match_state.released;
    let mut drawn = std::collections::BTreeSet::new();
    for (entity, visual, _, _) in &visuals {
        match released.get(&visual.id) {
            Some(guardian) if guardian.kind() == visual.kind => {
                drawn.insert(visual.id);
            }
            _ => commands.entity(entity).despawn(),
        }
    }
    for (&id, guardian) in released {
        if !drawn.contains(&id) {
            spawn(&mut commands, &guardian_art, &art, id, guardian, clock);
        }
    }

    let half_height = runtime
        .match_state
        .content()
        .traversal_profile()
        .requirements()
        .capsule_half_height;
    let mut voices = std::collections::BTreeMap::new();
    for (_, mut visual, mut transform, children) in &mut visuals {
        let Some(guardian) = released.get(&visual.id) else {
            continue;
        };
        match guardian {
            HexReleasedGuardian::Major(major) => {
                let floor = Vec3::new(
                    major.position.x,
                    hex_origin(major.cell)[1] + FLOOR_SLAB_TOP,
                    major.position.z,
                );
                let before = visual.at;
                visual.at = guardian::drawn_position(
                    Some(visual.at),
                    floor,
                    guardian::state_for(major.status),
                    time.delta_secs(),
                );
                let state = guardian::state_for(major.status);
                if state != visual.state {
                    let source = match guardian::transition(visual.state, state) {
                        Some(guardian::Transition::Latch) => Some(guardian_art.latch.clone()),
                        Some(guardian::Transition::Clamp) => Some(guardian_art.clamp_sound.clone()),
                        Some(guardian::Transition::Release) => Some(guardian_art.release.clone()),
                        None => None,
                    };
                    if let Some(source) = source {
                        guardian::one_shot(
                            &mut commands,
                            source,
                            guardian::ONE_SHOT_VOLUME * settings.effective_sfx_volume(),
                            visual.at + Vec3::Y * 2.5,
                        );
                    }
                    visual.state = state;
                    visual.since = clock;
                }
                let toward = major
                    .target
                    .and_then(|player| runtime.match_state.players.get(&player))
                    .unwrap_or_else(|| runtime.viewed())
                    .position
                    + Vec3::Y * EYE_OFFSET;
                let pose = form::pose(
                    FORM,
                    visual.state,
                    clock - visual.since,
                    clock,
                    Stage {
                        at: visual.at,
                        toward,
                    },
                );
                voices.insert(
                    visual.id,
                    (
                        visual.at + Vec3::Y * 2.5,
                        guardian::slide_gain(
                            visual.state,
                            Some(before),
                            visual.at,
                            time.delta_secs(),
                        ) * guardian::HUM_VOLUME,
                    ),
                );
                *transform = Transform::IDENTITY;
                guardian::apply(&guardian_art, &pose, children, &mut parts);
            }
            HexReleasedGuardian::Minor(minor) => {
                // A plumbed minor stands on whatever its plumb made its floor: its feet
                // are down its own frame, and the whole Roller turns with it. It does not
                // roll while a plumb carries it.
                let frame = minor.visual_frame();
                let feet = minor.position - frame.up() * half_height;
                if minor.plumbed() {
                    visual.at = feet;
                } else {
                    if roll(&mut visual, feet, art.stride * MINOR_SCALE) {
                        super::super::audio::play(
                            &mut commands,
                            art.land.clone(),
                            0.45 * settings.effective_sfx_volume(),
                            "Minor Guardian box impact",
                            Some(feet + frame.up() * 0.4),
                        );
                    }
                }
                let mut body = Roll::toward(visual.rest, visual.heading).at(visual.progress);
                // The body carries it along; the roll only turns and lifts it.
                body.centre -= visual.heading * art.stride * visual.progress;
                let toward = minor
                    .target
                    .and_then(|player| runtime.match_state.players.get(&player))
                    .unwrap_or_else(|| runtime.viewed())
                    .position
                    + Vec3::Y * EYE_OFFSET;
                let pose = form::roller_rolling(body, clock, (toward - feet) / MINOR_SCALE);
                *transform = Transform::from_translation(feet)
                    .with_rotation(frame.rotation)
                    .with_scale(Vec3::splat(MINOR_SCALE));
                guardian::apply(&guardian_art, &pose, children, &mut parts);
            }
        }
    }
    for (mut voice, mut transform, sink) in &mut hums {
        let Some(&(at, wanted)) = voices.get(&voice.id) else {
            continue;
        };
        transform.translation = at;
        voice.gain += (wanted - voice.gain) * (guardian::HUM_FADE * time.delta_secs()).min(1.0);
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(voice.gain * settings.effective_sfx_volume()));
        }
    }
}

/// Advance a minor's roll by the ground its feet covered since it was last drawn: a
/// whole tip for every `stride`, landing each on the face it tipped onto.
fn roll(visual: &mut ReleasedVisual, feet: Vec3, stride: f32) -> bool {
    let moved = (feet - visual.at).with_y(0.0);
    visual.at = feet;
    if moved.length() < 1e-4 {
        return false;
    }
    let heading = moved.normalize();
    let mut impact = false;
    // A sharp turn lands the tip in progress first, so the cage never tips two ways at
    // once.
    if heading.dot(visual.heading) < 0.5 && visual.progress > 0.0 {
        visual.rest = landed(Roll::toward(visual.rest, visual.heading).at(1.0));
        visual.progress = 0.0;
        impact = true;
    }
    visual.heading = heading;
    visual.progress += moved.length() / stride;
    while visual.progress >= 1.0 {
        visual.rest = landed(Roll::toward(visual.rest, heading).at(1.0));
        visual.progress -= 1.0;
        impact = true;
    }
    impact
}

/// A landed rest brought back over its own feet, which the body has carried forward.
fn landed(rest: Rest) -> Rest {
    Rest {
        rotation: rest.rotation,
        centre: Vec3::new(0.0, rest.centre.y, 0.0),
    }
}

fn spawn(
    commands: &mut Commands,
    guardian_art: &GuardianArt,
    art: &ReleasedArt,
    id: u16,
    guardian: &HexReleasedGuardian,
    clock: f32,
) {
    let kind = guardian.kind();
    let root = match kind {
        HexReleasedKind::Major => guardian::spawn_tumbler(commands, guardian_art),
        HexReleasedKind::Minor => commands
            .spawn((
                DespawnOnExit(GameState::HexWfc),
                Transform::IDENTITY,
                Visibility::Visible,
            ))
            .with_children(|root| {
                for (index, (mesh, look)) in art.roller.iter().enumerate() {
                    root.spawn((
                        GuardianPart { index, look: *look },
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(guardian::material(guardian_art, *look, false, 0.0)),
                        Transform::IDENTITY,
                        Visibility::Hidden,
                    ));
                }
            })
            .id(),
    };
    if kind == HexReleasedKind::Major {
        commands.entity(root).with_children(|root| {
            root.spawn((
                ReleasedHum { id, gain: 0.0 },
                AudioPlayer(guardian_art.hum.clone()),
                PlaybackSettings {
                    mode: PlaybackMode::Loop,
                    volume: Volume::Linear(0.0),
                    spatial: true,
                    spatial_scale: Some(SpatialScale::new(guardian::SPATIAL_SCALE)),
                    ..default()
                },
                Transform::IDENTITY,
                Name::new("Released major stone friction"),
            ));
        });
    }
    let position = guardian.position();
    commands.entity(root).insert((
        ReleasedVisual {
            id,
            kind,
            at: Vec3::new(
                position.x,
                hex_origin(guardian.cell())[1] + FLOOR_SLAB_TOP,
                position.z,
            ),
            state: State::Hunting,
            since: clock,
            rest: Rest::on_a_face(Vec3::ZERO),
            progress: 0.0,
            heading: Vec3::X,
        },
        Name::new(match kind {
            HexReleasedKind::Major => format!("Released major Guardian {id} (Tumbler)"),
            HexReleasedKind::Minor => format!("Minor Guardian {id} (Roller)"),
        }),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visual() -> ReleasedVisual {
        ReleasedVisual {
            id: 1,
            kind: HexReleasedKind::Minor,
            at: Vec3::ZERO,
            state: State::Hunting,
            since: 0.0,
            rest: Rest::on_a_face(Vec3::ZERO),
            progress: 0.0,
            heading: Vec3::X,
        }
    }

    /// A minor tips once for every stride it walks, and not at all standing still.
    #[test]
    fn a_minor_rolls_as_far_as_it_walks() {
        let stride = 1.0;
        let mut minor = visual();
        assert!(!roll(&mut minor, Vec3::ZERO, stride));
        assert_eq!(minor.rest, Rest::on_a_face(Vec3::ZERO), "standing still");
        let mut impacts = 0;
        for step in 1..=25 {
            impacts += usize::from(roll(&mut minor, Vec3::X * 0.1 * step as f32, stride));
        }
        assert_eq!(impacts, 2, "one box impact per completed flip");
        assert!((minor.progress - 0.5).abs() < 1e-3, "{}", minor.progress);
        assert_ne!(minor.rest.rotation, Rest::on_a_face(Vec3::ZERO).rotation);
        // Every rest it lands on stands on a face, level with where it began.
        assert!((minor.rest.centre.y - Rest::on_a_face(Vec3::ZERO).centre.y).abs() < 1e-3);
        assert_eq!(minor.rest.centre.with_y(0.0), Vec3::ZERO);
    }

    /// Turning back lands the tip in progress rather than tipping two ways at once.
    #[test]
    fn a_minor_that_turns_back_lands_its_tip_first() {
        let mut minor = visual();
        roll(&mut minor, Vec3::X * 0.5, 1.0);
        assert!(minor.progress > 0.4);
        roll(&mut minor, Vec3::X * 0.3, 1.0);
        assert!((minor.progress - 0.2).abs() < 1e-3);
        assert_eq!(minor.heading, -Vec3::X);
    }
}
