//! Stable-domain presentation of the other Observers, the objectives and the exit
//! beacon. The objectives' bodies are built in `objective_models`; an Observer's eye
//! in `observer`.

use bevy::prelude::*;
use observed_authoring::RoomSocketKind;
use observed_core::PlayerId;
use observed_hex::hex_origin;
use observed_match::hex_wfc::HexBodyPlace;
use observed_style::{MarkerRole, OutlineRole};

use super::objective_models::{ObjectiveModels, SYNC_COLUMN, SyncColumn, sync_fill};
use super::sim::HexWfcRuntime;
use crate::GameState;

#[derive(Component)]
pub(super) struct ActorVisual(PlayerId);

impl ActorVisual {
    /// Whose body this draws.
    pub(super) fn player(&self) -> PlayerId {
        self.0
    }
}

/// The exit beacon's root, so the spectator overview can hold it to its storey.
#[derive(Component)]
pub(super) struct ExitBeacon;

#[derive(Component)]
pub(super) struct ObjectiveVisual {
    room_generation_key: u64,
    kind: RoomSocketKind,
}

#[derive(Component)]
pub(super) struct ObjectiveLabel;

#[derive(Resource)]
pub(super) struct EntityVisualAssets {
    exit: Handle<StandardMaterial>,
    pickup_material: Handle<StandardMaterial>,
    interactable_material: Handle<StandardMaterial>,
}

type ObjectiveVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ObjectiveVisual,
        &'static mut Visibility,
        Option<&'static mut Transform>,
        Option<&'static ObjectiveLabel>,
        Option<&'static mut Text2d>,
    ),
    Without<ActorVisual>,
>;

pub(super) fn setup(
    mut commands: Commands,
    runtime: Res<HexWfcRuntime>,
    cosmetics: Res<super::cosmetics::MatchCosmetics>,
    desk: Option<Res<super::architect::ArchitectDesk>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = EntityVisualAssets {
        exit: signal_material(&mut materials, MarkerRole::Exit),
        pickup_material: outline_material(&mut materials, OutlineRole::Pickup),
        interactable_material: outline_material(&mut materials, OutlineRole::Interactable),
    };
    let eyes = super::observer::ObserverArt::new(&mut meshes, &mut materials);
    let badge = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let trail = meshes.add(Sphere::new(1.0));
    let cosmetic_materials: Vec<_> = (0..4)
        .map(|id| {
            let finish = observed_style::cosmetics::trim(id);
            materials.add(StandardMaterial {
                base_color: finish.base_color,
                metallic: finish.metallic,
                perceptual_roughness: finish.roughness,
                ..default()
            })
        })
        .collect();
    // Every body gets an eye, yours included: which one the camera is inside changes
    // during a match - a spectator's focus, the Architect looking through an Observer -
    // so `sync` hides it there rather than this leaving it out here.
    for player in runtime.match_state.players.values() {
        let local_team = runtime.local().team;
        let role = if player.id == runtime.local_player && desk.is_none() {
            MarkerRole::You
        } else if player.team == local_team {
            MarkerRole::Teammate
        } else {
            MarkerRole::Rival
        };
        // A floating eye at the body's eye height, compact where the old capsule was
        // body-sized: bodies begin close together, and a full-height figure at arm's
        // length hid the architecture players must read.
        let root = commands
            .spawn((
                ActorVisual(player.id),
                DespawnOnExit(GameState::HexWfc),
                Transform::from_translation(player.position),
                Visibility::default(),
                Name::new(format!("Observer {} eye", player.id.0)),
            ))
            .id();
        let look = cosmetics.look(player.id);
        eyes.dress(&mut commands, root, role, u32::from(player.id.0), look);
        super::cosmetics::decorate(
            &mut commands,
            root,
            player.id,
            look,
            (
                &badge,
                &trail,
                &cosmetic_materials[usize::from(look.color.min(3))],
            ),
        );
    }
    let models = ObjectiveModels::new(
        &mut meshes,
        &mut materials,
        assets.pickup_material.clone(),
        assets.interactable_material.clone(),
        assets.exit.clone(),
    );
    let exit_origin = Vec3::from_array(hex_origin(runtime.match_state.facility.config.exit()));
    let exit_floor = exit_origin + Vec3::Y * observed_hex::FLOOR_SLAB_TOP;
    commands
        .spawn((
            ExitBeacon,
            DespawnOnExit(GameState::HexWfc),
            Transform::from_translation(exit_floor),
            Visibility::Visible,
            Name::new("hex exit beacon"),
        ))
        .with_children(|root| models.spawn_parts(root, RoomSocketKind::Exit, 0));
    commands.spawn((
        DespawnOnExit(GameState::HexWfc),
        PointLight {
            color: observed_style::marker(MarkerRole::Exit).base_color,
            intensity: 2_200.0,
            range: 18.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(exit_origin + Vec3::Y * 2.5),
        Name::new("hex exit beacon light"),
    ));
    for socket in &runtime.match_state.geometry.sockets {
        match socket.kind {
            RoomSocketKind::Keystone
            | RoomSocketKind::StationA
            | RoomSocketKind::StationB
            | RoomSocketKind::Monitor
            | RoomSocketKind::GuardianControl
            | RoomSocketKind::Recovery
            | RoomSocketKind::LanternCache => {}
            RoomSocketKind::Exit => {
                commands.spawn((
                    ObjectiveVisual {
                        room_generation_key: socket.room_generation_key,
                        kind: socket.kind,
                    },
                    ObjectiveLabel,
                    DespawnOnExit(GameState::HexWfc),
                    Text2d::new(socket_glyph(socket.kind)),
                    TextFont {
                        font_size: FontSize::Px(40.0),
                        ..default()
                    },
                    TextColor(observed_style::outline(OutlineRole::ObjectiveBeacon).color),
                    Transform::from_translation(socket.position + Vec3::Y * 4.2),
                    Name::new("EXIT mechanism label"),
                ));
                continue;
            }
        };
        let visual = ObjectiveVisual {
            room_generation_key: socket.room_generation_key,
            kind: socket.kind,
        };
        commands
            .spawn((
                visual,
                DespawnOnExit(GameState::HexWfc),
                Transform::from_translation(socket.position)
                    .with_rotation(Quat::from_rotation_y(socket.yaw_degrees.to_radians())),
                Visibility::Visible,
                Name::new(format!("{} mechanism", socket_glyph(socket.kind))),
            ))
            .with_children(|root| {
                models.spawn_parts(root, socket.kind, socket.room_generation_key);
            });
        commands.spawn((
            ObjectiveVisual {
                room_generation_key: socket.room_generation_key,
                kind: socket.kind,
            },
            ObjectiveLabel,
            DespawnOnExit(GameState::HexWfc),
            Text2d::new(socket_glyph(socket.kind)),
            TextFont {
                font_size: FontSize::Px(34.0),
                ..default()
            },
            TextColor(observed_style::outline(outline_role(socket.kind)).color),
            Transform::from_translation(socket.position + Vec3::Y * 1.75),
            Name::new(format!("{} mechanism label", socket_glyph(socket.kind))),
        ));
    }
    commands.insert_resource(assets);
    commands.insert_resource(models);
}

/// Station sync columns, which fill with the team's progress.
type SyncColumns<'w, 's> = Query<
    'w,
    's,
    (
        &'static SyncColumn,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    (Without<ActorVisual>, Without<ObjectiveVisual>),
>;

pub(super) fn sync(
    runtime: Res<HexWfcRuntime>,
    (spectating, overview): (
        Option<Res<crate::sim::state::SpectatorBot>>,
        Res<super::view::spectate::SpectatorOverview>,
    ),
    mut columns: SyncColumns,
    mut actors: Query<(&ActorVisual, &mut Transform, &mut Visibility)>,
    camera: Query<&GlobalTransform, With<crate::view::components::GameCam>>,
    mut objectives: ObjectiveVisualQuery,
) {
    // The body the camera is inside, if it is inside one: in play, and a spectator
    // looking through the followed body's eyes. Drawing it there put the back of its
    // own iris over the whole view.
    let inside =
        (spectating.is_none() || (overview.eyes && !overview.active)).then(|| runtime.viewed().id);
    for (visual, mut transform, mut visibility) in &mut actors {
        let player = &runtime.match_state.players[&visual.0];
        transform.translation = super::ascent::presented_position(player);
        // A body lost to the void has left play.
        *visibility =
            if player.escaped || player.place == HexBodyPlace::Void || inside == Some(player.id) {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
    }
    let camera_rotation = camera.single().ok().map(GlobalTransform::rotation);
    let local_objectives = runtime.match_state.teams[&runtime.local().team].objectives;
    for (column, mut transform, mut visibility) in &mut columns {
        let fill = sync_fill(
            column.room_generation_key,
            local_objectives.dual_station_room,
            local_objectives.dual_station_ticks,
            local_objectives.dual_station_complete,
        );
        transform.scale = Vec3::new(1.0, (fill * SYNC_COLUMN).max(0.01), 1.0);
        *visibility = if fill > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (visual, mut visibility, transform, label, text) in &mut objectives {
        let visible = visual.kind != RoomSocketKind::Keystone
            || runtime
                .match_state
                .objectives
                .available_keystones
                .contains(&visual.room_generation_key);
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let (Some(rotation), Some(mut transform), Some(_)) = (camera_rotation, transform, label)
        {
            transform.rotation = rotation;
        }
        if let (Some(mut text), Some(_)) = (text, label) {
            **text = match visual.kind {
                RoomSocketKind::StationA | RoomSocketKind::StationB
                    if local_objectives.dual_station_complete =>
                {
                    "SYNC READY".to_string()
                }
                RoomSocketKind::StationA | RoomSocketKind::StationB
                    if local_objectives.dual_station_room == Some(visual.room_generation_key)
                        && local_objectives.dual_station_ticks > 0 =>
                {
                    let progress = u32::from(local_objectives.dual_station_ticks) * 100
                        / u32::from(observed_match::hex_wfc::DUAL_STATION_HOLD_TICKS);
                    format!("{} {progress}%", socket_glyph(visual.kind))
                }
                RoomSocketKind::Monitor
                    if runtime
                        .match_state
                        .objectives
                        .surveyed_monitors
                        .contains(&(runtime.local().team, visual.room_generation_key)) =>
                {
                    "SURVEYED".to_string()
                }
                _ => socket_glyph(visual.kind).to_string(),
            };
        }
    }
}

fn socket_glyph(kind: RoomSocketKind) -> &'static str {
    match kind {
        RoomSocketKind::Keystone => "KEY",
        RoomSocketKind::StationA => "SYNC A",
        RoomSocketKind::StationB => "SYNC B",
        RoomSocketKind::Monitor => "SURVEY",
        RoomSocketKind::LanternCache => "ANCHOR",
        RoomSocketKind::GuardianControl => "GUARD",
        RoomSocketKind::Recovery => "RECOVER",
        RoomSocketKind::Exit => "EXIT",
    }
}

fn outline_role(kind: RoomSocketKind) -> OutlineRole {
    match kind {
        RoomSocketKind::Keystone => OutlineRole::Pickup,
        RoomSocketKind::Exit => OutlineRole::ObjectiveBeacon,
        _ => OutlineRole::Interactable,
    }
}

fn outline_material(
    materials: &mut Assets<StandardMaterial>,
    role: OutlineRole,
) -> Handle<StandardMaterial> {
    let treatment = observed_style::outline(role);
    materials.add(StandardMaterial {
        base_color: treatment.color,
        emissive: treatment.color.to_linear() * 4.0,
        metallic: 0.18,
        perceptual_roughness: 0.3,
        ..default()
    })
}

pub(super) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<EntityVisualAssets>();
}

fn signal_material(
    materials: &mut Assets<StandardMaterial>,
    role: MarkerRole,
) -> Handle<StandardMaterial> {
    let treatment = observed_style::marker(role);
    materials.add(StandardMaterial {
        // Close-range rivals remain unmistakably style-owned signals without
        // turning into opaque bloom cards over thresholds and ramps.
        base_color: treatment.base_color.with_alpha(0.46),
        emissive: treatment.emissive * 0.24,
        alpha_mode: AlphaMode::Blend,
        metallic: 0.22,
        perceptual_roughness: 0.38,
        ..Default::default()
    })
}
