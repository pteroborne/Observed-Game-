use std::collections::BTreeMap;

use bevy::{
    ecs::system::SystemParam,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use fps_visibility_lab::field::{EYE_HEIGHT, FOV_HALF_DEG, GAP_HALF, HALF, forward, room_center};
use observed_core::RoomId;
use player_input::PlayerIntent;

/// Mouse-look sensitivity into the rate-based turn intent (`look.x`).
const MOUSE_TURN_SENS: f32 = 0.1;

use crate::model::{GATEWAY_COUNT, GatewayId, ModuleId, RewireStage, TRANSIT_SECONDS};

const HUB_FLOOR: Color = Color::srgb(0.08, 0.10, 0.15);
const HUB_WALL: Color = Color::srgb(0.32, 0.37, 0.48);
const VISIBLE: Color = Color::srgb(1.0, 0.76, 0.26);
const HIDDEN: Color = Color::srgb(0.25, 0.78, 1.0);
const PENDING: Color = Color::srgb(1.0, 0.36, 0.82);
const SAFE: Color = Color::srgb(0.36, 1.0, 0.58);
const WALL_HEIGHT: f32 = 3.4;

#[derive(Component)]
pub(crate) struct PlayerCam;

#[derive(Component)]
pub(crate) struct RewireUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ModuleVisualRoot {
    pub gateway: GatewayId,
    pub revision: u32,
    pub module: ModuleId,
}

#[derive(Component)]
struct StaticGeometry;

#[derive(Resource, Default)]
pub(crate) struct CameraIntent(pub PlayerIntent);

#[derive(Resource)]
pub struct RewireRuntime {
    pub auto: bool,
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub camera_override: Option<Transform>,
}

impl Default for RewireRuntime {
    fn default() -> Self {
        Self {
            auto: true,
            debug_visible: true,
            reset_requested: false,
            camera_override: None,
        }
    }
}

#[derive(Resource)]
pub(crate) struct RewireTimer(Timer);

impl Default for RewireTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(2.2, TimerMode::Repeating))
    }
}

#[derive(Resource, Clone)]
pub(crate) struct VisualAssets {
    cube: Handle<Mesh>,
    hub_floor: Handle<StandardMaterial>,
    hub_wall: Handle<StandardMaterial>,
    modules: [Handle<StandardMaterial>; GATEWAY_COUNT],
}

pub(crate) fn setup_lab(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stage: Res<RewireStage>,
) {
    let assets = VisualAssets {
        cube: meshes.add(Cuboid::from_length(1.0)),
        hub_floor: materials.add(HUB_FLOOR),
        hub_wall: materials.add(HUB_WALL),
        modules: [
            materials.add(Color::srgb(1.0, 0.44, 0.12)),
            materials.add(Color::srgb(0.10, 0.78, 1.0)),
            materials.add(Color::srgb(0.95, 0.20, 0.75)),
            materials.add(Color::srgb(0.18, 0.92, 0.42)),
        ],
    };

    commands.spawn((
        PlayerCam,
        Camera3d::default(),
        Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
        Name::new("Phase 22 First-Person Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.7, 0.0)),
        Name::new("Rewire Lab Sun"),
    ));

    spawn_box(
        &mut commands,
        &assets,
        assets.hub_floor.clone(),
        Vec3::new(0.0, -0.12, 0.0),
        Vec3::new(8.0, 0.22, 8.0),
        "Hub floor",
    );
    spawn_hub_walls(&mut commands, &assets);
    for gateway in GatewayId::ALL {
        spawn_module(&mut commands, &assets, stage.gateway(gateway));
    }
    spawn_ui(&mut commands);
    commands.insert_resource(assets);
}

fn spawn_box(
    commands: &mut Commands,
    assets: &VisualAssets,
    material: Handle<StandardMaterial>,
    position: Vec3,
    size: Vec3,
    name: &'static str,
) {
    commands.spawn((
        StaticGeometry,
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position).with_scale(size),
        Name::new(name),
    ));
}

fn spawn_hub_walls(commands: &mut Commands, assets: &VisualAssets) {
    let segment = HALF - GAP_HALF;
    let offset = (HALF + GAP_HALF) * 0.5;
    let y = WALL_HEIGHT * 0.5;
    for z in [-HALF, HALF] {
        for x in [-offset, offset] {
            spawn_box(
                commands,
                assets,
                assets.hub_wall.clone(),
                Vec3::new(x, y, z),
                Vec3::new(segment, WALL_HEIGHT, 0.22),
                "Hub north/south wall",
            );
        }
    }
    for x in [-HALF, HALF] {
        for z in [-offset, offset] {
            spawn_box(
                commands,
                assets,
                assets.hub_wall.clone(),
                Vec3::new(x, y, z),
                Vec3::new(0.22, WALL_HEIGHT, segment),
                "Hub east/west wall",
            );
        }
    }
}

fn spawn_module(
    commands: &mut Commands,
    assets: &VisualAssets,
    gateway: &crate::model::GatewayState,
) {
    let portal = RewireStage::portal_position(gateway.id);
    let direction = fps_visibility_lab::field::side_dir(gateway.id.side());
    let transform = Transform::from_translation(Vec3::new(portal.x, 0.0, portal.y))
        .looking_to(Vec3::new(direction.x, 0.0, direction.y), Vec3::Y);
    let material = assets.modules[gateway.displayed.0 as usize].clone();

    commands
        .spawn((
            ModuleVisualRoot {
                gateway: gateway.id,
                revision: gateway.revision,
                module: gateway.displayed,
            },
            transform,
            Visibility::default(),
            Name::new(format!(
                "{} portal - {} r{}",
                gateway.id.label(),
                gateway.displayed.label(),
                gateway.revision
            )),
        ))
        .with_children(|root| {
            module_box(
                root,
                assets,
                material.clone(),
                Vec3::new(0.0, -0.08, -5.0),
                Vec3::new(5.2, 0.16, 10.0),
            );
            for x in [-2.55, 2.55] {
                module_box(
                    root,
                    assets,
                    material.clone(),
                    Vec3::new(x, 1.7, -5.0),
                    Vec3::new(0.18, 3.4, 10.0),
                );
            }
            module_box(
                root,
                assets,
                material.clone(),
                Vec3::new(0.0, 1.7, -10.0),
                Vec3::new(5.2, 3.4, 0.18),
            );

            match gateway.displayed {
                ModuleId::AMBER_COLUMNS => {
                    for x in [-1.25, 1.25] {
                        module_box(
                            root,
                            assets,
                            material.clone(),
                            Vec3::new(x, 1.35, -5.4),
                            Vec3::new(0.55, 2.7, 0.55),
                        );
                    }
                }
                ModuleId::CYAN_CROSSBEAM => {
                    module_box(
                        root,
                        assets,
                        material.clone(),
                        Vec3::new(0.0, 2.35, -5.5),
                        Vec3::new(4.4, 0.45, 0.5),
                    );
                    module_box(
                        root,
                        assets,
                        material.clone(),
                        Vec3::new(-1.55, 1.1, -7.2),
                        Vec3::new(0.45, 2.2, 0.45),
                    );
                }
                ModuleId::MAGENTA_STEPS => {
                    for index in 0..4 {
                        let h = 0.22 + index as f32 * 0.22;
                        module_box(
                            root,
                            assets,
                            material.clone(),
                            Vec3::new(0.0, h * 0.5, -4.0 - index as f32 * 1.05),
                            Vec3::new(3.8, h, 1.0),
                        );
                    }
                }
                ModuleId::GREEN_ARCH => {
                    for x in [-1.65, 1.65] {
                        module_box(
                            root,
                            assets,
                            material.clone(),
                            Vec3::new(x, 1.3, -5.5),
                            Vec3::new(0.5, 2.6, 0.5),
                        );
                    }
                    module_box(
                        root,
                        assets,
                        material,
                        Vec3::new(0.0, 2.55, -5.5),
                        Vec3::new(3.8, 0.45, 0.55),
                    );
                }
                _ => {}
            }
        });
}

fn module_box(
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

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            RewireUiRoot,
            Name::new("FPS Rewire UI Root"),
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
                    width: px(500),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.94)),
                BorderColor::all(Color::srgba(0.45, 1.0, 0.68, 0.65)),
                children![(
                    DebugText,
                    Text::new("Rewire diagnostics starting..."),
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
                    width: px(450),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.94)),
                BorderColor::all(Color::srgba(0.45, 1.0, 0.68, 0.65)),
                children![(
                    Text::new(
                        "FPS REWIRE LAB (Phase 22)\n\
                         A / D or arrows   Turn\n\
                         Space             Request decoherence\n\
                         W                 Cross the doorway you face\n\
                         P                 Toggle automatic rewires\n\
                         R reset - F1 debug\n\n\
                         Gold portal = visible and frozen.\n\
                         Cyan portal = hidden and eligible.\n\
                         Magenta = an atomic batch is pending.\n\n\
                         3D module entities are replaced only after every portal in\n\
                         the batch is off-camera and doorway-clear. A crossing pins\n\
                         its old rendered destination until arrival.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.96, 0.94)),
                )],
            ));
        });
}

pub(crate) fn map_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut intent: ResMut<CameraIntent>,
) {
    let axis = |negative: KeyCode, positive: KeyCode| {
        (keyboard.pressed(positive) as i32 - keyboard.pressed(negative) as i32) as f32
    };
    let mouse = mouse_motion.map(|m| m.delta).unwrap_or(Vec2::ZERO);
    // Turn from the mouse (primary) plus arrow/A-D keys; left unclamped.
    intent.0 = PlayerIntent {
        look: Vec2::new(
            axis(KeyCode::ArrowLeft, KeyCode::ArrowRight)
                + axis(KeyCode::KeyA, KeyCode::KeyD)
                + mouse.x * MOUSE_TURN_SENS,
            0.0,
        ),
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
    mut stage: ResMut<RewireStage>,
    mut runtime: ResMut<RewireRuntime>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        stage.request_rewire();
    }
    if keyboard.just_pressed(KeyCode::KeyW) {
        stage.begin_faced_transit();
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        runtime.auto = !runtime.auto;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        runtime.debug_visible = !runtime.debug_visible;
    }
}

pub(crate) fn simulate(
    time: Res<Time<Fixed>>,
    intent: Res<CameraIntent>,
    mut stage: ResMut<RewireStage>,
) {
    stage.advance_camera(intent.0, time.delta_secs());
}

pub(crate) fn perform_reset(
    mut stage: ResMut<RewireStage>,
    mut runtime: ResMut<RewireRuntime>,
    mut timer: ResMut<RewireTimer>,
) {
    if !runtime.reset_requested {
        return;
    }
    stage.reset();
    let camera_override = runtime.camera_override.take();
    *runtime = RewireRuntime::default();
    runtime.camera_override = camera_override;
    timer.0.reset();
}

pub(crate) fn auto_request(
    time: Res<Time>,
    mut timer: ResMut<RewireTimer>,
    runtime: Res<RewireRuntime>,
    mut stage: ResMut<RewireStage>,
) {
    if runtime.auto && timer.0.tick(time.delta()).just_finished() {
        stage.request_rewire();
    }
}

pub(crate) fn commit_safe_swaps(mut stage: ResMut<RewireStage>) {
    stage.commit_pending();
}

pub(crate) fn sync_module_visuals(
    mut commands: Commands,
    assets: Res<VisualAssets>,
    stage: Res<RewireStage>,
    roots: Query<(Entity, &ModuleVisualRoot)>,
) {
    let mut current = BTreeMap::new();
    for (entity, root) in &roots {
        let expected = stage.gateway(root.gateway);
        if root.revision == expected.revision && root.module == expected.displayed {
            current.insert(root.gateway, entity);
        } else {
            commands.entity(entity).despawn();
        }
    }
    for gateway in GatewayId::ALL {
        if !current.contains_key(&gateway) {
            spawn_module(&mut commands, &assets, stage.gateway(gateway));
        }
    }
}

pub(crate) fn present_camera(
    stage: Res<RewireStage>,
    runtime: Res<RewireRuntime>,
    mut camera: Single<&mut Transform, With<PlayerCam>>,
) {
    if let Some(pose) = runtime.camera_override {
        **camera = pose;
        return;
    }
    let eye = Vec3::new(stage.vision.eye.x, EYE_HEIGHT, stage.vision.eye.y);
    let facing = forward(stage.vision.yaw);
    **camera =
        Transform::from_translation(eye).looking_to(Vec3::new(facing.x, -0.12, facing.y), Vec3::Y);
}

pub(crate) fn draw_debug(stage: Res<RewireStage>, runtime: Res<RewireRuntime>, mut gizmos: Gizmos) {
    if !runtime.debug_visible {
        return;
    }

    let pending = stage.pending.as_ref();
    for gateway in GatewayId::ALL {
        let p = RewireStage::portal_position(gateway);
        let is_pending = pending
            .is_some_and(|batch| batch.changes.iter().any(|change| change.gateway == gateway));
        let color = if is_pending {
            PENDING
        } else if stage.portal_visible(gateway) {
            VISIBLE
        } else {
            HIDDEN
        };
        gizmos.line(
            Vec3::new(p.x, 0.05, p.y),
            Vec3::new(p.x, WALL_HEIGHT + 0.9, p.y),
            color,
        );
        let direction = fps_visibility_lab::field::side_dir(gateway.side());
        let tangent = Vec2::new(-direction.y, direction.x);
        let a = p - tangent * GAP_HALF;
        let b = p + tangent * GAP_HALF;
        gizmos.line(Vec3::new(a.x, 0.06, a.y), Vec3::new(b.x, 0.06, b.y), color);
    }

    let eye = Vec3::new(stage.vision.eye.x, EYE_HEIGHT, stage.vision.eye.y);
    for angle in [
        stage.vision.yaw - FOV_HALF_DEG.to_radians(),
        stage.vision.yaw + FOV_HALF_DEG.to_radians(),
    ] {
        let ray = forward(angle);
        gizmos.line(eye, eye + Vec3::new(ray.x, 0.0, ray.y) * 8.0, Color::WHITE);
    }

    if let Some(transit) = stage.transit {
        let centre = room_center(RoomId(4));
        let portal = RewireStage::portal_position(transit.gateway);
        gizmos.line(
            Vec3::new(centre.x, 0.25, centre.y),
            Vec3::new(portal.x, 0.25, portal.y),
            SAFE,
        );
        gizmos.sphere(
            Isometry3d::from_translation(Vec3::new(stage.vision.eye.x, 0.45, stage.vision.eye.y)),
            0.28,
            SAFE,
        );
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    stage: Res<'w, RewireStage>,
    runtime: Res<'w, RewireRuntime>,
    cameras: Query<'w, 's, (), With<PlayerCam>>,
    ui_roots: Query<'w, 's, (), With<RewireUiRoot>>,
    module_roots: Query<'w, 's, &'static ModuleVisualRoot>,
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

    let routes = GatewayId::ALL
        .into_iter()
        .map(|gateway| {
            let state = context.stage.gateway(gateway);
            format!(
                "{}:{} r{}{}",
                gateway.label(),
                state.displayed.0,
                state.revision,
                if context.stage.portal_visible(gateway) {
                    "*"
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>()
        .join("  ");
    let pending = context.stage.pending.as_ref().map_or_else(
        || "none".to_string(),
        |batch| {
            let ids = batch
                .changes
                .iter()
                .map(|change| change.gateway.label())
                .collect::<Vec<_>>()
                .join(",");
            format!("batch {} [{}]", batch.id, ids)
        },
    );
    let transit = context.stage.transit.map_or_else(
        || "clear".to_string(),
        |transit| {
            format!(
                "{} -> {} ({:.0}%)",
                transit.gateway.label(),
                transit.destination.label(),
                transit.progress * 100.0
            )
        },
    );
    let cameras = context.cameras.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let module_roots = context.module_roots.iter().count();
    let healthy = cameras == 1
        && ui_roots == 1
        && module_roots == GATEWAY_COUNT
        && context.stage.seam_violations == 0;

    let mut text = context.text.into_inner();
    **text = format!(
        "REWIRE WHILE UNOBSERVED  {}\n\
         facing portal   {}\n\
         routes          {}\n\
         pending         {}\n\
         transit         {}\n\
         requests        {}\n\
         atomic commits  {}\n\
         visible swaps   {}\n\
         no-pop proof    {}\n\
         auto            {}\n\
         camera {}  UI {}  modules {}  resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.stage.faced_gateway().label(),
        routes,
        pending,
        transit,
        context.stage.request_count,
        context.stage.commit_count,
        context.stage.seam_violations,
        if context.stage.no_pop_proven() {
            "PASS"
        } else {
            "waiting for first safe commit"
        },
        if context.runtime.auto { "on" } else { "off" },
        cameras,
        ui_roots,
        module_roots,
        context.stage.reset_count,
        context.stage.last_event,
    );
}

pub(crate) fn stage_capture_showcase(stage: &mut RewireStage) {
    stage.set_facing(GatewayId::EAST);
    stage.set_facing(GatewayId::NORTH);
    stage.request_rewire();
    assert!(stage.commit_pending());
    stage.set_facing(GatewayId::EAST);

    // Also stage the anti-stranding half of the contract: a second batch includes
    // the east portal but is held while a player is midway through its old route.
    stage.set_facing(GatewayId::NORTH);
    stage.begin_transit(GatewayId::EAST);
    stage.request_rewire();
    stage.advance_transit(TRANSIT_SECONDS * 0.08);
    assert!(!stage.commit_pending());
}
