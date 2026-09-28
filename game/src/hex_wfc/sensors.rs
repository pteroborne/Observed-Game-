//! The Rogue's sensors in Architect Ascent, drawn where the match hangs them.
//!
//! The rules own every sensor and the physical match hangs it (`hex_wfc::model::sensors`);
//! this draws each and writes nothing back. A sensor is the design's shape language turned
//! against the climbers: a polyhedral eye in the director's colour, turning inside a ring,
//! with a small light of its own. It burns brighter and its ring breathes while it sees
//! someone, and it goes the unpowered grey, unlit, when its floor has no power and it is
//! blind. Whether it watches is a critical signal, so each state keeps a self-lit minimum
//! (design section 5).
//!
//! A sensor is heard where it hangs when the Rogue installs it and when a body takes it
//! down. The prompt under one says interact takes it down.

use std::collections::BTreeSet;

use bevy::prelude::*;
use observed_hex::HexCoord;
use observed_match::ascent::facility::AtSensor;
use observed_style::kinetic::{Role, treatment};
use observed_style::{MarkerRole, marker};

use super::hud::words::PromptView;
use super::sim::HexWfcRuntime;
use crate::GameState;
use crate::settings::{Settings, key_name};

pub(super) mod capture;

/// How fast an eye turns, radians a second; faster while it sees someone.
const TURN: f32 = 0.6;
const TURN_WATCHING: f32 = 2.4;
/// How fast a watching sensor's ring breathes, cycles a second.
const BREATH: f32 = 1.5;

#[derive(Resource)]
pub(super) struct SensorAssets {
    eye: Handle<Mesh>,
    ring: Handle<Mesh>,
    live: Handle<StandardMaterial>,
    watching: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    installed: Handle<AudioSource>,
    dismantled: Handle<AudioSource>,
}

impl FromWorld for SensorAssets {
    fn from_world(world: &mut World) -> Self {
        let server = world.resource::<AssetServer>();
        let (installed, dismantled) = (
            server.load("sounds/reroute.ogg"),
            server.load("sounds/tool_interact.ogg"),
        );
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let eye = meshes.add(Sphere::new(0.3).mesh().ico(0).expect("an icosahedron"));
        let ring = meshes.add(Torus::new(0.5, 0.56));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let director = marker(MarkerRole::Director);
        let grey = treatment(Role::Unpowered);
        let mut lit = |base: Color, emissive: LinearRgba| {
            materials.add(StandardMaterial {
                base_color: base,
                emissive,
                ..default()
            })
        };
        Self {
            eye,
            ring,
            live: lit(director.base_color, director.emissive * 0.6),
            watching: lit(director.base_color, director.emissive * 1.6),
            dark: lit(grey.base_color, LinearRgba::from(grey.base_color) * 0.5),
            installed,
            dismantled,
        }
    }
}

/// A sensor as drawn: the cell it watches from.
#[derive(Component)]
pub(super) struct SensorVisual {
    cell: HexCoord,
}

#[derive(Component)]
pub(super) struct Eye;

#[derive(Component)]
pub(super) struct Ring;

/// The sensors as last drawn, to hear what changed.
#[derive(Resource, Default)]
pub(super) struct SensorPresentation {
    hung: BTreeSet<HexCoord>,
}

fn spawn_sensor(commands: &mut Commands, assets: &SensorAssets, cell: HexCoord, at: Vec3) {
    let light = marker(MarkerRole::Director).base_color;
    commands
        .spawn((
            SensorVisual { cell },
            DespawnOnExit(GameState::HexWfc),
            Transform::from_translation(at),
            Visibility::default(),
            Name::new("Sensor"),
        ))
        .with_children(|sensor| {
            sensor.spawn((
                Eye,
                Mesh3d(assets.eye.clone()),
                MeshMaterial3d(assets.live.clone()),
                Transform::default(),
            ));
            sensor.spawn((
                Ring,
                Mesh3d(assets.ring.clone()),
                MeshMaterial3d(assets.live.clone()),
                Transform::default(),
            ));
            sensor.spawn((
                PointLight {
                    color: light,
                    intensity: 60_000.0,
                    range: 6.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, -0.4, 0.0),
            ));
        });
}

type Parts<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut MeshMaterial3d<StandardMaterial>,
        Has<Eye>,
    ),
    Or<(With<Eye>, With<Ring>)>,
>;

/// Hang, turn and light every sensor, and hear what changed.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    time: Res<Time>,
    settings: Res<Settings>,
    assets: Option<Res<SensorAssets>>,
    presentation: Option<ResMut<SensorPresentation>>,
    sensors: Query<(Entity, &SensorVisual, &Children)>,
    mut parts: Parts,
    mut lights: Query<&mut Visibility, (With<PointLight>, Without<SensorVisual>)>,
) {
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let (Some(assets), Some(mut presentation)) = (assets, presentation) else {
        commands.init_resource::<SensorAssets>();
        commands.init_resource::<SensorPresentation>();
        return;
    };
    let hung: Vec<(HexCoord, Vec3)> = runtime.match_state.sensors().collect();
    let volume = settings.effective_sfx_volume();
    for (entity, visual, _) in &sensors {
        if !hung.iter().any(|&(cell, _)| cell == visual.cell) {
            commands.entity(entity).despawn();
        }
    }
    let drawn: Vec<HexCoord> = sensors.iter().map(|(_, visual, _)| visual.cell).collect();
    for &(cell, at) in &hung {
        if !drawn.contains(&cell) {
            spawn_sensor(&mut commands, &assets, cell, at);
        }
    }
    // Installed or taken down: heard where it hangs.
    let now: BTreeSet<HexCoord> = hung.iter().map(|&(cell, _)| cell).collect();
    for &(_, at) in hung
        .iter()
        .filter(|(cell, _)| !presentation.hung.contains(cell))
    {
        super::audio::play(
            &mut commands,
            assets.installed.clone(),
            0.6 * volume,
            "Sensor",
            Some(at),
        );
    }
    for &cell in presentation.hung.difference(&now) {
        let at = Vec3::from_array(observed_hex::hex_origin(cell));
        super::audio::play(
            &mut commands,
            assets.dismantled.clone(),
            0.8 * volume,
            "Sensor",
            Some(at),
        );
    }
    presentation.hung = now;

    let rules = ascent.rules();
    let seconds = time.elapsed_secs();
    for (_, visual, children) in &sensors {
        let live = rules.sensor_live(visual.cell);
        let watching = live && rules.sensor_watching(visual.cell);
        let material = if watching {
            &assets.watching
        } else if live {
            &assets.live
        } else {
            &assets.dark
        };
        let turn = if watching { TURN_WATCHING } else { TURN };
        for child in children.iter() {
            if let Ok((mut transform, mut lit, eye)) = parts.get_mut(child) {
                if lit.0 != *material {
                    lit.0 = material.clone();
                }
                if eye {
                    transform.rotation =
                        Quat::from_rotation_y(seconds * turn) * Quat::from_rotation_x(0.4);
                } else {
                    let breath = if watching {
                        1.0 + 0.15 * (seconds * BREATH * std::f32::consts::TAU).sin()
                    } else {
                        1.0
                    };
                    transform.scale = Vec3::splat(breath);
                }
            }
            if let Ok(mut visibility) = lights.get_mut(child) {
                let wanted = if live {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *visibility != wanted {
                    *visibility = wanted;
                }
            }
        }
    }
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<SensorAssets>();
    commands.remove_resource::<SensorPresentation>();
}

/// The prompt under the sensor the local body stands at, if it stands under one.
pub(super) fn prompt(runtime: &HexWfcRuntime, settings: &Settings) -> Option<PromptView> {
    let at = runtime
        .ascent
        .as_ref()?
        .at_sensor(&runtime.match_state, runtime.local_player)?;
    Some(prompt_for(at, settings))
}

/// What the prompt says under a sensor.
pub(super) fn prompt_for(at: AtSensor, settings: &Settings) -> PromptView {
    let (key, pad, title, detail) = match at {
        AtSensor {
            dismantlable: false,
            ..
        } => (String::new(), "", "Rogue sensor", "Out of reach"),
        AtSensor { live: false, .. } => (
            key_name(settings.bindings.interact),
            "X",
            "Take down the sensor",
            "Blind while this floor has no power",
        ),
        AtSensor { live: true, .. } => (
            key_name(settings.bindings.interact),
            "X",
            "Take down the sensor",
            "It shows the Rogue whoever it sees",
        ),
    };
    PromptView {
        key,
        pad,
        title: title.to_owned(),
        detail,
        progress: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sensor_says_interact_takes_it_down_and_when_it_cannot() {
        let settings = Settings::default();
        let cell = HexCoord {
            q: 1,
            r: 1,
            level: 0,
        };
        let at = |dismantlable, live| AtSensor {
            cell,
            dismantlable,
            live,
        };
        let watching = prompt_for(at(true, true), &settings);
        assert_eq!(watching.title, "Take down the sensor");
        assert!(!watching.key.is_empty());
        assert_eq!(
            prompt_for(at(true, false), &settings).detail,
            "Blind while this floor has no power"
        );
        assert!(prompt_for(at(false, true), &settings).key.is_empty());
    }
}
