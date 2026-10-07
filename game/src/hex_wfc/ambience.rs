//! Bounded physical building soundscape, independent of simulation decisions.
use bevy::{
    audio::{PlaybackMode, SpatialScale, Volume},
    prelude::*,
};
use observed_content::ArchitectureRegister;
use observed_match::ascent::facility::FixtureKind;
use observed_style::backrooms::{BUZZ_GAIN, CREAK_GAIN, HVAC_GAIN, MACHINERY_GAIN};

use super::{sim::HexWfcRuntime, view::HexPractical};
use crate::{GameState, settings::Settings};

#[derive(Resource)]
struct Sounds {
    air: Handle<AudioSource>,
    dry: Handle<AudioSource>,
    wind: Handle<AudioSource>,
    buzz: Handle<AudioSource>,
    machine: Handle<AudioSource>,
    water: Handle<AudioSource>,
    creak: Handle<AudioSource>,
}
#[derive(Resource, Default)]
struct Soundscape {
    kind: Option<u8>,
    beds: Vec<Entity>,
    next_creak: f32,
}
#[derive(Component)]
struct Bed {
    active: bool,
    gain: f32,
}
#[derive(Component)]
struct Emitter {
    gain: f32,
    source: Handle<AudioSource>,
}

fn setup(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(Sounds {
        air: server.load(observed_assets::BACKROOMS_HVAC.path),
        dry: server.load(observed_assets::AMBIENCE_ARCHIVE.path),
        wind: server.load(observed_assets::AMBIENCE_GANTRY.path),
        buzz: server.load(observed_assets::BACKROOMS_BUZZ.path),
        machine: server.load(observed_assets::AMBIENCE_FOUNDRY.path),
        water: server.load(observed_assets::AMBIENCE_SPILLWAY.path),
        creak: server.load(observed_assets::BUILDING_CREAK.path),
    });
    commands.insert_resource(Soundscape::default());
}

fn cleanup(mut commands: Commands) {
    commands.remove_resource::<Sounds>();
    commands.remove_resource::<Soundscape>();
}

type Beds<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Bed,
        &'static mut PlaybackSettings,
        Option<&'static mut AudioSink>,
    ),
    Without<Emitter>,
>;
type Emitters<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Emitter,
        &'static mut Transform,
        &'static mut PlaybackSettings,
        Option<&'static mut SpatialAudioSink>,
    ),
    Without<Bed>,
>;
#[derive(bevy::ecs::system::SystemParam)]
struct Voices<'w, 's> {
    beds: Beds<'w, 's>,
    emitters: Emitters<'w, 's>,
    practicals: Query<'w, 's, (&'static HexPractical, &'static GlobalTransform), With<PointLight>>,
}
fn sync(
    mut commands: Commands,
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    settings: Res<Settings>,
    sounds: Res<Sounds>,
    mut state: ResMut<Soundscape>,
    voices: Voices,
) {
    let Voices {
        mut beds,
        mut emitters,
        practicals,
    } = voices;
    let player = runtime.viewed();
    let world = &runtime.match_state.facility;
    let register = world
        .architecture
        .get(&player.cell)
        .copied()
        .unwrap_or(ArchitectureRegister::LiminalGrid);
    let (kind, source) = match register {
        ArchitectureRegister::InfiniteGallery => (1, sounds.dry.clone()),
        ArchitectureRegister::Thinning => (2, sounds.wind.clone()),
        _ => (0, sounds.air.clone()),
    };
    if state.kind != Some(kind) {
        for (_, mut bed, _, _) in &mut beds {
            bed.active = false;
        }
        if state.beds.len() >= 2 {
            commands.entity(state.beds.remove(0)).despawn();
        }
        let id = commands
            .spawn((
                DespawnOnExit(GameState::HexWfc),
                Bed {
                    active: true,
                    gain: 0.0,
                },
                AudioPlayer(source),
                PlaybackSettings {
                    mode: PlaybackMode::Loop,
                    volume: Volume::Linear(0.0),
                    ..default()
                },
                Name::new("Facility room tone"),
            ))
            .id();
        state.beds.push(id);
        state.kind = Some(kind);
    }
    let volume = settings.effective_music_volume();
    for (_, mut bed, mut playback, sink) in &mut beds {
        let target = if bed.active { HVAC_GAIN } else { 0.0 };
        bed.gain += (target - bed.gain) * (time.delta_secs() * 2.0).min(1.0);
        playback.volume = Volume::Linear(bed.gain * volume);
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(bed.gain * volume));
        }
    }
    let powered = |level| {
        runtime
            .ascent
            .as_ref()
            .is_none_or(|rules| rules.rules().economy.is_powered(level))
    };
    let mut nearby = Vec::new();
    for (practical, transform) in &practicals {
        if practical.0.level == player.cell.level && powered(practical.0.level) {
            let at = transform.translation();
            nearby.push((
                at.distance_squared(player.position),
                at,
                sounds.buzz.clone(),
                BUZZ_GAIN,
            ));
        }
    }
    if let Some(ascent) = &runtime.ascent {
        for fixture in ascent.fixtures() {
            if fixture.kind == FixtureKind::Generator && powered(fixture.cell.level) {
                let at = fixture.floor + Vec3::Y;
                nearby.push((
                    at.distance_squared(player.position),
                    at,
                    sounds.machine.clone(),
                    MACHINERY_GAIN,
                ));
            }
        }
    }
    for blueprint in &world.blueprints {
        if world.placements.get(&blueprint.anchor).is_some_and(|p| {
            matches!(
                p.archetype,
                observed_facility::hex_wfc::HexArchetype::Cistern { .. }
            )
        }) {
            let at = Vec3::from_array(observed_hex::hex_origin(blueprint.anchor)) + Vec3::Y;
            nearby.push((
                at.distance_squared(player.position),
                at,
                sounds.water.clone(),
                MACHINERY_GAIN,
            ));
        }
    }
    nearby.retain(|(distance, _, _, _)| *distance <= 24.0 * 24.0);
    nearby.sort_by(|a, b| a.0.total_cmp(&b.0));
    nearby.truncate(4);
    let existing: Vec<_> = emitters.iter().map(|(id, _, _, _, _)| id).collect();
    for (index, (_, at, source, gain)) in nearby.iter().enumerate() {
        if let Some(&id) = existing.get(index) {
            if let Ok((_, mut emitter, mut transform, mut playback, sink)) = emitters.get_mut(id) {
                transform.translation = *at;
                emitter.gain = *gain;
                playback.volume = Volume::Linear(*gain * volume);
                if let Some(mut sink) = sink {
                    sink.set_volume(Volume::Linear(*gain * volume));
                }
                // Source identity follows its slot as the nearest set changes.
                if emitter.source != *source {
                    emitter.source = source.clone();
                    commands
                        .entity(id)
                        .remove::<SpatialAudioSink>()
                        .insert(AudioPlayer(source.clone()));
                }
            }
        } else {
            commands.spawn((
                DespawnOnExit(GameState::HexWfc),
                Emitter {
                    gain: *gain,
                    source: source.clone(),
                },
                AudioPlayer(source.clone()),
                PlaybackSettings {
                    mode: PlaybackMode::Loop,
                    volume: Volume::Linear(*gain * volume),
                    spatial: true,
                    spatial_scale: Some(SpatialScale::new(super::audio::HEX_SPATIAL_SCALE)),
                    ..default()
                },
                Transform::from_translation(*at),
                Name::new("Local building sound"),
            ));
        }
    }
    for id in existing.into_iter().skip(nearby.len()) {
        commands.entity(id).despawn();
    }
    let now = time.elapsed_secs();
    if now >= state.next_creak {
        state.next_creak = now + 17.0 + (runtime.match_state.seed % 11) as f32;
        super::audio::play(
            &mut commands,
            sounds.creak.clone(),
            CREAK_GAIN * volume,
            "Mechanical building creak",
            Some(player.position + Vec3::new(8.0, 1.5, 5.0)),
        );
    }
}

pub(super) fn configure(app: &mut App) {
    app.add_systems(OnEnter(GameState::HexWfc), setup.after(super::audio::setup))
        .add_systems(
            Update,
            sync.after(super::view::sync_streamed_cells)
                .run_if(in_state(GameState::HexWfc)),
        )
        .add_systems(OnExit(GameState::HexWfc), cleanup);
}

#[cfg(test)]
mod tests;
