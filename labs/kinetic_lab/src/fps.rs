//! First-person view of the same board the schematic draws.
//!
//! This module renders and reads input. It decides nothing: [`crate::model`]
//! owns the rules and [`crate::embodied`] owns where the body is.
//!
//! ## Shape language
//!
//! Canon: original geometric constructs, with "Modron" a mood reference only —
//! never shipped terminology and never a copied design. What the mood actually
//! contributes is a *principle*, and the principle is legible on its own terms:
//! **rank reads as order of the solid**, and everything of the facility moves
//! in rigid quantised steps rather than gliding.
//!
//! - A **minor Guardian** is a cube. Lowest rank, simplest solid, six faces.
//! - The **major Guardian** is a tetrahedron — the pyramidal silhouette canon
//!   already fixed, and a rarer solid for a rarer thing.
//! - A **recharge station** is a disc, and the **generator** a standing prism.
//!
//! The clockwork read is not decoration bolted on afterwards. Guardians already
//! occupy whole lattice cells and move on a fixed tick, because that is what the
//! determinism contract required. Presentation leans into it: a Guardian crosses
//! between cells in a short, crisp snap and then *holds* for the rest of its
//! interval, and its yaw lands on one of the six lattice faces rather than
//! anywhere between them. The constraint became the aesthetic.

use bevy::{
    camera::Hdr,
    input::mouse::AccumulatedMouseMotion,
    pbr::{DistanceFog, FogFalloff},
    post_process::bloom::Bloom,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use observed_hex::coords::HexCoord;
use player_input::PlayerIntent;

use crate::embodied::{
    Embodiment, FLOOR_TOP, PLATE_HALF_X, PLATE_HALF_Z, PLATE_THICKNESS, ToolRequest, WALL_HEIGHT,
    plate_center,
};
use crate::model::{
    CellKind, KineticEvent, KineticWorld, MAX_CHARGE, MinorGuardianId, PUSH_IMPULSE, ShoveFate,
    ToolRefusal,
};

/// How long a Guardian's move between cells takes to play, in seconds. Well
/// under its step interval, so the hold between moves is what you mostly see.
const SNAP_SECONDS: f32 = 0.16;
/// Metres per second a body crosses the lattice at.
///
/// A fixed duration is right for a one-cell step and badly wrong for a shove: a
/// four-cell push covers 56 metres, and at a flat 0.16s that is 350 m/s, which
/// reads as the target vanishing rather than being thrown. Constant *speed*
/// keeps the one-cell snap crisp while giving a long shove a flight you can
/// actually watch.
const SNAP_SPEED: f32 = 55.0;
/// Longest a single crossing may take, so nothing ever floats.
const SNAP_SECONDS_MAX: f32 = 1.3;
/// Raw mouse pixels to look units, matching the other first-person labs in this
/// workspace (`fps_maze_lab`). The controller multiplies by `look_step`, and the
/// product must stay proportional to the actual mouse movement — the whole point
/// is that a flick turns further than a nudge.
const MOUSE_SENSITIVITY: f32 = 0.075;
/// Presentation-only gravity for a Guardian that has gone over the edge. The
/// simulation has already removed it; this is just how it leaves the screen.
const VOID_GRAVITY: f32 = 22.0;
/// How long a destroyed Guardian keeps falling before its mesh is retired.
const VOID_FALL_SECONDS: f32 = 2.4;

const COLOR_PLATE: Color = Color::srgb(0.20, 0.25, 0.32);
const COLOR_PLATE_LEDGE: Color = Color::srgb(0.46, 0.34, 0.10);
const COLOR_PLATE_RETRACTING: Color = Color::srgb(0.38, 0.11, 0.09);
const COLOR_PLATE_WALL: Color = Color::srgb(0.22, 0.25, 0.30);
const COLOR_MINOR: Color = Color::srgb(1.0, 0.40, 0.26);
const COLOR_MAJOR_AWAKE: Color = Color::srgb(1.0, 0.14, 0.40);
const COLOR_MAJOR_FROZEN: Color = Color::srgb(0.36, 0.54, 0.72);
const COLOR_STATION: Color = Color::srgb(0.30, 1.0, 0.68);
const COLOR_STATION_DEAD: Color = Color::srgb(0.20, 0.30, 0.26);
const COLOR_GENERATOR: Color = Color::srgb(1.0, 0.84, 0.30);
const CROSSHAIR_IDLE: Color = Color::srgba(0.7, 0.95, 1.0, 0.55);
const CROSSHAIR_TARGET: Color = Color::srgb(0.85, 0.92, 1.0);
const CROSSHAIR_LETHAL: Color = Color::srgb(0.30, 1.0, 0.55);

#[derive(Component)]
pub(crate) struct FpsOwned;

#[derive(Component)]
pub(crate) struct PlayerCam;

#[derive(Component)]
pub(crate) struct PlateShell(pub HexCoord);

/// Presentation-side animation state for one Guardian. The simulation knows
/// only which cell it is in; the snap between cells lives here.
#[derive(Component)]
pub(crate) struct Clockwork {
    pub from: Vec3,
    pub to: Vec3,
    pub elapsed: f32,
    /// How long this particular crossing takes, from its distance.
    pub duration: f32,
    pub yaw: f32,
    /// Seconds spent falling since the simulation declared this Guardian gone.
    /// The model kills it the instant it enters void; hiding the mesh on that
    /// same tick would make it blink out of existence instead of going over the
    /// edge, which is precisely the moment the lab exists to show.
    pub falling: f32,
}

impl Clockwork {
    fn at(position: Vec3) -> Self {
        Self {
            from: position,
            to: position,
            elapsed: SNAP_SECONDS,
            duration: SNAP_SECONDS,
            yaw: 0.0,
            falling: 0.0,
        }
    }

    /// Begin a new snap toward `target`, facing the way it travels.
    fn retarget(&mut self, target: Vec3) {
        if (target - self.to).length_squared() < 1e-4 {
            return;
        }
        self.from = self.current();
        self.to = target;
        self.elapsed = 0.0;
        let delta = self.to - self.from;
        self.duration = (delta.length() / SNAP_SPEED).clamp(SNAP_SECONDS, SNAP_SECONDS_MAX);
        if delta.length_squared() > 1e-4 {
            // Land on the lattice face travelled, not a free angle. A mesh's
            // forward is -Z, so facing `delta` needs the negated components;
            // using them raw points every Guardian backwards.
            self.yaw = (-delta.x).atan2(-delta.z);
        }
    }

    fn current(&self) -> Vec3 {
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        // Crisp in, crisp out: a machine starting and stopping, not drifting.
        let eased = t * t * (3.0 - 2.0 * t);
        let mut position = self.from.lerp(self.to, eased);
        // Once it is over the edge it stops being clockwork and starts being
        // an object: it crosses to the void plate, then simply falls.
        position.y -= 0.5 * VOID_GRAVITY * self.falling * self.falling;
        position
    }
}

#[derive(Component)]
pub(crate) struct MinorShell(pub MinorGuardianId);

#[derive(Component)]
pub(crate) struct MajorShell;

#[derive(Component)]
pub(crate) struct StationShell;

#[derive(Component)]
pub(crate) struct PreviewShell;

#[derive(Component)]
pub(crate) struct Crosshair;

#[derive(Component)]
pub(crate) struct JailOverlay;

#[derive(Component)]
pub(crate) struct FpsUiRoot;

#[derive(Component)]
pub(crate) struct FpsDebugText;

/// Lab-local state. Not authoritative.
#[derive(Resource, Clone, Debug)]
pub struct FpsRuntime {
    pub request: ToolRequest,
    /// Input latched in `Update` and drained by the fixed tick.
    ///
    /// `AccumulatedMouseMotion` and `just_pressed` are *per-frame* signals, and
    /// `FixedUpdate` runs zero, one, or several times per frame. Sampling them
    /// from the fixed tick threw away most of the mouse on a 144 Hz display and
    /// silently ate jumps. Everything edge-triggered is accumulated here first
    /// so no press and no pixel of motion can fall between the schedules.
    pub pending_look: Vec2,
    pub pending_jump: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    /// Freeze the board. The camera keeps moving, so a stance can be studied
    /// (and photographed) without a Guardian walking onto you mid-look.
    pub paused: bool,
    /// When set, this replaces live input for one tick. The demo tape uses it to
    /// drive the lab down the *same* path a player's hands take — intents in,
    /// rules out — so a recording is a real run rather than a puppet show.
    pub scripted: Option<(PlayerIntent, ToolRequest)>,
    pub last_note: String,
}

impl Default for FpsRuntime {
    fn default() -> Self {
        Self {
            request: ToolRequest::None,
            pending_look: Vec2::ZERO,
            pending_jump: false,
            reset_requested: false,
            reset_count: 0,
            paused: false,
            scripted: None,
            last_note: "Look down a lane and push something off the edge.".to_string(),
        }
    }
}

/// Materials reused across the scene, so presentation never rebuilds an asset
/// per frame.
#[derive(Resource)]
pub(crate) struct FpsPalette {
    plate: Handle<StandardMaterial>,
    ledge: Handle<StandardMaterial>,
    retracting: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
    minor: Handle<StandardMaterial>,
    major_awake: Handle<StandardMaterial>,
    major_frozen: Handle<StandardMaterial>,
    station: Handle<StandardMaterial>,
    station_dead: Handle<StandardMaterial>,
    generator: Handle<StandardMaterial>,
    preview_lethal: Handle<StandardMaterial>,
    preview_safe: Handle<StandardMaterial>,
}

fn emissive_material(
    materials: &mut Assets<StandardMaterial>,
    color: Color,
    glow: f32,
) -> Handle<StandardMaterial> {
    let linear = color.to_linear();
    materials.add(StandardMaterial {
        base_color: color,
        emissive: LinearRgba::new(
            linear.red * glow,
            linear.green * glow,
            linear.blue * glow,
            1.0,
        ),
        perceptual_roughness: 0.75,
        ..default()
    })
}

pub(crate) fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    world: Res<KineticWorld>,
    embodiment: Res<Embodiment>,
) {
    let palette = FpsPalette {
        plate: emissive_material(&mut materials, COLOR_PLATE, 0.30),
        ledge: emissive_material(&mut materials, COLOR_PLATE_LEDGE, 1.3),
        retracting: emissive_material(&mut materials, COLOR_PLATE_RETRACTING, 1.8),
        wall: emissive_material(&mut materials, COLOR_PLATE_WALL, 0.35),
        minor: emissive_material(&mut materials, COLOR_MINOR, 2.6),
        major_awake: emissive_material(&mut materials, COLOR_MAJOR_AWAKE, 3.2),
        major_frozen: emissive_material(&mut materials, COLOR_MAJOR_FROZEN, 1.1),
        station: emissive_material(&mut materials, COLOR_STATION, 3.0),
        station_dead: emissive_material(&mut materials, COLOR_STATION_DEAD, 0.15),
        generator: emissive_material(&mut materials, COLOR_GENERATOR, 3.4),
        preview_lethal: emissive_material(&mut materials, Color::srgb(0.25, 1.0, 0.5), 4.0),
        preview_safe: emissive_material(&mut materials, Color::srgb(0.6, 0.66, 0.74), 1.6),
    };

    // Exactly the collider's footprint. These used to be 25 cm narrower "for a
    // seam", which meant you stood a hand's width past every visible edge on
    // invisible floor — while the docs claimed rendering matched collision
    // exactly. The seam now comes from the plate outlines, which cost nothing
    // and cannot lie about where the floor ends.
    let plate_mesh = meshes.add(Cuboid::new(
        PLATE_HALF_X * 2.0,
        PLATE_THICKNESS,
        PLATE_HALF_Z * 2.0,
    ));
    let wall_mesh = meshes.add(Cuboid::new(
        PLATE_HALF_X * 2.0,
        WALL_HEIGHT,
        PLATE_HALF_Z * 2.0,
    ));
    // Rank reads as order of the solid: a cube for the many, a tetrahedron for
    // the rare one.
    let minor_mesh = meshes.add(Cuboid::new(1.6, 1.6, 1.6));
    let major_mesh = meshes.add(Tetrahedron::default());
    let station_mesh = meshes.add(Cylinder::new(2.2, 0.3));
    let generator_mesh = meshes.add(Cuboid::new(2.0, 5.0, 2.0));
    // A beam rather than a floor decal: the lethal destination is usually void
    // seventy metres away, where a flat marker is invisible and — worse — hidden
    // behind the very Guardian being aimed at.
    let preview_mesh = meshes.add(Cuboid::new(1.1, 9.0, 1.1));

    commands.spawn((
        FpsOwned,
        PlayerCam,
        Camera3d::default(),
        Hdr,
        Bloom {
            intensity: 0.20,
            ..Bloom::NATURAL
        },
        // Fog is what makes an unlit plate fall away into the dark, so the void
        // rim reads as an edge rather than as a wall. The Legibility Contract
        // still binds: emissive actors punch through it at every distance.
        DistanceFog {
            color: Color::srgb(0.006, 0.010, 0.018),
            falloff: FogFalloff::Linear {
                start: 34.0,
                end: 210.0,
            },
            ..default()
        },
        Transform::from_translation(embodiment.body.eye(&embodiment.config)),
        Name::new("Kinetic FPS Camera"),
    ));

    // Neon-noir: almost no fill, so the emissive solids carry the room.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.32, 0.44, 0.62),
        brightness: 260.0,
        ..default()
    });
    commands.spawn((
        FpsOwned,
        DirectionalLight {
            illuminance: 1_400.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.85, -0.35, 0.0)),
        Name::new("Dim key light"),
    ));

    for index in 0..world.grid.cell_count() {
        let coord = world.grid.coord(index);
        let center = plate_center(coord);
        commands.spawn((
            FpsOwned,
            PlateShell(coord),
            Mesh3d(plate_mesh.clone()),
            MeshMaterial3d(palette.plate.clone()),
            Transform::from_xyz(center.x, FLOOR_TOP - PLATE_THICKNESS / 2.0, center.z),
            Name::new(format!("Plate {},{}", coord.q, coord.r)),
        ));
    }

    // Walls are separate standing blocks rather than a plate variant, so a wall
    // never silently becomes a floor.
    for index in 0..world.grid.cell_count() {
        let coord = world.grid.coord(index);
        if world.cell(coord) != CellKind::Wall {
            continue;
        }
        let center = plate_center(coord);
        commands.spawn((
            FpsOwned,
            Mesh3d(wall_mesh.clone()),
            MeshMaterial3d(palette.wall.clone()),
            Transform::from_xyz(center.x, FLOOR_TOP + WALL_HEIGHT / 2.0, center.z),
            Name::new("Wall"),
        ));
    }

    for station in &world.stations {
        let center = plate_center(station.cell);
        commands.spawn((
            FpsOwned,
            StationShell,
            Mesh3d(station_mesh.clone()),
            MeshMaterial3d(palette.station.clone()),
            Transform::from_xyz(center.x, FLOOR_TOP + 0.2, center.z),
            Name::new("Recharge Station"),
        ));
    }

    let generator = plate_center(world.generator);
    commands.spawn((
        FpsOwned,
        Mesh3d(generator_mesh.clone()),
        MeshMaterial3d(palette.generator.clone()),
        Transform::from_xyz(generator.x, FLOOR_TOP + 2.5, generator.z),
        Name::new("Generator"),
    ));

    for minor in &world.minors {
        let center = plate_center(minor.cell);
        let position = Vec3::new(center.x, FLOOR_TOP + 1.1, center.z);
        commands.spawn((
            FpsOwned,
            MinorShell(minor.id),
            Clockwork::at(position),
            Mesh3d(minor_mesh.clone()),
            MeshMaterial3d(palette.minor.clone()),
            Transform::from_translation(position),
            Name::new(format!("Minor Guardian {}", minor.id.0)),
        ));
    }

    let major = plate_center(world.major.cell);
    let major_position = Vec3::new(major.x, FLOOR_TOP + 1.6, major.z);
    commands.spawn((
        FpsOwned,
        MajorShell,
        Clockwork::at(major_position),
        Mesh3d(major_mesh.clone()),
        MeshMaterial3d(palette.major_awake.clone()),
        Transform::from_translation(major_position).with_scale(Vec3::splat(2.6)),
        Name::new("Major Guardian"),
    ));

    commands.spawn((
        FpsOwned,
        PreviewShell,
        Mesh3d(preview_mesh.clone()),
        MeshMaterial3d(palette.preview_safe.clone()),
        Transform::from_xyz(0.0, FLOOR_TOP + 4.5, 0.0),
        Visibility::Hidden,
        Name::new("Shove Preview"),
    ));

    commands.insert_resource(palette);
    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            FpsOwned,
            FpsUiRoot,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
            Name::new("Kinetic FPS UI"),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(470),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.012, 0.022, 0.032, 0.94)),
                BorderColor::all(Color::srgba(0.45, 0.85, 1.0, 0.55)),
                children![(
                    FpsDebugText,
                    Text::new("Kinetic diagnostics starting..."),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.95, 1.0)),
                )],
            ));
            root.spawn((
                Node {
                    width: px(330),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.012, 0.022, 0.032, 0.94)),
                BorderColor::all(Color::srgba(1.0, 0.55, 0.3, 0.55)),
                children![(
                    Text::new(
                        "WASD move   SHIFT run   SPACE jump\n\
                         LMB push   RMB pull   E generator\n\
                         P pause    R reset    ESC free the cursor\n\
                         crosshair: dim = no target, white = target,\n\
                         green = this push kills",
                    ),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.92, 0.88)),
                )],
            ));
        });

    // Jail is a terminal state: the model stops accepting tool intents from a
    // jailed Observer and every Guardian stops moving, so the run is over. It
    // used to announce itself by changing one word inside a debug blob, which
    // is indistinguishable from the controls having broken. Say it plainly.
    commands.spawn((
        FpsOwned,
        JailOverlay,
        Node {
            position_type: PositionType::Absolute,
            left: percent(0),
            top: percent(0),
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.35, 0.02, 0.10, 0.45)),
        GlobalZIndex(30),
        Visibility::Hidden,
        Name::new("Jail Overlay"),
        children![(
            Text::new("JAILED\n\nA Guardian reached you.\nPress R to reset."),
            TextFont {
                font_size: FontSize::Px(34.0),
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.86, 0.88)),
        )],
    ));

    // Crosshair. It has to report whether the tool actually has a target: a
    // reticle that looks identical whether a push will fire or be refused is
    // what makes a working trigger feel like a dead one.
    commands.spawn((
        FpsOwned,
        Crosshair,
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            width: px(6),
            height: px(6),
            ..default()
        },
        BackgroundColor(CROSSHAIR_IDLE),
        GlobalZIndex(21),
        Name::new("Crosshair"),
    ));
}

pub(crate) fn present_jail_overlay(
    world: Res<KineticWorld>,
    mut overlay: Query<&mut Visibility, With<JailOverlay>>,
) {
    let jailed = world
        .observers
        .first()
        .is_some_and(|observer| observer.jailed);
    if let Ok(mut visibility) = overlay.single_mut() {
        *visibility = if jailed {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Colour the crosshair by what the trigger would actually do right now.
pub(crate) fn present_crosshair(
    world: Res<KineticWorld>,
    mut crosshair: Query<(&mut BackgroundColor, &mut Node), With<Crosshair>>,
) {
    let Ok((mut color, mut node)) = crosshair.single_mut() else {
        return;
    };
    let resolved = world.observers.first().and_then(|observer| {
        world
            .target_in_lane(observer)
            .and_then(|id| world.resolve_shove(id, observer.facing, PUSH_IMPULSE))
    });
    let (wanted, size) = match resolved {
        Some(resolution) if matches!(resolution.fate, ShoveFate::Void | ShoveFate::Doomed) => {
            (CROSSHAIR_LETHAL, 12.0)
        }
        Some(_) => (CROSSHAIR_TARGET, 10.0),
        None => (CROSSHAIR_IDLE, 6.0),
    };
    color.0 = wanted;
    node.width = px(size);
    node.height = px(size);
}

pub(crate) fn grab_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

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

/// Latch every edge-triggered input in `Update`, where it is sampled once per
/// frame, so nothing can be lost or doubled by the fixed tick's cadence.
pub(crate) fn gather_requests(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut runtime: ResMut<FpsRuntime>,
) {
    // Accumulate rather than overwrite: several frames may pass between ticks,
    // and every pixel of them is part of the same turn.
    if let Some(motion) = mouse_motion {
        runtime.pending_look += motion.delta * MOUSE_SENSITIVITY;
    }
    if keys.just_pressed(KeyCode::Space) {
        runtime.pending_jump = true;
    }

    if buttons.just_pressed(MouseButton::Left) {
        runtime.request = ToolRequest::Push;
    } else if buttons.just_pressed(MouseButton::Right) {
        runtime.request = ToolRequest::Pull;
    } else if keys.just_pressed(KeyCode::KeyE) {
        runtime.request = ToolRequest::ToggleGenerator;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        runtime.reset_requested = true;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        runtime.paused = !runtime.paused;
    }
}

pub(crate) fn perform_reset(
    mut runtime: ResMut<FpsRuntime>,
    mut world: ResMut<KineticWorld>,
    mut embodiment: ResMut<Embodiment>,
) {
    if !runtime.reset_requested {
        return;
    }
    let reset_count = runtime.reset_count + 1;
    *runtime = FpsRuntime {
        reset_count,
        ..default()
    };
    *world = KineticWorld::authored();
    *embodiment = Embodiment::new(&world);
}

/// One fixed tick: body, then board.
pub(crate) fn simulate(
    keys: Res<ButtonInput<KeyCode>>,
    mut runtime: ResMut<FpsRuntime>,
    mut world: ResMut<KineticWorld>,
    mut embodiment: ResMut<Embodiment>,
) {
    // A scripted tick takes the identical path as a played one: same intent
    // type, same tool request, same `Embodiment::step`.
    if let Some((intent, request)) = runtime.scripted.take() {
        embodiment.step(&mut world, intent, request);
        after_step(&mut runtime, &world, &embodiment);
        return;
    }

    // Drain the latched look and jump whether or not the board is paused, so
    // pausing stops the *world* and not the camera. Leaving them latched would
    // also mean a paused second of mouse movement snapped the view on resume.
    let mouse = std::mem::take(&mut runtime.pending_look);
    let jumped = std::mem::take(&mut runtime.pending_jump);

    let mut movement = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        movement.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        movement.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        movement.x += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        movement.x -= 1.0;
    }
    let intent = PlayerIntent {
        movement,
        look: mouse,
        jump_pressed: jumped,
        sprint_held: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        ..default()
    };

    if runtime.paused {
        // Move the body so look and walk still work, but hold the board: no
        // Guardian steps, no retraction, no capture.
        embodiment.step_body_only(intent);
        return;
    }

    let request = std::mem::take(&mut runtime.request);
    embodiment.step(&mut world, intent, request);
    after_step(&mut runtime, &world, &embodiment);
}

/// Bookkeeping every tick needs, whether it came from hands or from a tape.
fn after_step(runtime: &mut FpsRuntime, world: &KineticWorld, embodiment: &Embodiment) {
    if embodiment.fell_into_void {
        runtime.last_note = "You fell into true void. Recovered to spawn.".to_string();
    } else if let Some(note) = describe(world) {
        runtime.last_note = note;
    }
}

/// A committed retraction removes a plate, so the arena has to follow or the
/// player keeps standing on a floor the simulation says is gone.
pub(crate) fn refresh_arena_after_retraction(
    world: Res<KineticWorld>,
    mut embodiment: ResMut<Embodiment>,
) {
    if world
        .events
        .iter()
        .any(|event| matches!(event, KineticEvent::TileRetracted { .. }))
    {
        embodiment.refresh_arena(&world);
    }
}

/// One readable line for this tick.
///
/// Every event kind has something worth saying, so this reports the first event
/// of the tick rather than searching for one that happens to be interesting.
fn describe(world: &KineticWorld) -> Option<String> {
    world.events.first().map(|event| match event {
        KineticEvent::Shoved(resolution) => match resolution.fate {
            ShoveFate::Void => format!(
                "Shoved {} cells off the edge - the void killed it, not the tool.",
                resolution.cells_travelled
            ),
            ShoveFate::Doomed => {
                "Shoved onto a retracting plate - it dies when the plate commits.".to_string()
            }
            ShoveFate::Rest => format!(
                "Shoved {} cells onto solid floor. Alive, staggered.",
                resolution.cells_travelled
            ),
            ShoveFate::Blocked => "Blocked by structure - nothing moved.".to_string(),
        },
        KineticEvent::GuardianDestroyed { by_retraction, .. } => {
            if *by_retraction {
                "A plate committed and took its passenger.".to_string()
            } else {
                "Minor Guardian committed to void.".to_string()
            }
        }
        KineticEvent::ToolRefused { refusal, .. } => match refusal {
            ToolRefusal::NoTargetInLane => "Nothing in the lane.".to_string(),
            ToolRefusal::NotEnoughCharge => "Not enough charge.".to_string(),
            ToolRefusal::NotOnGenerator => "Stand on the generator to operate it.".to_string(),
        },
        KineticEvent::ChargeRestored { charge, .. } => {
            format!("Station restored a charge ({charge}/{MAX_CHARGE}).")
        }
        KineticEvent::GeneratorToggled { powered, .. } => {
            if *powered {
                "Power restored. Stations live, sight returns.".to_string()
            } else {
                "Power cut. Stations dead, sight is your own plate only.".to_string()
            }
        }
        KineticEvent::ObserverCaptured { by_major, .. } => {
            if *by_major {
                "Captured by the major Guardian.".to_string()
            } else {
                "Captured by a minor Guardian.".to_string()
            }
        }
        KineticEvent::TileRetracted { .. } => "A plate finished retracting.".to_string(),
    })
}

pub(crate) fn sync_camera(
    embodiment: Res<Embodiment>,
    mut camera: Query<&mut Transform, With<PlayerCam>>,
) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    // Aim the camera with the body's own look direction rather than rebuilding
    // it from Euler angles: `FpsBody::forward` uses (sin yaw, -cos yaw), which
    // is the opposite handedness to a Y-rotation of Bevy's -Z default, so an
    // Euler reconstruction silently looks the wrong way down the lane.
    *transform = Transform::from_translation(embodiment.body.eye(&embodiment.config))
        .looking_to(embodiment.body.look_dir(), Vec3::Y);
}

pub(crate) fn present_plates(
    world: Res<KineticWorld>,
    palette: Res<FpsPalette>,
    mut plates: Query<(
        &PlateShell,
        &mut MeshMaterial3d<StandardMaterial>,
        &mut Visibility,
    )>,
) {
    for (plate, mut material, mut visibility) in &mut plates {
        let kind = world.cell(plate.0);
        // A retracted plate is *gone*, not recoloured: the hole you can fall
        // through has to be the hole you can see.
        *visibility = if kind == CellKind::Void {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        let wanted = match kind {
            CellKind::Ledge => &palette.ledge,
            CellKind::Retracting { .. } => &palette.retracting,
            CellKind::Wall => &palette.wall,
            _ => &palette.plate,
        };
        if material.0.id() != wanted.id() {
            material.0 = wanted.clone();
        }
    }
}

type MinorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static MinorShell,
        &'static mut Clockwork,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    Without<MajorShell>,
>;
type MajorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Clockwork,
        &'static mut Transform,
        &'static mut MeshMaterial3d<StandardMaterial>,
    ),
    With<MajorShell>,
>;

pub(crate) fn present_guardians(
    time: Res<Time>,
    world: Res<KineticWorld>,
    palette: Res<FpsPalette>,
    mut minors: MinorQuery,
    mut major: MajorQuery,
) {
    let delta = time.delta_secs();

    for (shell, mut clockwork, mut transform, mut visibility) in &mut minors {
        let Some(minor) = world.minor(shell.0) else {
            continue;
        };
        if minor.alive {
            clockwork.falling = 0.0;
        } else if clockwork.elapsed >= clockwork.duration {
            // Fall only after the throw lands. Dropping during the flight would
            // sink it through the very ledge plates it is being thrown across.
            clockwork.falling += delta;
        }
        // A destroyed Guardian keeps its mesh long enough to be seen leaving:
        // it crosses to the void plate, then drops out of the world.
        *visibility = if minor.alive || clockwork.falling < VOID_FALL_SECONDS {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };

        let center = plate_center(minor.cell);
        clockwork.retarget(Vec3::new(center.x, FLOOR_TOP + 1.1, center.z));
        clockwork.elapsed += delta;
        transform.translation = clockwork.current();

        // Yaw snaps to the face travelled; a staggered Guardian tips, so being
        // shoved reads on the body and not only in the log. One going over the
        // edge tumbles: order breaks down exactly when the facility loses it.
        let tip = if minor.alive {
            if minor.stagger > 0 { 0.5 } else { 0.0 }
        } else {
            clockwork.falling * 3.2
        };
        let roll = if minor.alive {
            0.0
        } else {
            clockwork.falling * 2.1
        };
        transform.rotation = Quat::from_euler(EulerRot::YXZ, clockwork.yaw, tip, roll);
    }

    if let Ok((mut clockwork, mut transform, mut material)) = major.single_mut() {
        let center = plate_center(world.major.cell);
        clockwork.retarget(Vec3::new(center.x, FLOOR_TOP + 1.6, center.z));
        clockwork.elapsed += delta;
        transform.translation = clockwork.current();
        transform.rotation = Quat::from_euler(EulerRot::YXZ, clockwork.yaw, 0.0, 0.0);
        let wanted = if world.major.frozen {
            &palette.major_frozen
        } else {
            &palette.major_awake
        };
        if material.0.id() != wanted.id() {
            material.0 = wanted.clone();
        }
    }
}

pub(crate) fn present_stations(
    world: Res<KineticWorld>,
    palette: Res<FpsPalette>,
    mut stations: Query<&mut MeshMaterial3d<StandardMaterial>, With<StationShell>>,
) {
    let wanted = if world.powered {
        &palette.station
    } else {
        &palette.station_dead
    };
    for mut material in &mut stations {
        if material.0.id() != wanted.id() {
            material.0 = wanted.clone();
        }
    }
}

/// Mark the plate a push would send its target to.
///
/// Same pure `resolve_shove` the tick runs, so the marker cannot disagree with
/// the outcome.
pub(crate) fn present_preview(
    world: Res<KineticWorld>,
    palette: Res<FpsPalette>,
    mut preview: Query<
        (
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<PreviewShell>,
    >,
) {
    let Ok((mut transform, mut visibility, mut material)) = preview.single_mut() else {
        return;
    };
    let resolved = world.observers.first().and_then(|observer| {
        world
            .target_in_lane(observer)
            .and_then(|id| world.resolve_shove(id, observer.facing, PUSH_IMPULSE))
    });
    let Some(resolution) = resolved else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Inherited;
    let center = plate_center(resolution.to);
    // A lethal destination is usually void, which has no plate to stand the
    // marker on — the beam hangs over the hole instead.
    transform.translation = Vec3::new(center.x, FLOOR_TOP + 4.5, center.z);
    let wanted = match resolution.fate {
        ShoveFate::Void | ShoveFate::Doomed => &palette.preview_lethal,
        _ => &palette.preview_safe,
    };
    if material.0.id() != wanted.id() {
        material.0 = wanted.clone();
    }
}

/// Draw the path the target would travel, plate by plate.
///
/// The beam says *where it ends*; this says *how it gets there*, which is what
/// makes the ledge rule legible — the line visibly runs past the three cells a
/// push pays for and keeps going.
pub(crate) fn draw_lane(world: Res<KineticWorld>, mut gizmos: Gizmos) {
    let Some(observer) = world.observers.first() else {
        return;
    };

    // Always draw the reachable lane, target or not. The schematic view already
    // did this and said why; the first-person view — the one actually played —
    // drew nothing when the lane was empty, so the player stood inside an
    // invisible 60-degree wedge with no way to tell where it pointed. An empty
    // lane has to look empty, not look like nothing.
    let mut cursor = observer.cell;
    let height = FLOOR_TOP + 1.1;
    let lane_color = Color::srgba(0.45, 0.92, 1.0, 0.30);
    for _ in 0..crate::model::TOOL_RANGE {
        let Some(next) = world.grid.neighbor(cursor, observer.facing) else {
            break;
        };
        let a = plate_center(cursor);
        let b = plate_center(next);
        gizmos.line(
            Vec3::new(a.x, height, a.z),
            Vec3::new(b.x, height, b.z),
            lane_color,
        );
        cursor = next;
    }

    let Some(resolution) = world
        .target_in_lane(observer)
        .and_then(|id| world.resolve_shove(id, observer.facing, PUSH_IMPULSE))
    else {
        return;
    };

    let color = match resolution.fate {
        ShoveFate::Void => Color::srgb(0.25, 1.0, 0.5),
        ShoveFate::Doomed => Color::srgb(1.0, 0.72, 0.22),
        ShoveFate::Rest => Color::srgb(0.6, 0.66, 0.74),
        ShoveFate::Blocked => Color::srgb(1.0, 0.28, 0.24),
    };

    // Walk the same faces the resolution walked, so the drawn path is the
    // resolved path rather than a straight line that might cut a corner.
    let mut cursor = resolution.from;
    let height = FLOOR_TOP + 1.1;
    for _ in 0..resolution.cells_travelled {
        let Some(next) = world.grid.neighbor(cursor, resolution.face) else {
            break;
        };
        let a = plate_center(cursor);
        let b = plate_center(next);
        gizmos.line(
            Vec3::new(a.x, height, a.z),
            Vec3::new(b.x, height, b.z),
            color,
        );
        cursor = next;
    }
}

pub(crate) fn update_hud(
    world: Res<KineticWorld>,
    runtime: Res<FpsRuntime>,
    embodiment: Res<Embodiment>,
    mut text: Query<&mut Text, With<FpsDebugText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let Some(observer) = world.observers.first() else {
        return;
    };
    let lane = world
        .target_in_lane(observer)
        .and_then(|id| world.resolve_shove(id, observer.facing, PUSH_IMPULSE))
        .map_or_else(
            || "no target in lane".to_string(),
            |resolution| {
                format!(
                    "{:?} after {} cells",
                    resolution.fate, resolution.cells_travelled
                )
            },
        );

    **text = format!(
        "tick {}   |   cell {},{}   |   facing {:?}\n\
         charge {}/{}   |   power {}\n\
         minors alive {}   |   major {}\n\
         lane: {}\n\
         resets {}   |   {}\n\
         {}",
        world.tick,
        observer.cell.q,
        observer.cell.r,
        embodiment.facing,
        observer.charge,
        MAX_CHARGE,
        if world.powered { "ON" } else { "OUT" },
        world.living_minors(),
        if world.major.frozen {
            "FROZEN (observed)"
        } else {
            "awake"
        },
        lane,
        runtime.reset_count,
        if observer.jailed { "JAILED" } else { "free" },
        runtime.last_note,
    );
}

/// A Guardian's snap must finish well inside its step interval, or the clockwork
/// read turns into a glide. Enforced at compile time so tuning one without the
/// other cannot quietly soften the whole shape language.
const _: () = assert!(SNAP_SECONDS * 60.0 < crate::model::MINOR_STEP_TICKS as f32);
