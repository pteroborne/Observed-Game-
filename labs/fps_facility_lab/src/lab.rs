use std::collections::BTreeMap;

use bevy::{
    ecs::system::SystemParam,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use observation_lab::model::{DOOR_COUNT, DoorId, ROOM_COUNT, Side};
use player_input::PlayerIntent;
use room_lab::{PortType, RoomTemplate};

/// Mouse-look sensitivity: raw pixel delta scaled into `PlayerIntent.look` units.
const MOUSE_SENS: f32 = 0.06;

use crate::model::{
    FacilityStage, MODULE_HALF, MODULE_HEIGHT, ModuleInstance3d, ModuleRegistry3d, PortRole3d,
    side_direction, world_port,
};

const WALL: Color = Color::srgb(0.32, 0.37, 0.48);
const OPEN: Color = Color::srgb(0.24, 0.82, 1.0);
const SEALED: Color = Color::srgb(1.0, 0.26, 0.20);
const OBSERVED: Color = Color::srgb(1.0, 0.76, 0.25);
const PLAYER: Color = Color::srgb(0.36, 1.0, 0.58);
const WALL_THICKNESS: f32 = 0.24;

#[derive(Component)]
pub(crate) struct PlayerCam;

#[derive(Component)]
pub(crate) struct FacilityUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ModuleVisualRoot {
    pub room: observed_core::RoomId,
    pub template: RoomTemplate,
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DoorPanelRoot(pub DoorId);

#[derive(Resource, Default)]
pub(crate) struct InputIntent(pub PlayerIntent);

#[derive(Resource)]
pub struct FacilityRuntime {
    pub auto_decohere: bool,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub camera_override: Option<Transform>,
}

impl Default for FacilityRuntime {
    fn default() -> Self {
        Self {
            auto_decohere: false,
            debug_visible: true,
            reset_requested: false,
            camera_override: None,
        }
    }
}

#[derive(Resource)]
pub(crate) struct DecohereTimer(Timer);

impl Default for DecohereTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(3.0, TimerMode::Repeating))
    }
}

#[derive(Resource, Clone)]
pub(crate) struct VisualAssets {
    cube: Handle<Mesh>,
    wall: Handle<StandardMaterial>,
    sealed: Handle<StandardMaterial>,
    templates: [Handle<StandardMaterial>; 8],
    ports: [Handle<StandardMaterial>; 7],
}

pub(crate) fn setup_lab(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stage: Res<FacilityStage>,
) {
    let assets = VisualAssets {
        cube: meshes.add(Cuboid::from_length(1.0)),
        wall: materials.add(WALL),
        sealed: materials.add(SEALED),
        templates: RoomTemplate::ALL.map(|template| materials.add(template.color())),
        ports: [
            materials.add(PortType::Passage.color()),
            materials.add(PortType::Door.color()),
            materials.add(PortType::Ladder.color()),
            materials.add(PortType::Machinery.color()),
            materials.add(PortType::Equipment.color()),
            materials.add(PortType::Grapple.color()),
            materials.add(PortType::Observation.color()),
        ],
    };

    commands.spawn((
        PlayerCam,
        Camera3d::default(),
        Transform::from_translation(stage.body.eye(&stage.config)),
        Name::new("Phase 23 First-Person Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 13_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.65, 0.0)),
        Name::new("Facility Sun"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.22, 0.28, 0.40),
        brightness: 160.0,
        ..default()
    });

    for module in &stage.modules {
        spawn_module(&mut commands, &assets, &stage.registry, module);
    }
    spawn_ui(&mut commands);
    commands.insert_resource(assets);
}

fn spawn_module(
    commands: &mut Commands,
    assets: &VisualAssets,
    registry: &ModuleRegistry3d,
    module: &ModuleInstance3d,
) {
    let definition = registry.load(module.template).expect("authored definition");
    let material = assets.templates[template_index(module.template)].clone();
    let rotation = Quat::from_rotation_y(module.pose.rotation.radians());
    commands
        .spawn((
            ModuleVisualRoot {
                room: module.room,
                template: module.template,
            },
            Transform::from_translation(module.pose.translation).with_rotation(rotation),
            Visibility::default(),
            Name::new(format!(
                "Room {} - {}",
                module.room.0,
                module.template.name()
            )),
        ))
        .with_children(|root| {
            local_box(
                root,
                assets,
                material.clone(),
                Vec3::new(0.0, -0.10, 0.0),
                Vec3::new(MODULE_HALF * 2.0, 0.20, MODULE_HALF * 2.0),
            );

            let segment = MODULE_HALF - crate::model::PORT_HALF;
            let offset = (MODULE_HALF + crate::model::PORT_HALF) * 0.5;
            let y = MODULE_HEIGHT * 0.5;
            for z in [-MODULE_HALF, MODULE_HALF] {
                for x in [-offset, offset] {
                    local_box(
                        root,
                        assets,
                        assets.wall.clone(),
                        Vec3::new(x, y, z),
                        Vec3::new(segment, MODULE_HEIGHT, WALL_THICKNESS * 2.0),
                    );
                }
            }
            for x in [-MODULE_HALF, MODULE_HALF] {
                for z in [-offset, offset] {
                    local_box(
                        root,
                        assets,
                        assets.wall.clone(),
                        Vec3::new(x, y, z),
                        Vec3::new(WALL_THICKNESS * 2.0, MODULE_HEIGHT, segment),
                    );
                }
            }

            for obstacle in &definition.obstacles {
                local_box(
                    root,
                    assets,
                    material.clone(),
                    obstacle.local_center,
                    obstacle.half * 2.0,
                );
            }

            for port in &definition.ports {
                let marker = if matches!(port.role, PortRole3d::Graph(_)) {
                    Vec3::new(0.18, 1.5, 0.18)
                } else {
                    Vec3::new(0.42, 0.42, 0.42)
                };
                local_box(
                    root,
                    assets,
                    assets.ports[port_type_index(port.kind)].clone(),
                    port.local_position,
                    marker,
                );
            }
        });
}

fn local_box(
    parent: &mut ChildSpawnerCommands,
    assets: &VisualAssets,
    material: Handle<StandardMaterial>,
    position: Vec3,
    size: Vec3,
) {
    parent.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position).with_scale(size),
    ));
}

fn spawn_door_panel(
    commands: &mut Commands,
    assets: &VisualAssets,
    stage: &FacilityStage,
    door: DoorId,
) {
    let reference = stage.projection.port_for_door(door);
    let module = stage.module(reference.room);
    let definition = stage
        .registry
        .load(module.template)
        .expect("authored definition");
    let port = world_port(module, definition, reference.port).expect("projected port");
    let direction = side_direction(port.facing);
    let size = if direction.x.abs() > 0.5 {
        Vec3::new(
            WALL_THICKNESS * 2.0,
            MODULE_HEIGHT,
            crate::model::PORT_HALF * 2.0,
        )
    } else {
        Vec3::new(
            crate::model::PORT_HALF * 2.0,
            MODULE_HEIGHT,
            WALL_THICKNESS * 2.0,
        )
    };
    commands.spawn((
        DoorPanelRoot(door),
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.sealed.clone()),
        Transform::from_translation(Vec3::new(
            port.position.x,
            MODULE_HEIGHT * 0.5,
            port.position.z,
        ))
        .with_scale(size),
        Name::new(format!("Sealed graph door {}", door.0)),
    ));
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            FacilityUiRoot,
            Name::new("FPS Facility UI Root"),
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                DebugPanel,
                Node {
                    width: px(510),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.94)),
                BorderColor::all(Color::srgba(0.45, 1.0, 0.68, 0.65)),
                children![(
                    DebugText,
                    Text::new("3D facility diagnostics starting..."),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 1.0, 0.92)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(455),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.94)),
                BorderColor::all(Color::srgba(0.45, 1.0, 0.68, 0.65)),
                children![(
                    Text::new(
                        "FPS FACILITY LAB (Phase 23)\n\
                         WASD               Move\n\
                         Arrow keys         Look\n\
                         Shift              Sprint\n\
                         Space              Jump\n\
                         C                  Decohere unseen graph\n\
                         P                  Toggle auto-decoherence\n\
                         R reset - F1 debug\n\n\
                         Walk through a cyan Passage opening. Crossing its threshold\n\
                         follows the current observation-graph partner and places you\n\
                         inside that destination room. Red openings are graph-sealed.\n\
                         Small colored fixtures are the typed room_lab ports promoted\n\
                         into 3D: Door, Ladder, Machinery, Equipment, Grapple, and\n\
                         Observation.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.90, 0.96, 0.94)),
                )],
            ));
        });
}

pub(crate) fn map_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut intent: ResMut<InputIntent>,
) {
    let axis = |negative: KeyCode, positive: KeyCode| {
        (keyboard.pressed(positive) as i32 - keyboard.pressed(negative) as i32) as f32
    };
    let mouse = mouse_motion.map(|m| m.delta).unwrap_or(Vec2::ZERO);
    let mut movement = Vec2::new(
        axis(KeyCode::KeyA, KeyCode::KeyD),
        axis(KeyCode::KeyS, KeyCode::KeyW),
    );
    if movement.length_squared() > 1.0 {
        movement = movement.normalize_or_zero();
    }
    // Look from the mouse (primary) plus arrow keys (fallback); left unclamped.
    intent.0 = PlayerIntent {
        movement,
        look: Vec2::new(
            axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) + mouse.x * MOUSE_SENS,
            axis(KeyCode::ArrowUp, KeyCode::ArrowDown) + mouse.y * MOUSE_SENS,
        ),
        jump_pressed: keyboard.pressed(KeyCode::Space),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        ..default()
    };
}

/// Lock and hide the cursor for mouse look (graceful when there is no window).
pub(crate) fn grab_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

/// Toggle the cursor grab with Escape so the window can be freed.
pub(crate) fn toggle_grab(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    if let Ok(mut cursor) = cursors.single_mut() {
        let grabbed = cursor.grab_mode != CursorGrabMode::None;
        cursor.grab_mode = if grabbed {
            CursorGrabMode::None
        } else {
            CursorGrabMode::Locked
        };
        cursor.visible = grabbed;
    }
}

pub(crate) fn handle_actions(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut stage: ResMut<FacilityStage>,
    mut runtime: ResMut<FacilityRuntime>,
) {
    if keyboard.just_pressed(KeyCode::KeyC) {
        stage.decohere();
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        runtime.auto_decohere = !runtime.auto_decohere;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
}

pub(crate) fn simulate(intent: Res<InputIntent>, mut stage: ResMut<FacilityStage>) {
    stage.step(intent.0);
}

pub(crate) fn auto_decohere(
    time: Res<Time>,
    runtime: Res<FacilityRuntime>,
    mut timer: ResMut<DecohereTimer>,
    mut stage: ResMut<FacilityStage>,
) {
    if runtime.auto_decohere && timer.0.tick(time.delta()).just_finished() {
        stage.decohere();
    }
}

pub(crate) fn perform_reset(
    mut stage: ResMut<FacilityStage>,
    mut runtime: ResMut<FacilityRuntime>,
    mut timer: ResMut<DecohereTimer>,
) {
    if !runtime.reset_requested {
        return;
    }
    stage.reset();
    let camera_override = runtime.camera_override.take();
    *runtime = FacilityRuntime::default();
    runtime.camera_override = camera_override;
    timer.0.reset();
}

pub(crate) fn sync_door_panels(
    mut commands: Commands,
    assets: Res<VisualAssets>,
    stage: Res<FacilityStage>,
    existing: Query<(Entity, &DoorPanelRoot)>,
) {
    let mut present = BTreeMap::new();
    for (entity, panel) in &existing {
        if stage.graph.is_sealed(panel.0) {
            present.insert(panel.0, entity);
        } else {
            commands.entity(entity).despawn();
        }
    }
    for index in 0..DOOR_COUNT {
        let door = DoorId(index as u16);
        if stage.graph.is_sealed(door) && !present.contains_key(&door) {
            spawn_door_panel(&mut commands, &assets, &stage, door);
        }
    }
}

pub(crate) fn present_camera(
    stage: Res<FacilityStage>,
    runtime: Res<FacilityRuntime>,
    mut camera: Single<&mut Transform, With<PlayerCam>>,
) {
    if let Some(pose) = runtime.camera_override {
        **camera = pose;
        return;
    }
    **camera = Transform::from_translation(stage.body.eye(&stage.config))
        .looking_to(stage.body.look_dir(), Vec3::Y);
}

pub(crate) fn draw_debug(
    stage: Res<FacilityStage>,
    runtime: Res<FacilityRuntime>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    for module in &stage.modules {
        let centre = module.pose.translation;
        let color = if module.room == stage.player_room {
            OBSERVED
        } else {
            WALL
        };
        let half = MODULE_HALF;
        let y = 0.03;
        gizmos.linestrip(
            [
                Vec3::new(centre.x - half, y, centre.z - half),
                Vec3::new(centre.x + half, y, centre.z - half),
                Vec3::new(centre.x + half, y, centre.z + half),
                Vec3::new(centre.x - half, y, centre.z + half),
                Vec3::new(centre.x - half, y, centre.z - half),
            ],
            color,
        );
        if module.room == stage.player_room {
            gizmos.line(centre, centre + Vec3::Y * 6.0, OBSERVED);
        }
    }

    for connection in &stage.projection.connections {
        let a_module = stage.module(connection.a.room);
        let b_module = stage.module(connection.b.room);
        let a_def = stage.registry.load(a_module.template).unwrap();
        let b_def = stage.registry.load(b_module.template).unwrap();
        let a = world_port(a_module, a_def, connection.a.port).unwrap();
        let b = world_port(b_module, b_def, connection.b.port).unwrap();
        let color =
            if connection.a.room == stage.player_room || connection.b.room == stage.player_room {
                OBSERVED
            } else {
                OPEN
            };
        gizmos.line(
            Vec3::new(a.position.x, MODULE_HEIGHT + 1.2, a.position.z),
            Vec3::new(b.position.x, MODULE_HEIGHT + 1.2, b.position.z),
            color,
        );
    }

    for module in &stage.modules {
        let definition = stage.registry.load(module.template).unwrap();
        for port in &definition.ports {
            let port = world_port(module, definition, port.id).unwrap();
            let color = port.kind.color();
            gizmos.sphere(
                Isometry3d::from_translation(port.position),
                if matches!(port.role, PortRole3d::Graph(_)) {
                    0.18
                } else {
                    0.28
                },
                color,
            );
        }
    }

    gizmos.line(
        stage.body.position,
        stage.body.position + Vec3::Y * 2.0,
        PLAYER,
    );
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    stage: Res<'w, FacilityStage>,
    runtime: Res<'w, FacilityRuntime>,
    cameras: Query<'w, 's, (), With<PlayerCam>>,
    ui_roots: Query<'w, 's, (), With<FacilityUiRoot>>,
    modules: Query<'w, 's, &'static ModuleVisualRoot>,
    panels: Query<'w, 's, &'static DoorPanelRoot>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    panel: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
}

pub(crate) fn update_debug_text(mut context: DebugContext) {
    let visibility = if context.runtime.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.panel = visibility;
    **context.help = visibility;

    let current_doors = Side::ALL
        .into_iter()
        .map(|side| {
            let door = context.stage.graph.door_id(context.stage.player_room, side);
            if context.stage.graph.is_sealed(door) {
                format!("{}:sealed", side.label())
            } else {
                let destination = context.stage.graph.door(context.stage.graph.partner(door));
                format!(
                    "{}:R{}{}",
                    side.label(),
                    destination.room.0,
                    destination.side.label()
                )
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    let cameras = context.cameras.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let module_count = context.modules.iter().count();
    let panel_count = context.panels.iter().count();
    let sealed_count = (0..DOOR_COUNT)
        .filter(|index| context.stage.graph.is_sealed(DoorId(*index as u16)))
        .count();
    let healthy = cameras == 1
        && ui_roots == 1
        && module_count == ROOM_COUNT
        && panel_count == sealed_count
        && context.stage.projection_exact();

    let last_route = context.stage.traversal_history.last().map_or_else(
        || "none yet".to_string(),
        |route| {
            format!(
                "R{}{} -> R{}{}",
                route.from_room.0,
                route.from_side.label(),
                route.to_room.0,
                route.to_side.label()
            )
        },
    );

    let mut text = context.text.into_inner();
    **text = format!(
        "3D FACILITY FROM GRAPH  {}\n\
         room / template  {} / {}\n\
         current ports    {}\n\
         graph doors      {} -> {} typed Passage ports\n\
         graph links      {} rendered connections\n\
         projection exact {}\n\
         authored reach   {} / {} rooms\n\
         traversals       {}  last {}\n\
         decoherences     {}  rewired last {}\n\
         position         ({:.1}, {:.1}, {:.1})\n\
         auto             {}\n\
         camera {}  UI {}  modules {}  panels {}  resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.stage.player_room.0,
        context.stage.current_template().name(),
        current_doors,
        DOOR_COUNT,
        context.stage.projection.door_ports.len(),
        context.stage.projection.connections.len(),
        if context.stage.projection_exact() {
            "PASS"
        } else {
            "FAIL"
        },
        crate::model::reachable_rooms(
            &observation_lab::model::ObservationWorld::authored(),
            observed_core::RoomId(0)
        )
        .len(),
        ROOM_COUNT,
        context.stage.traversal_history.len(),
        last_route,
        context.stage.decohere_count,
        context.stage.graph.rewires_last,
        context.stage.body.position.x,
        context.stage.body.position.y,
        context.stage.body.position.z,
        if context.runtime.auto_decohere {
            "on"
        } else {
            "off"
        },
        cameras,
        ui_roots,
        module_count,
        panel_count,
        context.stage.reset_count,
        context.stage.last_event,
    );
}

pub(crate) fn stage_capture_showcase(stage: &mut FacilityStage) {
    stage.relocate_player(observed_core::RoomId(4), Side::West);
    stage.traverse(Side::East);
    stage.traverse(Side::North);
    stage.decohere();
}

fn template_index(template: RoomTemplate) -> usize {
    RoomTemplate::ALL
        .iter()
        .position(|candidate| *candidate == template)
        .unwrap_or(0)
}

fn port_type_index(kind: PortType) -> usize {
    match kind {
        PortType::Passage => 0,
        PortType::Door => 1,
        PortType::Ladder => 2,
        PortType::Machinery => 3,
        PortType::Equipment => 4,
        PortType::Grapple => 5,
        PortType::Observation => 6,
    }
}
