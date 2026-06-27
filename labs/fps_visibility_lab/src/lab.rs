use bevy::{
    ecs::system::SystemParam,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use observed_core::RoomId;
use observed_observation::{DOOR_COUNT, DoorId, ROOM_COUNT};
use player_input::PlayerIntent;

/// Mouse-look sensitivity into the rate-based turn intent (`look.x`).
const MOUSE_TURN_SENS: f32 = 0.1;

use crate::field::{
    CELLS, EYE_HEIGHT, FOV_HALF_DEG, GAP_HALF, ROOM_SIZE, Seg, VisionField, cell_center, door_pos,
    forward, is_interior, side_dir, walls,
};

const SEEN_CELL: Color = Color::srgb(0.35, 1.0, 0.58);
const UNSEEN_CELL: Color = Color::srgb(0.18, 0.23, 0.31);
const SEEN_LINK: Color = Color::srgb(1.0, 0.72, 0.28);
const FREE_LINK: Color = Color::srgb(0.26, 0.78, 1.0);
const WALL: Color = Color::srgb(0.58, 0.64, 0.74);
const FRUSTUM: Color = Color::srgb(0.95, 0.97, 1.0);
const WALL_HEIGHT: f32 = 3.4;

#[derive(Component)]
pub(crate) struct PlayerCam;

#[derive(Component)]
pub(crate) struct FpsUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Resource, Default)]
pub(crate) struct CameraIntent(pub PlayerIntent);

#[derive(Resource)]
pub struct VisibilityRuntime {
    pub auto: bool,
    pub debug_visible: bool,
    pub reset_count: u32,
    pub reset_requested: bool,
    pub decohere_requested: bool,
    pub camera_override: Option<Transform>,
}

impl Default for VisibilityRuntime {
    fn default() -> Self {
        Self {
            auto: true,
            debug_visible: true,
            reset_count: 0,
            reset_requested: false,
            decohere_requested: false,
            camera_override: None,
        }
    }
}

#[derive(Resource)]
pub(crate) struct DecohereTimer(Timer);

impl Default for DecohereTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.8, TimerMode::Repeating))
    }
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands.spawn((
        PlayerCam,
        Camera3d::default(),
        Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
        Name::new("Continuous Visibility Camera"),
    ));
    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            FpsUiRoot,
            Name::new("FPS Visibility UI Root"),
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
                    width: px(460),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.4, 1.0, 0.65, 0.65)),
                children![(
                    DebugText,
                    Text::new("Visibility diagnostics starting..."),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 1.0, 0.92)),
                )],
            ));
            root.spawn((
                HelpText,
                Node {
                    width: px(440),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.4, 1.0, 0.65, 0.65)),
                children![(
                    Text::new(
                        "FPS VISIBILITY LAB (Phase 21)\n\
                         A / D or arrows   Look left / right\n\
                         W / S             Move forward / back\n\
                         Q / E             Strafe left / right\n\
                         Space             Decohere now\n\
                         P                 Toggle auto-decoherence\n\
                         R reset - F1 debug\n\n\
                         Green floor cells are directly inside the camera frustum\n\
                         and not blocked by a wall. A room may be only partly green.\n\
                         Gold doorway links are frozen because an endpoint is seen;\n\
                         cyan links are unseen and may re-pair on decoherence.",
                    ),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.95, 0.92)),
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
    let mut movement = Vec2::new(
        axis(KeyCode::KeyQ, KeyCode::KeyE),
        axis(KeyCode::KeyS, KeyCode::KeyW),
    );
    if movement.length_squared() > 1.0 {
        movement = movement.normalize_or_zero();
    }
    // Turn from the mouse (primary) plus arrow/A-D keys; left unclamped so a flick
    // turns faster than holding a key.
    intent.0 = PlayerIntent {
        movement,
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

pub(crate) fn handle_toggles(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<VisibilityRuntime>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        runtime.decohere_requested = true;
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

pub(crate) fn advance_camera(
    time: Res<Time<Fixed>>,
    intent: Res<CameraIntent>,
    mut field: ResMut<VisionField>,
) {
    field.advance_camera(intent.0, time.delta_secs());
}

pub(crate) fn perform_reset(
    mut field: ResMut<VisionField>,
    mut runtime: ResMut<VisibilityRuntime>,
    mut timer: ResMut<DecohereTimer>,
) {
    if !runtime.reset_requested {
        return;
    }
    let count = runtime.reset_count + 1;
    field.reset();
    *runtime = VisibilityRuntime::default();
    runtime.reset_count = count;
    timer.0.reset();
}

pub(crate) fn decohere_step(
    time: Res<Time>,
    mut timer: ResMut<DecohereTimer>,
    mut runtime: ResMut<VisibilityRuntime>,
    mut field: ResMut<VisionField>,
) {
    let manual = runtime.decohere_requested;
    runtime.decohere_requested = false;
    let auto = runtime.auto && timer.0.tick(time.delta()).just_finished();
    if manual || auto {
        field.decohere();
    }
}

pub(crate) fn present_camera(
    field: Res<VisionField>,
    runtime: Res<VisibilityRuntime>,
    mut camera: Single<&mut Transform, With<PlayerCam>>,
) {
    if let Some(pose) = runtime.camera_override {
        **camera = pose;
        return;
    }
    let eye = Vec3::new(field.eye.x, EYE_HEIGHT, field.eye.y);
    let fwd = forward(field.yaw);
    **camera = Transform::from_translation(eye).looking_to(Vec3::new(fwd.x, -0.18, fwd.y), Vec3::Y);
}

fn draw_wall(gizmos: &mut Gizmos, segment: Seg) {
    let a = Vec3::new(segment.a.x, 0.02, segment.a.y);
    let b = Vec3::new(segment.b.x, 0.02, segment.b.y);
    gizmos.line(a, b, WALL);
    gizmos.line(a + Vec3::Y * WALL_HEIGHT, b + Vec3::Y * WALL_HEIGHT, WALL);
    gizmos.line(a, a + Vec3::Y * WALL_HEIGHT, WALL);
    gizmos.line(b, b + Vec3::Y * WALL_HEIGHT, WALL);
}

fn draw_cell(gizmos: &mut Gizmos, centre: Vec2, half: f32, color: Color) {
    let y = 0.04;
    let p = |x: f32, z: f32| Vec3::new(centre.x + x, y, centre.y + z);
    gizmos.linestrip(
        [
            p(-half, -half),
            p(half, -half),
            p(half, half),
            p(-half, half),
            p(-half, -half),
        ],
        color,
    );
    if color == SEEN_CELL {
        gizmos.line(
            Vec3::new(centre.x, y, centre.y),
            Vec3::new(centre.x, 0.55, centre.y),
            color,
        );
    }
}

pub(crate) fn draw_debug(
    field: Res<VisionField>,
    runtime: Res<VisibilityRuntime>,
    mut gizmos: Gizmos,
) {
    if !runtime.debug_visible {
        return;
    }

    let cell_half = ROOM_SIZE / CELLS as f32 * 0.43;
    for room_index in 0..ROOM_COUNT {
        let room = RoomId(room_index as u32);
        for cj in 0..CELLS {
            for ci in 0..CELLS {
                let index = crate::field::cell_index(room, ci, cj);
                let color = if field.seen_cells[index] {
                    SEEN_CELL
                } else {
                    UNSEEN_CELL
                };
                draw_cell(&mut gizmos, cell_center(room, ci, cj), cell_half, color);
            }
        }
    }

    for wall in walls() {
        draw_wall(&mut gizmos, wall);
    }

    for (a, b) in field.graph.connections() {
        let da = field.graph.door(a);
        let db = field.graph.door(b);
        if !is_interior(da.room, da.side) || !is_interior(db.room, db.side) {
            continue;
        }
        let pa = door_pos(da.room, da.side);
        let pb = door_pos(db.room, db.side);
        let start = Vec3::new(pa.x, WALL_HEIGHT + 0.7, pa.y);
        let end = Vec3::new(pb.x, WALL_HEIGHT + 0.7, pb.y);
        gizmos.line(
            start,
            end,
            if field.frozen(a) {
                SEEN_LINK
            } else {
                FREE_LINK
            },
        );
    }

    for index in 0..DOOR_COUNT {
        let door = DoorId(index as u16);
        let d = field.graph.door(door);
        if !is_interior(d.room, d.side) {
            continue;
        }
        let p = door_pos(d.room, d.side);
        let base = Vec3::new(p.x, 0.05, p.y);
        let color = if field.seen_doors[index] {
            SEEN_LINK
        } else {
            FREE_LINK
        };
        gizmos.line(base, base + Vec3::Y * 1.1, color);
    }

    let eye = Vec3::new(field.eye.x, EYE_HEIGHT, field.eye.y);
    let left = forward(field.yaw - FOV_HALF_DEG.to_radians());
    let right = forward(field.yaw + FOV_HALF_DEG.to_radians());
    let ray_length = ROOM_SIZE * 2.2;
    gizmos.line(
        eye,
        eye + Vec3::new(left.x, 0.0, left.y) * ray_length,
        FRUSTUM,
    );
    gizmos.line(
        eye,
        eye + Vec3::new(right.x, 0.0, right.y) * ray_length,
        FRUSTUM,
    );

    // Mark doorway apertures explicitly so the source of occlusion is readable.
    for room_index in 0..ROOM_COUNT {
        let room = RoomId(room_index as u32);
        for side in observed_observation::Side::ALL {
            if !is_interior(room, side) {
                continue;
            }
            let centre = door_pos(room, side);
            let direction = side_dir(side);
            let tangent = Vec2::new(-direction.y, direction.x);
            let a = centre - tangent * GAP_HALF;
            let b = centre + tangent * GAP_HALF;
            gizmos.line(
                Vec3::new(a.x, 0.08, a.y),
                Vec3::new(b.x, 0.08, b.y),
                Color::srgb(0.95, 0.48, 0.20),
            );
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    field: Res<'w, VisionField>,
    runtime: Res<'w, VisibilityRuntime>,
    cameras: Query<'w, 's, (), With<PlayerCam>>,
    ui_roots: Query<'w, 's, (), With<FpsUiRoot>>,
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

    let cameras = context.cameras.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = cameras == 1
        && ui_roots == 1
        && context.field.seen_cell_count() > 0
        && context.field.partially_seen_rooms() > 0;

    let coverage = (0..ROOM_COUNT)
        .filter_map(|index| {
            let room = RoomId(index as u32);
            let seen = context.field.room_seen_cell_count(room);
            (seen > 0).then(|| {
                format!(
                    "{}:{:.0}%",
                    index,
                    context.field.room_coverage(room) * 100.0
                )
            })
        })
        .collect::<Vec<_>>()
        .join("  ");

    let mut text = context.text.into_inner();
    **text = format!(
        "CONTINUOUS VISIBILITY  {}\n\
         eye             ({:.2}, {:.2})\n\
         yaw             {:.1} deg\n\
         visible cells   {} / {}\n\
         partial rooms   {}\n\
         visible doors   {} / {}\n\
         room coverage   {}\n\
         decoherences    {}\n\
         auto            {}\n\
         camera {}  UI {}  resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.field.eye.x,
        context.field.eye.y,
        context.field.yaw.to_degrees().rem_euclid(360.0),
        context.field.seen_cell_count(),
        crate::field::CELL_COUNT,
        context.field.partially_seen_rooms(),
        context.field.seen_door_count(),
        DOOR_COUNT,
        if coverage.is_empty() { "-" } else { &coverage },
        context.field.decohere_count,
        if context.runtime.auto { "on" } else { "off" },
        cameras,
        ui_roots,
        context.runtime.reset_count,
        context.field.last_event,
    );
}
