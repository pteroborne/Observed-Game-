use bevy::{
    ecs::system::SystemParam,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use player_input::PlayerIntent;

use crate::controller::{Aabb3, FIXED_DT, FpsArena, FpsBody, FpsConfig, run_path, step_body};

/// Mouse-look sensitivity: raw pixel delta scaled into `PlayerIntent.look` units.
const MOUSE_SENS: f32 = 0.06;

const LIVE: Color = Color::srgb(0.40, 1.0, 0.60);
const REPLAY: Color = Color::srgb(1.0, 0.45, 0.95);
const GRID: Color = Color::srgb(0.22, 0.26, 0.34);
const SOLID: Color = Color::srgb(0.55, 0.62, 0.74);
const MARK: Color = Color::srgb(0.95, 0.97, 1.0);

const SPAWN: Vec3 = Vec3::new(0.0, 0.9, 15.0);
const TAPE_CAP: usize = 6000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Live,
    Replay,
}

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
pub struct Stage {
    pub arena: FpsArena,
    pub config: FpsConfig,
    pub body: FpsBody,
    pub tape: Vec<PlayerIntent>,
    pub live_path: Vec<Vec3>,
    pub mode: Mode,
    pub ghost: FpsBody,
    pub replay_path: Vec<Vec3>,
    pub replay_cursor: usize,
    pub replay_match: Option<bool>,
    pub debug_visible: bool,
    pub reset_count: u32,
    pub last_event: String,
    /// Capture-only fixed camera; the live lab is first-person.
    pub camera_override: Option<Transform>,
}

impl Default for Stage {
    fn default() -> Self {
        Self {
            arena: FpsArena::authored(),
            config: FpsConfig::default(),
            body: FpsBody::spawned(SPAWN, 0.0),
            tape: Vec::new(),
            live_path: Vec::new(),
            mode: Mode::Live,
            ghost: FpsBody::spawned(SPAWN, 0.0),
            replay_path: Vec::new(),
            replay_cursor: 0,
            replay_match: None,
            debug_visible: true,
            reset_count: 0,
            last_event: "Walk to record a path, then press T to replay it.".to_string(),
            camera_override: None,
        }
    }
}

impl Stage {
    /// The body the camera currently follows.
    pub fn active(&self) -> &FpsBody {
        match self.mode {
            Mode::Live => &self.body,
            Mode::Replay => &self.ghost,
        }
    }

    /// Advance live play one fixed tick from an intent, recording it.
    pub fn step_live(&mut self, intent: PlayerIntent) {
        if self.tape.len() >= TAPE_CAP {
            return;
        }
        self.tape.push(intent);
        step_body(&mut self.body, intent, &self.arena, &self.config, FIXED_DT);
        self.live_path.push(self.body.position);
    }

    /// Start replaying the recorded tape on a fresh ghost body.
    pub fn begin_replay(&mut self) {
        if self.tape.is_empty() {
            self.last_event = "Nothing recorded yet — walk first.".to_string();
            return;
        }
        self.mode = Mode::Replay;
        self.ghost = FpsBody::spawned(SPAWN, 0.0);
        self.replay_path.clear();
        self.replay_cursor = 0;
        self.replay_match = None;
        self.last_event = "Replaying the recorded tape…".to_string();
    }

    /// Advance the replay one fixed tick; finalize the match check at the end.
    pub fn step_replay(&mut self) {
        if self.replay_cursor < self.tape.len() {
            let intent = self.tape[self.replay_cursor];
            step_body(&mut self.ghost, intent, &self.arena, &self.config, FIXED_DT);
            self.replay_path.push(self.ghost.position);
            self.replay_cursor += 1;
        }
        if self.replay_cursor == self.tape.len() && self.replay_match.is_none() {
            let matches = self.replay_path == self.live_path;
            self.replay_match = Some(matches);
            self.last_event = if matches {
                "Replay reproduced the path exactly.".to_string()
            } else {
                "Replay DIVERGED — non-deterministic!".to_string()
            };
        }
    }

    pub fn return_to_live(&mut self) {
        self.mode = Mode::Live;
        self.last_event = "Back to live control (tape kept).".to_string();
    }

    pub fn reset(&mut self) {
        let count = self.reset_count + 1;
        *self = Self::default();
        self.reset_count = count;
    }
}

pub(crate) fn setup_lab(mut commands: Commands) {
    commands.spawn((
        PlayerCam,
        Camera3d::default(),
        Transform::from_translation(SPAWN),
        Name::new("First-Person Camera"),
    ));

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            FpsUiRoot,
            Name::new("FPS Controller UI Root"),
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
                    width: px(440),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.02, 0.94)),
                BorderColor::all(Color::srgba(0.5, 1.0, 0.7, 0.6)),
                children![(
                    DebugText,
                    Text::new("Controller diagnostics starting…"),
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
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.025, 0.02, 0.94)),
                BorderColor::all(Color::srgba(0.5, 1.0, 0.7, 0.6)),
                children![(
                    Text::new(
                        "FPS CONTROLLER LAB (feasibility)\n\
                         WASD       Move (relative to facing)\n\
                         ← → ↑ ↓    Look (yaw / pitch)\n\
                         Shift      Sprint\n\
                         Space      Jump\n\
                         T          Replay the recorded path / back to live\n\
                         R reset · F1 debug\n\n\
                         A deterministic first-person controller on the shared\n\
                         PlayerIntent, stepped at a fixed timestep. Your live path is\n\
                         recorded (green); press T and the same intents are replayed on\n\
                         a fresh body (magenta) — the trails coincide exactly, which is\n\
                         what makes replay and lockstep networking possible.",
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

fn intent_from_keys(keys: &ButtonInput<KeyCode>, mouse: Vec2) -> PlayerIntent {
    let axis =
        |neg: KeyCode, pos: KeyCode| (keys.pressed(pos) as i32 - keys.pressed(neg) as i32) as f32;
    PlayerIntent {
        movement: Vec2::new(
            axis(KeyCode::KeyA, KeyCode::KeyD),
            axis(KeyCode::KeyS, KeyCode::KeyW),
        ),
        // Look from the mouse (primary) plus arrow keys (fallback). ArrowUp looks up
        // (negative look.y), matching screen-up being a negative mouse delta.
        look: Vec2::new(
            axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) + mouse.x * MOUSE_SENS,
            axis(KeyCode::ArrowUp, KeyCode::ArrowDown) + mouse.y * MOUSE_SENS,
        ),
        jump_pressed: keys.pressed(KeyCode::Space),
        sprint_held: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        ..Default::default()
    }
}

/// Fixed-timestep simulation: record live intents or advance the replay. The
/// mouse-derived look is recorded in the tape, so replay still reproduces it.
pub(crate) fn simulate(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut stage: ResMut<Stage>,
) {
    match stage.mode {
        Mode::Live => {
            let mouse = mouse_motion.map(|m| m.delta).unwrap_or(Vec2::ZERO);
            let intent = intent_from_keys(&keys, mouse);
            stage.step_live(intent);
        }
        Mode::Replay => stage.step_replay(),
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
    keys: Res<ButtonInput<KeyCode>>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
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

pub(crate) fn handle_toggles(keys: Res<ButtonInput<KeyCode>>, mut stage: ResMut<Stage>) {
    if keys.just_pressed(KeyCode::KeyT) {
        match stage.mode {
            Mode::Live => stage.begin_replay(),
            Mode::Replay => stage.return_to_live(),
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        stage.reset();
    }
    if keys.just_pressed(KeyCode::F1) {
        stage.debug_visible = !stage.debug_visible;
    }
}

pub(crate) fn present_camera(
    stage: Res<Stage>,
    mut camera: Single<&mut Transform, With<PlayerCam>>,
) {
    if let Some(pose) = stage.camera_override {
        **camera = pose;
        return;
    }
    let body = stage.active();
    let eye = body.eye(&stage.config);
    **camera = Transform::from_translation(eye).looking_to(body.look_dir(), Vec3::Y);
}

fn draw_box(gizmos: &mut Gizmos, b: &Aabb3, color: Color) {
    let c = [
        Vec3::new(b.min.x, b.min.y, b.min.z),
        Vec3::new(b.max.x, b.min.y, b.min.z),
        Vec3::new(b.max.x, b.min.y, b.max.z),
        Vec3::new(b.min.x, b.min.y, b.max.z),
        Vec3::new(b.min.x, b.max.y, b.min.z),
        Vec3::new(b.max.x, b.max.y, b.min.z),
        Vec3::new(b.max.x, b.max.y, b.max.z),
        Vec3::new(b.min.x, b.max.y, b.max.z),
    ];
    for (a, d) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
        gizmos.line(c[a], c[d], color);
        gizmos.line(c[a + 4], c[d + 4], color);
        gizmos.line(c[a], c[a + 4], color);
    }
}

fn draw_trail(gizmos: &mut Gizmos, path: &[Vec3], y: f32, color: Color) {
    if path.len() < 2 {
        return;
    }
    gizmos.linestrip(path.iter().map(|p| Vec3::new(p.x, y, p.z)), color);
    // Periodic upright ticks make the trail read as a ribbon in perspective.
    for p in path.iter().step_by(8) {
        gizmos.line(Vec3::new(p.x, y, p.z), Vec3::new(p.x, y + 0.6, p.z), color);
    }
}

pub(crate) fn draw_debug(stage: Res<Stage>, mut gizmos: Gizmos) {
    if !stage.debug_visible {
        return;
    }
    let h = stage.arena.floor_half;

    // Floor grid.
    let mut x = -h;
    while x <= h + 0.01 {
        gizmos.line(Vec3::new(x, 0.02, -h), Vec3::new(x, 0.02, h), GRID);
        gizmos.line(Vec3::new(-h, 0.02, x), Vec3::new(h, 0.02, x), GRID);
        x += 3.0;
    }

    // Solids (walls + pillars).
    for solid in &stage.arena.solids {
        draw_box(&mut gizmos, solid, SOLID);
    }

    // Trails: recorded (green) and, once replayed, the replay (magenta) on top.
    draw_trail(&mut gizmos, &stage.live_path, 0.25, LIVE);
    draw_trail(&mut gizmos, &stage.replay_path, 0.45, REPLAY);

    // The active body: a vertical marker.
    let p = stage.active().position;
    let color = if stage.mode == Mode::Replay {
        REPLAY
    } else {
        MARK
    };
    gizmos.line(
        Vec3::new(p.x, 0.05, p.z),
        Vec3::new(p.x, stage.config.half_height * 2.0, p.z),
        color,
    );
}

#[derive(SystemParam)]
pub(crate) struct DebugContext<'w, 's> {
    stage: Res<'w, Stage>,
    cams: Query<'w, 's, (), With<PlayerCam>>,
    ui_roots: Query<'w, 's, (), With<FpsUiRoot>>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    panel: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
}

pub(crate) fn update_debug_text(mut context: DebugContext) {
    let visibility = if context.stage.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.panel = visibility;
    **context.help = visibility;

    let stage = &*context.stage;
    let body = stage.active();
    let speed = Vec3::new(body.velocity.x, 0.0, body.velocity.z).length();

    let cams = context.cams.iter().count();
    let ui_roots = context.ui_roots.iter().count();
    let healthy = cams == 1 && ui_roots == 1;

    let replay = match stage.replay_match {
        Some(true) => "MATCH ✓",
        Some(false) => "MISMATCH ✗",
        None if stage.mode == crate::lab::Mode::Replay => "replaying…",
        None => "—",
    };

    let mut text = context.text.into_inner();
    **text = format!(
        "FPS CONTROLLER  {}\n\
         mode            {:?}\n\
         recorded ticks  {}\n\
         live path pts   {}\n\
         replay pts      {}\n\
         replay check    {}\n\
         position        ({:.1}, {:.1}, {:.1})\n\
         grounded        {}\n\
         facing          {:.0}°   speed {:.1} m/s\n\
         camera {cams}  UI {ui_roots}   resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        stage.mode,
        stage.tape.len(),
        stage.live_path.len(),
        stage.replay_path.len(),
        replay,
        body.position.x,
        body.position.y,
        body.position.z,
        if body.grounded { "yes" } else { "no" },
        body.yaw.to_degrees().rem_euclid(360.0),
        speed,
        stage.reset_count,
        stage.last_event,
    );
}

/// Build a scripted demonstration: walk, turn, walk, with a sprint-jump finish.
/// Used by the evidence capture (and handy for tests).
pub fn scripted_tape() -> Vec<PlayerIntent> {
    let mut tape = Vec::new();
    let walk = |fwd: f32, strafe: f32, look: f32, jump: bool, sprint: bool| PlayerIntent {
        movement: Vec2::new(strafe, fwd),
        look: Vec2::new(look, 0.0),
        jump_pressed: jump,
        sprint_held: sprint,
        ..Default::default()
    };
    for _ in 0..150 {
        tape.push(walk(1.0, 0.0, 0.0, false, false)); // forward (north)
    }
    for _ in 0..45 {
        tape.push(walk(0.3, 0.0, 1.0, false, false)); // turn right while easing forward
    }
    for _ in 0..150 {
        tape.push(walk(1.0, 0.0, 0.0, false, true)); // sprint along the new heading
    }
    for _ in 0..20 {
        tape.push(walk(1.0, 0.0, 0.0, true, true)); // a couple of jumps
    }
    tape
}

/// Compute both trails for a scripted tape without needing the fixed-update loop —
/// used by the evidence capture.
pub fn precompute(stage: &mut Stage, tape: Vec<PlayerIntent>) {
    stage.live_path = run_path(
        FpsBody::spawned(SPAWN, 0.0),
        &tape,
        &stage.arena,
        &stage.config,
        FIXED_DT,
    );
    stage.replay_path = run_path(
        FpsBody::spawned(SPAWN, 0.0),
        &tape,
        &stage.arena,
        &stage.config,
        FIXED_DT,
    );
    stage.replay_match = Some(stage.replay_path == stage.live_path);
    stage.replay_cursor = tape.len();
    stage.body = FpsBody::spawned(SPAWN, 0.0);
    if let Some(end) = stage.live_path.last() {
        stage.body.position = *end;
    }
    stage.tape = tape;
    stage.mode = Mode::Live;
}
