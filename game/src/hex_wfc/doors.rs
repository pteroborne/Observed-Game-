//! Deployed doors in Architect Ascent, drawn where the match stands them.
//!
//! The rules own every door and the physical match stands it (`hex_wfc::model::doors`); this
//! draws each and writes nothing back. A door is a bronze frame in the doorway - two posts
//! and a lintel - and a dark shutter that rolls down to close and up to open. A strip along
//! the lintel says the door's state from either side and from across a dark floor: the
//! exit green while it is open, the collapse red while it is closed, and the unpowered grey
//! when its floor has no power and it is frozen. Every door state is a critical signal, so
//! each keeps a self-lit minimum (design section 5).
//!
//! A door heard deployed, opened or closed is heard where it stands. The prompt at a door
//! says what interact will do, or why it will not.

use std::collections::BTreeMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use observed_hex::{HexCoord, HexFace};
use observed_match::ascent::facility::AtDoor;
use observed_match::hex_wfc::{DOOR_HALF_WIDTH, DOOR_HEIGHT, HexDoor};
use observed_style::equipment::{Hardware, finish};
use observed_style::kinetic::{Role, treatment};
use observed_style::{MarkerRole, marker};

use super::hud::words::PromptView;
use super::sim::HexWfcRuntime;
use crate::GameState;
use crate::settings::{Settings, key_name};

pub(super) mod capture;

/// How fast a shutter rolls, in doorways a second.
const ROLL_SPEED: f32 = 2.5;
/// How much of the doorway an open door's shutter still hangs down, rolled into the lintel.
const ROLLED: f32 = 0.04;
const POST: f32 = 0.22;
const DEPTH: f32 = 0.45;
const LINTEL: f32 = 0.3;

#[derive(Resource)]
pub(super) struct DoorAssets {
    post: Handle<Mesh>,
    lintel: Handle<Mesh>,
    strip: Handle<Mesh>,
    shutter: Handle<Mesh>,
    bronze: Handle<StandardMaterial>,
    panel: Handle<StandardMaterial>,
    open: Handle<StandardMaterial>,
    closed: Handle<StandardMaterial>,
    frozen: Handle<StandardMaterial>,
    sound: Handle<AudioSource>,
}

impl FromWorld for DoorAssets {
    fn from_world(world: &mut World) -> Self {
        let sound = world.resource::<AssetServer>().load("sounds/door.ogg");
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let width = DOOR_HALF_WIDTH * 2.0;
        let post = meshes.add(Cuboid::new(POST, DOOR_HEIGHT, DEPTH));
        let lintel = meshes.add(Cuboid::new(width + POST * 2.0, LINTEL, DEPTH));
        let strip = meshes.add(Cuboid::new(width, 0.07, DEPTH + 0.04));
        // A unit-tall shutter hung from its top edge, scaled down the doorway.
        let shutter = meshes.add(
            Cuboid::new(width, 1.0, 0.12)
                .mesh()
                .build()
                .translated_by(Vec3::new(0.0, -0.5, 0.0)),
        );
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let mut hardware = |part: Hardware| {
            let f = finish(part);
            materials.add(StandardMaterial {
                base_color: f.base_color,
                metallic: f.metallic,
                perceptual_roughness: f.roughness,
                ..default()
            })
        };
        let (bronze, panel) = (hardware(Hardware::Trim), hardware(Hardware::Body));
        let mut lit = |base: Color, emissive: LinearRgba| {
            materials.add(StandardMaterial {
                base_color: base,
                emissive,
                ..default()
            })
        };
        let (green, red, grey) = (
            marker(MarkerRole::Exit),
            marker(MarkerRole::Collapse),
            treatment(Role::Unpowered),
        );
        Self {
            post,
            lintel,
            strip,
            shutter,
            bronze,
            panel,
            open: lit(green.base_color, green.emissive * 0.6),
            closed: lit(red.base_color, red.emissive * 0.6),
            frozen: lit(grey.base_color, LinearRgba::from(grey.base_color) * 0.8),
            sound,
        }
    }
}

/// A door as drawn: its threshold, and how far down its shutter hangs (0 to 1).
#[derive(Component)]
pub(super) struct DoorVisual {
    key: (HexCoord, HexFace),
    shown: f32,
    height: f32,
}

#[derive(Component)]
pub(super) struct Shutter;

#[derive(Component)]
pub(super) struct Strip;

/// The doors as last drawn, to hear what changed.
#[derive(Resource, Default)]
pub(super) struct DoorPresentation {
    closed: BTreeMap<(HexCoord, HexFace), bool>,
}

fn spawn_door(commands: &mut Commands, assets: &DoorAssets, door: &HexDoor) -> Entity {
    let (floor, along) = door.pose();
    let rotation = Quat::from_rotation_arc(Vec3::X, along);
    let reach = DOOR_HALF_WIDTH + POST * 0.5;
    commands
        .spawn((
            DoorVisual {
                key: (door.cell, door.face),
                // Deployed rolled up: a door deployed closed rolls down into place.
                shown: ROLLED,
                height: door.height,
            },
            DespawnOnExit(GameState::HexWfc),
            Transform::from_translation(floor).with_rotation(rotation),
            Visibility::default(),
            Name::new("Door"),
        ))
        .with_children(|frame| {
            for side in [-1.0, 1.0] {
                frame.spawn((
                    Mesh3d(assets.post.clone()),
                    MeshMaterial3d(assets.bronze.clone()),
                    Transform::from_xyz(side * reach, door.height * 0.5, 0.0)
                        .with_scale(Vec3::new(1.0, door.height / DOOR_HEIGHT, 1.0)),
                ));
            }
            frame.spawn((
                Mesh3d(assets.lintel.clone()),
                MeshMaterial3d(assets.bronze.clone()),
                Transform::from_xyz(0.0, door.height + LINTEL * 0.5, 0.0),
            ));
            frame.spawn((
                Strip,
                Mesh3d(assets.strip.clone()),
                MeshMaterial3d(assets.closed.clone()),
                Transform::from_xyz(0.0, door.height - 0.02, 0.0),
            ));
            frame.spawn((
                Shutter,
                Mesh3d(assets.shutter.clone()),
                MeshMaterial3d(assets.panel.clone()),
                Transform::from_xyz(0.0, door.height, 0.0).with_scale(Vec3::new(
                    1.0,
                    ROLLED * door.height,
                    1.0,
                )),
            ));
        })
        .id()
}

type Shutters<'w, 's> = Query<'w, 's, &'static mut Transform, (With<Shutter>, Without<DoorVisual>)>;
type Strips<'w, 's> =
    Query<'w, 's, &'static mut MeshMaterial3d<StandardMaterial>, (With<Strip>, Without<Shutter>)>;

#[derive(SystemParam)]
pub(super) struct DoorParts<'w, 's> {
    doors: Query<'w, 's, (Entity, &'static mut DoorVisual, &'static Children)>,
    shutters: Shutters<'w, 's>,
    strips: Strips<'w, 's>,
}

/// Stand, roll and light every deployed door, and hear what changed.
pub(super) fn sync(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    time: Res<Time>,
    settings: Res<Settings>,
    assets: Option<Res<DoorAssets>>,
    presentation: Option<ResMut<DoorPresentation>>,
    parts: DoorParts,
) {
    let DoorParts {
        mut doors,
        mut shutters,
        mut strips,
    } = parts;
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let (Some(assets), Some(mut presentation)) = (assets, presentation) else {
        commands.init_resource::<DoorAssets>();
        commands.init_resource::<DoorPresentation>();
        return;
    };
    let game = &runtime.match_state;
    let standing: BTreeMap<_, _> = game
        .doors()
        .map(|door| ((door.cell, door.face), door))
        .collect();
    // Gone: a rewritten or retracted cell took it.
    for (entity, visual, _) in &doors {
        if !standing
            .get(&visual.key)
            .is_some_and(|door| door.height == visual.height)
        {
            commands.entity(entity).despawn();
        }
    }
    let drawn: Vec<_> = doors
        .iter()
        .filter(|(_, visual, _)| {
            standing
                .get(&visual.key)
                .is_some_and(|door| door.height == visual.height)
        })
        .map(|(_, visual, _)| visual.key)
        .collect();
    let volume = settings.effective_sfx_volume();
    for (&key, door) in &standing {
        if !drawn.contains(&key) {
            spawn_door(&mut commands, &assets, door);
        }
        if presentation.closed.get(&key) != Some(&door.closed) {
            // Deployed, opened or closed: heard where it stands.
            let (floor, _) = door.pose();
            super::audio::play(
                &mut commands,
                assets.sound.clone(),
                0.7 * volume,
                "Door",
                Some(floor + Vec3::Y * (DOOR_HEIGHT * 0.5)),
            );
        }
    }
    presentation.closed = standing
        .iter()
        .map(|(&key, door)| (key, door.closed))
        .collect();

    let economy = &ascent.rules().economy;
    let step = time.delta_secs() * ROLL_SPEED;
    for (_, mut visual, children) in &mut doors {
        let Some(door) = standing.get(&visual.key) else {
            continue;
        };
        let target = if door.closed { 1.0 } else { ROLLED };
        visual.shown = if visual.shown < target {
            (visual.shown + step).min(target)
        } else {
            (visual.shown - step).max(target)
        };
        let lit = if !economy.is_powered(door.cell.level) {
            &assets.frozen
        } else if door.closed {
            &assets.closed
        } else {
            &assets.open
        };
        for child in children.iter() {
            if let Ok(mut transform) = shutters.get_mut(child) {
                transform.scale.y = visual.shown * visual.height;
            }
            if let Ok(mut material) = strips.get_mut(child)
                && material.0 != *lit
            {
                material.0 = lit.clone();
            }
        }
    }
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<DoorAssets>();
    commands.remove_resource::<DoorPresentation>();
}

/// The prompt at the door the local body stands at, if it stands at one.
pub(super) fn prompt(runtime: &HexWfcRuntime, settings: &Settings) -> Option<PromptView> {
    let (_, at) = runtime
        .ascent
        .as_ref()?
        .at_door(&runtime.match_state, runtime.local_player)?;
    Some(prompt_for(at, settings))
}

/// What the prompt says at a door.
pub(super) fn prompt_for(at: AtDoor, settings: &Settings) -> PromptView {
    let key = || key_name(settings.bindings.interact);
    let (key, pad, title, detail) = match at {
        AtDoor { powered: false, .. } => (
            String::new(),
            "",
            "Door frozen",
            "This floor has no power. Find its generator",
        ),
        AtDoor {
            operable: false, ..
        } => (String::new(), "", "Door", "It will not move now"),
        AtDoor { closed: true, .. } => (key(), "X", "Open the door", "Anyone may close it again"),
        AtDoor { closed: false, .. } => (
            key(),
            "X",
            "Close the door",
            "It stops bodies, minors and sight",
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
    fn a_door_says_what_interact_does_and_why_it_will_not() {
        let settings = Settings::default();
        let at = |closed, operable, powered| AtDoor {
            closed,
            operable,
            powered,
        };
        let open = prompt_for(at(true, true, true), &settings);
        assert_eq!(open.title, "Open the door");
        assert!(!open.key.is_empty());
        assert_eq!(
            prompt_for(at(false, true, true), &settings).title,
            "Close the door"
        );
        let frozen = prompt_for(at(true, false, false), &settings);
        assert_eq!(frozen.title, "Door frozen");
        assert!(frozen.key.is_empty(), "a frozen door names no key");
        assert!(prompt_for(at(false, false, true), &settings).key.is_empty());
    }
}
