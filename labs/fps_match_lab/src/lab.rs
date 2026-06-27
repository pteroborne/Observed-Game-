use std::collections::BTreeMap;

use bevy::{
    ecs::system::SystemParam,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use competitive_facility::model::{EXIT_ROOM, TEAM_COUNT};
use director_lab::model::Role;
use fps_facility_lab::model::{
    MODULE_HALF, MODULE_HEIGHT, MODULE_SPACING, ModuleInstance3d, ModuleRegistry3d, PortRole3d,
    module_center, side_direction, world_port,
};
use observation_lab::model::{DOOR_COUNT, DoorId, ROOM_COUNT};
use observed_core::RoomId;
use player_input::PlayerIntent;
use room_lab::{PortType, RoomTemplate};

use crate::model::{FirstPersonMatch, MatchTape};

const WALL: Color = Color::srgb(0.28, 0.33, 0.44);
const OPEN: Color = Color::srgb(0.24, 0.82, 1.0);
const SPINE: Color = Color::srgb(1.0, 0.78, 0.24);
const COLLAPSE: Color = Color::srgb(1.0, 0.22, 0.24);
const TARGET: Color = Color::srgb(0.36, 1.0, 0.58);
const SEALED: Color = Color::srgb(1.0, 0.24, 0.18);
const WALL_THICKNESS: f32 = 0.24;
const TEAM_COLORS: [Color; TEAM_COUNT] = [
    Color::srgb(0.96, 0.28, 0.34),
    Color::srgb(0.32, 0.62, 1.0),
    Color::srgb(0.72, 0.46, 1.0),
    Color::srgb(1.0, 0.62, 0.20),
];
const TEAM_OFFSETS: [Vec3; TEAM_COUNT] = [
    Vec3::new(-1.8, 0.0, -1.8),
    Vec3::new(1.8, 0.0, -1.8),
    Vec3::new(-1.8, 0.0, 1.8),
    Vec3::new(1.8, 0.0, 1.8),
];
/// Mouse-look sensitivity: raw pixel delta scaled into `PlayerIntent.look` units.
const MOUSE_SENS: f32 = 0.06;

#[derive(Component)]
pub(crate) struct MatchCamera;

#[derive(Component)]
pub(crate) struct MatchUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Component)]
pub(crate) struct MatchModuleRoot;

#[derive(Component, Clone, Copy)]
pub(crate) struct TeamMarker(pub usize);

#[derive(Component, Clone, Copy)]
pub(crate) struct DoorPanel(pub DoorId);

#[derive(Resource, Default)]
pub(crate) struct InputIntent(pub PlayerIntent);

#[derive(Resource)]
pub(crate) struct LiveMatch(pub FirstPersonMatch);

#[derive(Resource, Default)]
pub(crate) struct LiveTape(pub MatchTape);

#[derive(Resource)]
pub(crate) struct DemoTape(pub MatchTape);

#[derive(Resource)]
pub(crate) struct ActiveView(pub FirstPersonMatch);

#[derive(Resource)]
pub(crate) struct ResolutionTimer(pub Timer);

impl Default for ResolutionTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.8, TimerMode::Repeating))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchMode {
    Live,
    Replay,
}

#[derive(Resource)]
pub struct MatchRuntime {
    pub mode: MatchMode,
    pub tactical_view: bool,
    pub replay_cursor: f32,
    pub replay_playing: bool,
    pub replay_speed: f32,
    pub replay_verified: bool,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub camera_override: Option<Transform>,
}

impl Default for MatchRuntime {
    fn default() -> Self {
        Self {
            mode: MatchMode::Live,
            tactical_view: false,
            replay_cursor: 0.0,
            replay_playing: false,
            replay_speed: 2.5,
            replay_verified: false,
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
            camera_override: None,
        }
    }
}

#[derive(Resource, Clone)]
pub(crate) struct VisualAssets {
    cube: Handle<Mesh>,
    wall: Handle<StandardMaterial>,
    sealed: Handle<StandardMaterial>,
    templates: [Handle<StandardMaterial>; 8],
    ports: [Handle<StandardMaterial>; 7],
    teams: [Handle<StandardMaterial>; TEAM_COUNT],
}

pub(crate) fn setup_lab(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    live: Res<LiveMatch>,
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
        teams: TEAM_COLORS.map(|color| materials.add(color)),
    };

    commands.spawn((
        MatchCamera,
        Camera3d::default(),
        Transform::from_translation(live.0.facility.body.eye(&live.0.facility.config)),
        Name::new("First-Person Match Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 13_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.65, 0.0)),
        Name::new("Match Sun"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.22, 0.28, 0.40),
        brightness: 160.0,
        ..default()
    });

    for module in &live.0.facility.modules {
        spawn_module(&mut commands, &assets, &live.0.facility.registry, module);
    }
    for team in 0..TEAM_COUNT {
        commands.spawn((
            TeamMarker(team),
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.teams[team].clone()),
            Transform::from_scale(Vec3::new(0.7, 2.4, 0.7)),
            Name::new(format!("Team {} marker", team + 1)),
        ));
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
    commands
        .spawn((
            MatchModuleRoot,
            Transform::from_translation(module.pose.translation)
                .with_rotation(Quat::from_rotation_y(module.pose.rotation.radians())),
            Visibility::default(),
            Name::new(format!(
                "Match room {} - {}",
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
            let segment = MODULE_HALF - fps_facility_lab::model::PORT_HALF;
            let offset = (MODULE_HALF + fps_facility_lab::model::PORT_HALF) * 0.5;
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
                local_box(
                    root,
                    assets,
                    assets.ports[port_type_index(port.kind)].clone(),
                    port.local_position,
                    if matches!(port.role, PortRole3d::Graph(_)) {
                        Vec3::new(0.18, 1.5, 0.18)
                    } else {
                        Vec3::splat(0.42)
                    },
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
    session: &FirstPersonMatch,
    door: DoorId,
) {
    let reference = session.facility.projection.port_for_door(door);
    let module = session.facility.module(reference.room);
    let definition = session.facility.registry.load(module.template).unwrap();
    let port = world_port(module, definition, reference.port).unwrap();
    let direction = side_direction(port.facing);
    let size = if direction.x.abs() > 0.5 {
        Vec3::new(
            WALL_THICKNESS * 2.0,
            MODULE_HEIGHT,
            fps_facility_lab::model::PORT_HALF * 2.0,
        )
    } else {
        Vec3::new(
            fps_facility_lab::model::PORT_HALF * 2.0,
            MODULE_HEIGHT,
            WALL_THICKNESS * 2.0,
        )
    };
    commands.spawn((
        DoorPanel(door),
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.sealed.clone()),
        Transform::from_translation(Vec3::new(
            port.position.x,
            MODULE_HEIGHT * 0.5,
            port.position.z,
        ))
        .with_scale(size),
    ));
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            MatchUiRoot,
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
                    width: px(520),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.95)),
                BorderColor::all(Color::srgba(0.45, 1.0, 0.68, 0.65)),
                children![(
                    DebugText,
                    Text::new("First-person match diagnostics starting..."),
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
                    width: px(465),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.95)),
                BorderColor::all(Color::srgba(0.45, 1.0, 0.68, 0.65)),
                children![(
                    Text::new(
                        "FIRST-PERSON COMPETITIVE MATCH (Phase 24)\n\
                         WASD + mouse / Shift / Space    Move, look, sprint, jump\n\
                         E                               Seize control at its console\n\
                         Tab                             Tac-map / first-person\n\
                         T                               Live / exact replay\n\
                         ← / →  [ / ]                    Replay step / seek\n\
                         Esc release cursor - R reset - F1 debug\n\n\
                         Follow the gold target Passage. Crossing it earns Team 1's\n\
                         Advance round; bots race simultaneously, unseen graph links\n\
                         rewire, and the collapse absorbs laggards into the director.\n\
                         The overhead view is the match_replay schematic promoted to\n\
                         the 3D facility: graph, teams, exit, collapse, and timeline\n\
                         are read directly from simulation/replay state.",
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
    // Look comes from the mouse (primary) plus arrow keys (fallback). Mouse-x turns,
    // mouse-y pitches; screen-down delta is positive, matching the controller's
    // look.y convention. Look is left unclamped so fast flicks feel responsive.
    let mouse = mouse_motion.map(|m| m.delta).unwrap_or(Vec2::ZERO);
    let look = Vec2::new(
        axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) + mouse.x * MOUSE_SENS,
        axis(KeyCode::ArrowUp, KeyCode::ArrowDown) + mouse.y * MOUSE_SENS,
    );
    let mut movement = Vec2::new(
        axis(KeyCode::KeyA, KeyCode::KeyD),
        axis(KeyCode::KeyS, KeyCode::KeyW),
    );
    if movement.length_squared() > 1.0 {
        movement = movement.normalize_or_zero();
    }
    intent.0 = PlayerIntent {
        movement,
        look,
        jump_pressed: keyboard.pressed(KeyCode::Space),
        sprint_held: keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        interact_pressed: keyboard.just_pressed(KeyCode::KeyE),
        ..default()
    };
}

/// Lock and hide the cursor for mouse look (graceful when there is no window, e.g.
/// in headless tests).
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

pub(crate) fn simulate_live(
    mut intent: ResMut<InputIntent>,
    runtime: Res<MatchRuntime>,
    mut live: ResMut<LiveMatch>,
    mut tape: ResMut<LiveTape>,
) {
    if runtime.mode != MatchMode::Live {
        intent.0.interact_pressed = false;
        return;
    }
    let before = live.0.snapshot();
    if let Some(action) = live.0.step_player(intent.0) {
        tape.0.push_live(action, &before, &live.0);
    }
    intent.0.interact_pressed = false;
}

pub(crate) fn resolve_after_local_finish(
    time: Res<Time>,
    mut timer: ResMut<ResolutionTimer>,
    runtime: Res<MatchRuntime>,
    mut live: ResMut<LiveMatch>,
    mut tape: ResMut<LiveTape>,
) {
    if runtime.mode != MatchMode::Live || live.0.competitive.finished {
        timer.0.reset();
        return;
    }
    let local_active = live
        .0
        .competitive
        .team(live.0.local_team)
        .is_some_and(|team| team.active_runner());
    if local_active {
        timer.0.reset();
        return;
    }
    if timer.0.tick(time.delta()).just_finished() {
        let before = live.0.snapshot();
        live.0.apply_action(crate::model::LocalAction::Wait);
        tape.0
            .push_live(crate::model::LocalAction::Wait, &before, &live.0);
    }
}

pub(crate) fn handle_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<MatchRuntime>,
) {
    if keyboard.just_pressed(KeyCode::Tab) {
        runtime.tactical_view = !runtime.tactical_view;
    }
    if keyboard.just_pressed(KeyCode::KeyT) {
        runtime.mode = match runtime.mode {
            MatchMode::Live => {
                runtime.tactical_view = true;
                runtime.replay_cursor = 0.0;
                runtime.replay_playing = true;
                MatchMode::Replay
            }
            MatchMode::Replay => MatchMode::Live,
        };
    }
    if runtime.mode == MatchMode::Replay {
        if keyboard.just_pressed(KeyCode::Space) {
            runtime.replay_playing = !runtime.replay_playing;
        }
        if keyboard.just_pressed(KeyCode::ArrowRight) {
            runtime.replay_cursor = runtime.replay_cursor.floor() + 1.0;
            runtime.replay_playing = false;
        }
        if keyboard.just_pressed(KeyCode::ArrowLeft) {
            runtime.replay_cursor = (runtime.replay_cursor.floor() - 1.0).max(0.0);
            runtime.replay_playing = false;
        }
        if keyboard.just_pressed(KeyCode::BracketRight) {
            runtime.replay_cursor += 5.0;
            runtime.replay_playing = false;
        }
        if keyboard.just_pressed(KeyCode::BracketLeft) {
            runtime.replay_cursor = (runtime.replay_cursor - 5.0).max(0.0);
            runtime.replay_playing = false;
        }
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
}

fn chosen_tape<'a>(live: &LiveMatch, live_tape: &'a LiveTape, demo: &'a DemoTape) -> &'a MatchTape {
    if live.0.competitive.finished && !live_tape.0.is_empty() {
        &live_tape.0
    } else {
        &demo.0
    }
}

pub(crate) fn advance_replay(
    time: Res<Time>,
    live: Res<LiveMatch>,
    live_tape: Res<LiveTape>,
    demo: Res<DemoTape>,
    mut runtime: ResMut<MatchRuntime>,
) {
    let tape = chosen_tape(&live, &live_tape, &demo);
    let end = tape.len() as f32;
    runtime.replay_cursor = runtime.replay_cursor.min(end);
    if runtime.mode == MatchMode::Replay && runtime.replay_playing {
        runtime.replay_cursor =
            (runtime.replay_cursor + runtime.replay_speed * time.delta_secs()).min(end);
        if runtime.replay_cursor >= end {
            runtime.replay_playing = false;
        }
    }
    runtime.replay_verified = tape
        .snapshots
        .last()
        .is_some_and(|expected| tape.replay_to(tape.len()).snapshot() == *expected);
}

pub(crate) fn update_view(
    live: Res<LiveMatch>,
    live_tape: Res<LiveTape>,
    demo: Res<DemoTape>,
    runtime: Res<MatchRuntime>,
    mut view: ResMut<ActiveView>,
) {
    view.0 = match runtime.mode {
        MatchMode::Live => live.0.clone(),
        MatchMode::Replay => {
            chosen_tape(&live, &live_tape, &demo).replay_to(runtime.replay_cursor.floor() as usize)
        }
    };
}

pub(crate) fn perform_reset(
    mut live: ResMut<LiveMatch>,
    mut tape: ResMut<LiveTape>,
    mut runtime: ResMut<MatchRuntime>,
) {
    if !runtime.reset_requested {
        return;
    }
    let resets = runtime.reset_count + 1;
    let camera_override = runtime.camera_override.take();
    live.0 = FirstPersonMatch::default();
    tape.0 = MatchTape::default();
    *runtime = MatchRuntime::default();
    runtime.reset_count = resets;
    runtime.camera_override = camera_override;
}

pub(crate) fn sync_door_panels(
    mut commands: Commands,
    assets: Res<VisualAssets>,
    view: Res<ActiveView>,
    panels: Query<(Entity, &DoorPanel)>,
) {
    let mut present = BTreeMap::new();
    for (entity, panel) in &panels {
        if view.0.facility.graph.is_sealed(panel.0) {
            present.insert(panel.0, entity);
        } else {
            commands.entity(entity).despawn();
        }
    }
    for index in 0..DOOR_COUNT {
        let door = DoorId(index as u16);
        if view.0.facility.graph.is_sealed(door) && !present.contains_key(&door) {
            spawn_door_panel(&mut commands, &assets, &view.0, door);
        }
    }
}

pub(crate) fn present_teams(
    view: Res<ActiveView>,
    mut markers: Query<(&TeamMarker, &mut Transform, &mut Visibility)>,
) {
    for (marker, mut transform, mut visibility) in &mut markers {
        let team = &view.0.competitive.teams[marker.0];
        let room = view.0.competitive.team_room(marker.0);
        let centre = module_center(room);
        transform.translation = centre + TEAM_OFFSETS[marker.0] + Vec3::Y * 1.2;
        *visibility = Visibility::Visible;
        // Absorbed teams sit low (director agents); active runners stand tall.
        transform.scale.y = if team.role == Role::Director {
            1.0
        } else {
            2.4
        };
    }
}

pub(crate) fn present_camera(
    view: Res<ActiveView>,
    runtime: Res<MatchRuntime>,
    mut camera: Single<&mut Transform, With<MatchCamera>>,
) {
    if let Some(pose) = runtime.camera_override {
        **camera = pose;
        return;
    }
    if runtime.tactical_view || runtime.mode == MatchMode::Replay {
        **camera =
            Transform::from_xyz(37.0, 47.0, 39.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
    } else {
        **camera = Transform::from_translation(view.0.facility.body.eye(&view.0.facility.config))
            .looking_to(view.0.facility.body.look_dir(), Vec3::Y);
    }
}

pub(crate) fn draw_debug(
    view: Res<ActiveView>,
    live: Res<LiveMatch>,
    live_tape: Res<LiveTape>,
    demo: Res<DemoTape>,
    runtime: Res<MatchRuntime>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }
    let session = &view.0;
    let graph = &session.competitive.structure.graph;

    // The match_replay schematic, promoted into the 3D world above the modules.
    for room_index in 0..ROOM_COUNT {
        let room = RoomId(room_index as u32);
        let centre = module_center(room) + Vec3::Y * 7.5;
        let color = if graph.observed(room) { TARGET } else { WALL };
        let h = MODULE_HALF * 0.82;
        gizmos.linestrip(
            [
                centre + Vec3::new(-h, 0.0, -h),
                centre + Vec3::new(h, 0.0, -h),
                centre + Vec3::new(h, 0.0, h),
                centre + Vec3::new(-h, 0.0, h),
                centre + Vec3::new(-h, 0.0, -h),
            ],
            color,
        );
    }
    for connection in &session.facility.projection.connections {
        let a_module = session.facility.module(connection.a.room);
        let b_module = session.facility.module(connection.b.room);
        let a_def = session.facility.registry.load(a_module.template).unwrap();
        let b_def = session.facility.registry.load(b_module.template).unwrap();
        let a = world_port(a_module, a_def, connection.a.port).unwrap();
        let b = world_port(b_module, b_def, connection.b.port).unwrap();
        let door = graph.door_id(a.reference.room, a.facing);
        gizmos.line(
            Vec3::new(a.position.x, 7.5, a.position.z),
            Vec3::new(b.position.x, 7.5, b.position.z),
            if session.competitive.structure.is_protected(door) {
                SPINE
            } else {
                OPEN
            },
        );
    }
    for room in session.competitive.collapse_rooms() {
        let centre = module_center(room) + Vec3::Y * 7.7;
        gizmos.circle(
            Isometry3d::new(centre, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            MODULE_HALF * 0.72,
            COLLAPSE,
        );
    }
    let exit = module_center(RoomId(EXIT_ROOM)) + Vec3::Y * 7.7;
    gizmos.circle(
        Isometry3d::new(exit, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        MODULE_HALF * 0.9,
        TARGET,
    );

    if let Some((_, side)) = session.target() {
        let reference = session
            .facility
            .projection
            .port_for_door(graph.door_id(session.local_room(), side));
        let module = session.facility.module(reference.room);
        let definition = session.facility.registry.load(module.template).unwrap();
        let port = world_port(module, definition, reference.port).unwrap();
        gizmos.line(
            Vec3::new(port.position.x, 0.1, port.position.z),
            Vec3::new(port.position.x, 5.8, port.position.z),
            TARGET,
        );
    }

    // Replay scrubber in world space below the front row.
    let tape = chosen_tape(&live, &live_tape, &demo);
    let z = MODULE_SPACING * 1.9;
    let x0 = -MODULE_SPACING;
    let x1 = MODULE_SPACING;
    let fraction = if tape.is_empty() {
        0.0
    } else {
        (runtime.replay_cursor / tape.len() as f32).clamp(0.0, 1.0)
    };
    gizmos.line(Vec3::new(x0, 0.3, z), Vec3::new(x1, 0.3, z), WALL);
    gizmos.line(
        Vec3::new(x0, 0.35, z),
        Vec3::new(x0 + (x1 - x0) * fraction, 0.35, z),
        OPEN,
    );
    for marker in &tape.markers {
        let x = x0 + (x1 - x0) * marker.round as f32 / tape.len().max(1) as f32;
        gizmos.line(Vec3::new(x, 0.1, z), Vec3::new(x, 0.9, z), SPINE);
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    view: Res<'w, ActiveView>,
    live: Res<'w, LiveMatch>,
    live_tape: Res<'w, LiveTape>,
    demo: Res<'w, DemoTape>,
    runtime: Res<'w, MatchRuntime>,
    cameras: Query<'w, 's, (), With<MatchCamera>>,
    ui_roots: Query<'w, 's, (), With<MatchUiRoot>>,
    modules: Query<'w, 's, (), With<MatchModuleRoot>>,
    teams: Query<'w, 's, (), With<TeamMarker>>,
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

    let session = &context.view.0;
    let tape = chosen_tape(&context.live, &context.live_tape, &context.demo);
    let mut standings = String::new();
    for team_id in session.competitive.standings() {
        let index = session
            .competitive
            .teams
            .iter()
            .position(|team| team.id == team_id)
            .unwrap();
        let team = &session.competitive.teams[index];
        let status = if team.placement.is_some() || team.role == Role::Director {
            team.status()
        } else {
            format!("{:.0}%", session.competitive.team_progress(index) * 100.0)
        };
        standings.push_str(&format!("  {} {}\n", team_id.label(), status));
    }
    let target = session.target().map_or_else(
        || "exit reached".to_string(),
        |(room, side)| format!("{} -> room {}", side.label(), room.0),
    );
    let cameras = context.cameras.iter().count();
    let ui = context.ui_roots.iter().count();
    let modules = context.modules.iter().count();
    let teams = context.teams.iter().count();
    let healthy = cameras == 1
        && ui == 1
        && modules == ROOM_COUNT
        && teams == TEAM_COUNT
        && session.facility.projection_exact()
        && context.runtime.replay_verified;

    let mut text = context.text.into_inner();
    **text = format!(
        "FIRST-PERSON MATCH  {}\n\
         mode / camera    {:?} / {}\n\
         round            {}   replay {:.0}/{}\n\
         local room       {}  target {}\n\
         local progress   {:.0}%\n\
         match            {}\n\
         control holder   {}\n\
         collapse         {:.0}%  director strength {}\n\
         escaped {}  absorbed {}\n\
         replay exact     {}\n\
         live tape        {} frames / {} markers\n\
         graph projection {}\n\
         camera {} UI {} modules {} teams {} resets {}\n\n\
         standings:\n{}\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.runtime.mode,
        if context.runtime.tactical_view || context.runtime.mode == MatchMode::Replay {
            "TAC-MAP"
        } else {
            "FIRST PERSON"
        },
        session.competitive.round,
        context.runtime.replay_cursor,
        tape.len(),
        session.facility.player_room.0,
        target,
        session.local_progress() * 100.0,
        if session.competitive.finished {
            "FINISHED"
        } else {
            "racing"
        },
        session
            .competitive
            .control_holder
            .map_or_else(|| "none".to_string(), |team| team.label()),
        session.competitive.purge_line.max(0.0) * 100.0,
        session.competitive.director_strength(),
        session.competitive.escaped_count(),
        session.competitive.absorbed_count(),
        if context.runtime.replay_verified {
            "MATCH"
        } else {
            "MISMATCH"
        },
        context.live_tape.0.len(),
        context.live_tape.0.markers.len(),
        if session.facility.projection_exact() {
            "exact"
        } else {
            "FAILED"
        },
        cameras,
        ui,
        modules,
        teams,
        context.runtime.reset_count,
        standings.trim_end(),
        session.last_event,
    );
}

pub(crate) fn configure_capture(runtime: &mut MatchRuntime, tape: &MatchTape) {
    runtime.mode = MatchMode::Replay;
    runtime.tactical_view = true;
    runtime.replay_playing = false;
    runtime.replay_cursor = (tape.len() / 2) as f32;
    runtime.replay_verified = tape
        .snapshots
        .last()
        .is_some_and(|expected| tape.replay_to(tape.len()).snapshot() == *expected);
    runtime.camera_override =
        Some(Transform::from_xyz(37.0, 47.0, 39.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y));
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
