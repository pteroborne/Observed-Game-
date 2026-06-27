use std::f32::consts::FRAC_PI_2;

use bevy::{
    ecs::system::SystemParam,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use observation_lab::model::{DOOR_COUNT, DoorId, ROOM_COUNT, Side};
use observed_core::RoomId;

use crate::vision::{
    EYE_HEIGHT, FOV_HALF_DEG, ROOM_HALF, WALL_HEIGHT, door_position, forward_from_yaw, room_center,
    side_direction, visible_rooms,
};

const SEEN: Color = Color::srgb(0.40, 1.0, 0.60);
const FREE: Color = Color::srgb(0.30, 0.80, 1.0);
const WALL: Color = Color::srgb(0.30, 0.34, 0.42);
const GAZE: Color = Color::srgb(0.95, 0.97, 1.0);

/// Downward tilt of the first-person camera so the floor wireframe is in frame.
const PITCH: f32 = 0.62;
const TURN_RATE: f32 = 1.9; // rad/s
const MOUSE_YAW_SENS: f32 = 0.003; // radians of yaw per pixel of mouse motion

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

#[derive(Resource)]
pub struct Observatory {
    pub graph: observation_lab::model::ObservationWorld,
    pub player: RoomId,
    pub yaw: f32,
    pub auto: bool,
    pub debug_visible: bool,
    pub decohere_count: u32,
    pub reset_count: u32,
    pub reset_requested: bool,
    pub decohere_requested: bool,
    pub visible: Vec<RoomId>,
    pub last_event: String,
    /// When set, the camera uses this fixed pose instead of the first-person one.
    /// Used only by the evidence capture to show the whole facility from above; the
    /// observed set is still computed from the first-person `player`/`yaw`.
    pub camera_override: Option<Transform>,
}

impl Default for Observatory {
    fn default() -> Self {
        let graph = observation_lab::model::ObservationWorld::authored();
        let mut me = Self {
            graph,
            player: RoomId(0),
            yaw: FRAC_PI_2, // look east, down the top row's corridor
            auto: true,
            debug_visible: true,
            decohere_count: 0,
            reset_count: 0,
            reset_requested: false,
            decohere_requested: false,
            visible: Vec::new(),
            last_event: "Look around — what you see freezes; the rest rewires.".to_string(),
            camera_override: None,
        };
        me.recompute_visibility();
        me
    }
}

impl Observatory {
    pub fn recompute_visibility(&mut self) {
        let forward = forward_from_yaw(self.yaw);
        self.visible = visible_rooms(&self.graph, self.player, forward);
        // Drive the proven observation machinery from line-of-sight: the rooms we
        // see become the observers, so their connections pin and freeze.
        self.graph.players = self.visible.clone();
    }

    pub fn decohere(&mut self) {
        self.graph.decohere();
        self.decohere_count += 1;
        self.last_event = format!(
            "Decohered: {} doors rewired (seen rooms held).",
            self.graph.rewires_last
        );
    }

    fn reset(&mut self) {
        let count = self.reset_count + 1;
        *self = Self::default();
        self.reset_count = count;
    }

    /// Walk through the open doorway most aligned with where we are looking,
    /// following its current graph link.
    fn step_forward(&mut self) {
        let forward = forward_from_yaw(self.yaw);
        let cos_half = FOV_HALF_DEG.to_radians().cos();
        let mut best: Option<RoomId> = None;
        let mut best_dot = cos_half;
        for side in Side::ALL {
            let door = self.graph.door_id(self.player, side);
            if self.graph.is_sealed(door) {
                continue;
            }
            let dot = side_direction(side).dot(forward);
            if dot >= best_dot {
                best_dot = dot;
                best = Some(self.graph.door(self.graph.partner(door)).room);
            }
        }
        if let Some(dest) = best {
            self.player = dest;
            self.last_event = format!("Stepped through to room {}.", dest.0);
        }
    }
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands.spawn((
        PlayerCam,
        Camera3d::default(),
        Transform::from_translation(room_center(RoomId(0)) + Vec3::Y * EYE_HEIGHT),
        Name::new("First-Person Camera"),
    ));

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            FpsUiRoot,
            Name::new("FPS Observation UI Root"),
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
                    width: px(450),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.5, 1.0, 0.7, 0.6)),
                children![(
                    DebugText,
                    Text::new("Vision diagnostics starting…"),
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
                BorderColor::all(Color::srgba(0.5, 1.0, 0.7, 0.6)),
                children![(
                    Text::new(
                        "FPS OBSERVATION LAB (feasibility)\n\
                         ← / → or A/D   Look around\n\
                         W / ↑          Step through the door you face\n\
                         Space          Decohere now\n\
                         P              Toggle auto-decoherence\n\
                         R reset · F1 debug\n\n\
                         Observation is driven by the first-person camera: the rooms\n\
                         in your line of sight (green) freeze their connections, while\n\
                         everything you are not looking at (cyan) rewires on each\n\
                         decoherence. Looking through a doorway follows its CURRENT\n\
                         link, so what you see — and freeze — depends on where you aim.",
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

pub(crate) fn control(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut obs: ResMut<Observatory>,
) {
    let dt = time.delta_secs();
    // Mouse look (primary) turns the observer; arrow/A-D keys remain a fallback.
    if let Some(mouse) = mouse_motion {
        obs.yaw += mouse.delta.x * MOUSE_YAW_SENS;
    }
    if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
        obs.yaw -= TURN_RATE * dt;
    }
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
        obs.yaw += TURN_RATE * dt;
    }
    if keyboard.just_pressed(KeyCode::KeyW) || keyboard.just_pressed(KeyCode::ArrowUp) {
        obs.step_forward();
    }
    if keyboard.just_pressed(KeyCode::Space) {
        obs.decohere_requested = true;
    }
    if keyboard.just_pressed(KeyCode::KeyP) {
        obs.auto = !obs.auto;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        obs.reset_requested = true;
    }
    if keyboard.just_pressed(KeyCode::F1) {
        obs.debug_visible = !obs.debug_visible;
    }
}

pub(crate) fn perform_reset(mut obs: ResMut<Observatory>) {
    if obs.reset_requested {
        obs.reset();
    }
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

/// Recompute what the camera sees and write it into the graph's observer set, so
/// the proven pinning/decoherence treats line-of-sight as observation.
pub(crate) fn update_observation(mut obs: ResMut<Observatory>) {
    obs.recompute_visibility();
}

pub(crate) fn decohere_step(
    time: Res<Time>,
    mut timer: ResMut<DecohereTimer>,
    mut obs: ResMut<Observatory>,
) {
    let manual = obs.decohere_requested;
    obs.decohere_requested = false;
    let auto = obs.auto && timer.0.tick(time.delta()).just_finished();
    if manual || auto {
        obs.decohere();
    }
}

#[derive(Resource)]
pub struct DecohereTimer(pub Timer);

impl Default for DecohereTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.6, TimerMode::Repeating))
    }
}

pub(crate) fn present_camera(
    obs: Res<Observatory>,
    mut camera: Single<&mut Transform, With<PlayerCam>>,
) {
    if let Some(pose) = obs.camera_override {
        **camera = pose;
        return;
    }
    let eye = room_center(obs.player) + Vec3::Y * EYE_HEIGHT;
    let forward = forward_from_yaw(obs.yaw);
    let target = eye + Vec3::new(forward.x, -PITCH, forward.z);
    **camera = Transform::from_translation(eye).looking_at(target, Vec3::Y);
}

fn floor_square(gizmos: &mut Gizmos, center: Vec3, half: f32, color: Color) {
    let y = center.y + 0.02;
    let corner = |dx: f32, dz: f32| Vec3::new(center.x + dx, y, center.z + dz);
    gizmos.linestrip(
        [
            corner(-half, -half),
            corner(half, -half),
            corner(half, half),
            corner(-half, half),
            corner(-half, -half),
        ],
        color,
    );
}

pub(crate) fn draw_debug(obs: Res<Observatory>, mut gizmos: Gizmos) {
    if !obs.debug_visible {
        return;
    }
    let graph = &obs.graph;

    // Room floors + corner posts + ceiling: green when seen (frozen), grey
    // otherwise. The posts give the wireframe real volume in first person.
    for index in 0..ROOM_COUNT {
        let room = RoomId(index as u32);
        let seen = obs.visible.contains(&room);
        let center = room_center(room);
        let color = if seen { SEEN } else { WALL };
        floor_square(&mut gizmos, center, ROOM_HALF, color);
        floor_square(
            &mut gizmos,
            center + Vec3::Y * WALL_HEIGHT,
            ROOM_HALF,
            color,
        );
        for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let base = center + Vec3::new(dx * ROOM_HALF, 0.0, dz * ROOM_HALF);
            gizmos.line(base, base + Vec3::Y * WALL_HEIGHT, color);
        }
        if seen {
            // A beam marks the rooms held by observation.
            gizmos.line(
                center + Vec3::Y * 0.05,
                center + Vec3::Y * (WALL_HEIGHT + 2.5),
                SEEN,
            );
        }
    }

    // Connections, following their current links; green if frozen by sight.
    for (a, b) in graph.connections() {
        let da = graph.door(a);
        let db = graph.door(b);
        let start = door_position(da.room, da.side) + Vec3::Y * 1.6;
        let end = door_position(db.room, db.side) + Vec3::Y * 1.6;
        let color = if graph.is_pinned(a) { SEEN } else { FREE };
        gizmos.line(start, end, color);
    }

    // Sealed walls: short ticks on the wall midpoints.
    for index in 0..DOOR_COUNT {
        let door = DoorId(index as u16);
        if graph.is_sealed(door) {
            let d = graph.door(door);
            let p = door_position(d.room, d.side);
            gizmos.line(p + Vec3::Y * 0.1, p + Vec3::Y * 1.2, WALL);
        }
    }

    // The gaze: a line on the floor from the player along the facing direction.
    let here = room_center(obs.player) + Vec3::Y * 0.06;
    let forward = forward_from_yaw(obs.yaw);
    gizmos.line(here, here + forward * (ROOM_HALF * 1.6), GAZE);
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    obs: Res<'w, Observatory>,
    cams: Query<'w, 's, (), With<PlayerCam>>,
    ui_roots: Query<'w, 's, (), With<FpsUiRoot>>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    panel: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
}

pub(crate) fn update_debug_text(mut context: DebugContext) {
    let visibility = if context.obs.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.panel = visibility;
    **context.help = visibility;

    let obs = &*context.obs;
    let seen: Vec<u32> = obs.visible.iter().map(|r| r.0).collect();
    let frozen = (0..DOOR_COUNT)
        .filter(|i| obs.graph.is_pinned(DoorId(*i as u16)))
        .count();
    let free = DOOR_COUNT - frozen;

    let cams = context.cams.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = cams == 1 && ui_roots == 1;

    let facing = {
        let f = forward_from_yaw(obs.yaw);
        // Nearest cardinal label for readability.
        let cards = [
            (Vec3::NEG_Z, "N"),
            (Vec3::X, "E"),
            (Vec3::Z, "S"),
            (Vec3::NEG_X, "W"),
        ];
        cards
            .iter()
            .max_by(|a, b| a.0.dot(f).total_cmp(&b.0.dot(f)))
            .map(|(_, l)| *l)
            .unwrap_or("?")
    };

    let mut text = context.text.into_inner();
    **text = format!(
        "FPS OBSERVATION  {}\n\
         player room     {}\n\
         facing          {} ({:.0}°)\n\
         rooms in sight  {:?}\n\
         frozen doors    {} / {}\n\
         free doors      {}\n\
         decoherences    {}\n\
         auto            {}\n\
         camera {cams}  UI {ui_roots}   resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        obs.player.0,
        facing,
        obs.yaw.to_degrees().rem_euclid(360.0),
        seen,
        frozen,
        DOOR_COUNT,
        free,
        obs.decohere_count,
        if obs.auto { "on" } else { "off" },
        obs.reset_count,
        obs.last_event,
    );
}
