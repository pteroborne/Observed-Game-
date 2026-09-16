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
use observed_assets::AssetSlot;
use observed_hex::{coords::HexCoord, coords::lateral_distance};
use player_input::PlayerIntent;

use crate::embodied::{
    Embodiment, FLOOR_TOP, PLATE_HALF_X, PLATE_HALF_Z, PLATE_THICKNESS, ToolRequest, WALL_HEIGHT,
    plate_center,
};
use crate::model::{
    AimState, CellKind, KineticEvent, KineticWorld, MAX_CHARGE, MatchOutcome, MinorGuardianId,
    ShoveFate, TOOL_RANGE, ToolRefusal,
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
/// Rest pose of the tool in camera space: low right, angled inward.
// Far enough out that the tool reads as held rather than pressed against the
// lens. At 0.62 metres it covered most of the lower-right quadrant and hid the
// plate the player was about to be pushed off; worse, a body that long at a
// fixed height splays *downward* as it approaches the camera, because the near
// end is magnified, so it ran off the bottom of the frame. The fix is distance
// plus a shorter body, not a smaller one.
const VIEWMODEL_HOME: Vec3 = Vec3::new(0.32, -0.24, -1.00);
/// Half-extents of the tool body, in metres. Kept short in Z for the reason
/// above.
const VIEWMODEL_SIZE: Vec3 = Vec3::new(0.16, 0.12, 0.42);
/// How far the tool kicks back when fired, and how long the whole flash lasts.
const RECOIL_DISTANCE: f32 = 0.16;
const FLASH_SECONDS: f32 = 0.28;

const COLOR_PLATE: Color = Color::srgb(0.20, 0.25, 0.32);
const COLOR_PLATE_LEDGE: Color = Color::srgb(0.46, 0.34, 0.10);
const COLOR_PLATE_RETRACTING: Color = Color::srgb(0.38, 0.11, 0.09);
const COLOR_PLATE_WALL: Color = Color::srgb(0.30, 0.35, 0.44);
const COLOR_MINOR: Color = Color::srgb(1.0, 0.40, 0.26);
const COLOR_MAJOR_AWAKE: Color = Color::srgb(1.0, 0.14, 0.40);
const COLOR_MAJOR_FROZEN: Color = Color::srgb(0.36, 0.54, 0.72);
const COLOR_STATION: Color = Color::srgb(0.30, 1.0, 0.68);
const COLOR_STATION_DEAD: Color = Color::srgb(0.20, 0.30, 0.26);
const COLOR_GENERATOR: Color = Color::srgb(1.0, 0.84, 0.30);
const CROSSHAIR_IDLE: Color = Color::srgba(0.7, 0.95, 1.0, 0.55);
const CROSSHAIR_TARGET: Color = Color::srgb(0.85, 0.92, 1.0);
const CROSSHAIR_LETHAL: Color = Color::srgb(0.30, 1.0, 0.55);
/// Seen, but too far to grab. Distinct from idle, because "walk closer" and
/// "there is nothing there" call for opposite reactions.
const CROSSHAIR_FAR: Color = Color::srgb(1.0, 0.72, 0.25);
/// In range, in the cone, and behind a wall. Moving closer will not help.
const CROSSHAIR_BLOCKED: Color = Color::srgb(0.96, 0.36, 0.34);
/// Half-length of one reticle arm, in pixels, before a kill count lengthens it.
const CROSSHAIR_ARM: f32 = 11.0;
/// Arm thickness, in pixels.
const CROSSHAIR_THICKNESS: f32 = 3.0;
/// Gap between the centre and the inner end of each arm once the tool can
/// actually grab what it is looking at. The reticle *shuts* on a target.
const CROSSHAIR_GAP_SHUT: f32 = 6.0;
/// Widest the reticle ever opens, so a Guardian across the facility does not
/// push the arms off screen.
const CROSSHAIR_GAP_MAX: f32 = 20.0;

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

/// The ring that sits on whatever the tool has actually selected.
///
/// The cone is wide enough now that "what am I about to grab" stops being
/// obvious from the crosshair alone. This answers it directly, on the Guardian,
/// where the player is already looking.
#[derive(Component)]
pub(crate) struct TargetRing;

/// The tool itself, held in view. Recoils when fired.
#[derive(Component)]
pub(crate) struct ViewModel {
    /// Rest pose in camera space.
    pub home: Vec3,
}

/// The light that flashes at the muzzle when the tool goes off.
#[derive(Component)]
pub(crate) struct MuzzleFlash;

/// The light that flashes where a shot landed.
#[derive(Component)]
pub(crate) struct ImpactFlash;

/// The reticle's centre dot. Always visible, so the screen centre is never
/// ambiguous even when the arms are wide open.
#[derive(Component)]
pub(crate) struct Crosshair;

/// One of the reticle's four arms.
///
/// The arms carry two independent readings at once: their *gap* from centre is
/// range — it closes as the target comes within [`TOOL_RANGE`] and snaps shut
/// when the tool can grab it — and their *colour and length* are capability,
/// what the trigger would actually do.
#[derive(Component, Clone, Copy)]
pub(crate) struct CrosshairArm {
    /// Unit direction from the centre: one of the four axis-aligned offsets.
    dir: (f32, f32),
}

impl CrosshairArm {
    /// Where this arm sits for a given gap and length, in pixels relative to
    /// the screen centre.
    fn layout(self, gap: f32, length: f32) -> (f32, f32, f32, f32) {
        let half = CROSSHAIR_THICKNESS / 2.0;
        let (width, height) = if self.dir.0 == 0.0 {
            (CROSSHAIR_THICKNESS, length)
        } else {
            (length, CROSSHAIR_THICKNESS)
        };
        let axis = |d: f32, span: f32| {
            if d > 0.0 {
                gap
            } else if d < 0.0 {
                -(gap + span)
            } else {
                -half
            }
        };
        (
            axis(self.dir.0, width),
            axis(self.dir.1, height),
            width,
            height,
        )
    }
}

/// The reticle's look, derived from [`AimState`] alone.
///
/// Split out from the system so the mapping can be tested without a window:
/// a reticle that lies about range is exactly the bug this whole pass exists
/// to fix, and it is not something a screenshot proves.
#[must_use]
pub(crate) fn crosshair_look(state: AimState) -> (Color, f32, f32) {
    match state {
        AimState::Empty => (CROSSHAIR_IDLE, CROSSHAIR_GAP_MAX, CROSSHAIR_ARM),
        // The gap closes as you walk in, so the reticle is a distance meter you
        // read without taking your eyes off the Guardian.
        AimState::OutOfReach { cells } => {
            let over = cells.saturating_sub(TOOL_RANGE) as f32;
            let gap = (CROSSHAIR_GAP_SHUT + 4.0 * over).min(CROSSHAIR_GAP_MAX);
            (CROSSHAIR_FAR, gap, CROSSHAIR_ARM)
        }
        // Wide open: closing the distance is not the answer here.
        AimState::Occluded => (CROSSHAIR_BLOCKED, CROSSHAIR_GAP_MAX, CROSSHAIR_ARM),
        AimState::Reach { kills: 0 } => (CROSSHAIR_TARGET, CROSSHAIR_GAP_SHUT, CROSSHAIR_ARM),
        AimState::Reach { kills } => (
            CROSSHAIR_LETHAL,
            CROSSHAIR_GAP_SHUT,
            // One extra pixel of reach per Guardian the chain takes.
            CROSSHAIR_ARM + 4.0 * (kills.min(4) - 1) as f32,
        ),
    }
}

#[derive(Component)]
pub(crate) struct JailOverlay;

#[derive(Component)]
pub(crate) struct JailText;

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
    /// The last shot, latched for presentation.
    ///
    /// Simulation events live for exactly one tick and presentation runs in
    /// `Update`, so a flash driven straight off `world.events` would fire on
    /// some frames and be missed on others. Latching it here is what lets the
    /// muzzle, the impact light and the recoil all read from one fact.
    pub last_shot: Option<ShotFeedback>,
}

/// What presentation needs to know about the shot that just happened.
#[derive(Clone, Copy, Debug)]
pub struct ShotFeedback {
    pub at: Vec3,
    pub fate: ShoveFate,
    /// Seconds since it happened. Every effect fades on this one clock.
    pub age: f32,
    /// A refusal is a shot too, and has to look like one.
    pub refused: bool,
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
            last_note: "Look at a Guardian and put it into the architecture.".to_string(),
            last_shot: None,
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
        // Walls carry their own light. With one key light and no shadow maps, a
        // wall face pointing away from it falls to ambient and reads as a void
        // hole rather than as a surface — which in a lab about edges is the one
        // confusion that must not happen.
        wall: emissive_material(&mut materials, COLOR_PLATE_WALL, 1.6),
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

    // Everything held in view is a child of the camera, so it inherits the look
    // for free and the recoil is a local offset rather than a world calculation.
    let view_tool = commands
        .spawn((
            FpsOwned,
            ViewModel {
                home: VIEWMODEL_HOME,
            },
            Mesh3d(meshes.add(Cuboid::new(
                VIEWMODEL_SIZE.x,
                VIEWMODEL_SIZE.y,
                VIEWMODEL_SIZE.z,
            ))),
            MeshMaterial3d(emissive_material(
                &mut materials,
                Color::srgb(0.42, 0.78, 0.95),
                1.1,
            )),
            Transform::from_translation(VIEWMODEL_HOME).with_rotation(Quat::from_rotation_y(-0.20)),
            Name::new("Kinetic Tool"),
        ))
        .with_children(|tool| {
            // An emitter ring at the business end, so the thing reads as a tool
            // pointed somewhere rather than a floating brick.
            tool.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.085, 0.05))),
                MeshMaterial3d(emissive_material(
                    &mut materials,
                    Color::srgb(0.55, 1.0, 0.95),
                    5.0,
                )),
                Transform::from_xyz(0.0, 0.0, -VIEWMODEL_SIZE.z / 2.0 - 0.02)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                Name::new("Emitter"),
            ));
            tool.spawn((
                MuzzleFlash,
                PointLight {
                    color: Color::srgb(0.6, 1.0, 0.95),
                    intensity: 0.0,
                    range: 14.0,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, -VIEWMODEL_SIZE.z / 2.0 - 0.10),
                Name::new("Muzzle Flash"),
            ));
        })
        .id();

    let camera = commands
        .spawn((
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
        ))
        .id();
    // Hang the tool off the camera so it inherits the look for free and recoil
    // stays a local offset rather than a world-space calculation.
    commands.entity(camera).add_child(view_tool);

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

    // Walls between plates, from the solver's door mask. On the authored board
    // there are none, which is exactly why it plays as an open plain.
    for (coord, face) in crate::embodied::wall_faces(&world) {
        let Some((offset, half)) = crate::embodied::wall_slab(face) else {
            continue;
        };
        let center = plate_center(coord);
        commands.spawn((
            FpsOwned,
            Mesh3d(meshes.add(Cuboid::new(half.x * 2.0, half.y * 2.0, half.z * 2.0))),
            MeshMaterial3d(palette.wall.clone()),
            Transform::from_xyz(center.x + offset.x, offset.y, center.z + offset.z),
            Name::new(format!("Wall {},{} {face:?}", coord.q, coord.r)),
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

    // A fixed pool, not one shell per starting Guardian: a siege spawns waves,
    // and `MAX_LIVE_MINORS` is the ceiling the model enforces so this pool can
    // exist at all. Spawning entities mid-match instead would put entity
    // lifetime on the hot path and give the reset-leak test something to fail on.
    for slot in 0..crate::model::MAX_LIVE_MINORS {
        let id = MinorGuardianId(slot as u32);
        let position = Vec3::new(0.0, FLOOR_TOP + 1.1, 0.0);
        commands.spawn((
            FpsOwned,
            MinorShell(id),
            Clockwork::at(position),
            Mesh3d(minor_mesh.clone()),
            MeshMaterial3d(palette.minor.clone()),
            Transform::from_translation(position),
            Visibility::Hidden,
            Name::new(format!("Minor Guardian {slot}")),
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
        TargetRing,
        Mesh3d(meshes.add(Cylinder::new(1.5, 0.08))),
        MeshMaterial3d(palette.preview_safe.clone()),
        Transform::from_xyz(0.0, FLOOR_TOP + 0.15, 0.0),
        Visibility::Hidden,
        Name::new("Target Ring"),
    ));

    commands.spawn((
        FpsOwned,
        ImpactFlash,
        PointLight {
            color: Color::srgb(0.5, 1.0, 0.7),
            intensity: 0.0,
            range: 26.0,

            ..default()
        },
        Transform::from_xyz(0.0, FLOOR_TOP + 1.5, 0.0),
        Name::new("Impact Flash"),
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
                    width: px(400),
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
                         P pause   R reset   ESC free the cursor\n\
                         \n\
                         Aim anywhere. Look at a Guardian, not down a row.\n\
                         The outlined plates are what the tool can reach.\n\
                         \n\
                         crosshair open, amber: seen, too far to grab\n\
                         crosshair open, red: a wall is in the way\n\
                         crosshair shut, white: grabbed\n\
                         crosshair shut, green: this push kills",
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
            JailText,
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
    // what makes a working trigger feel like a dead one. A single square could
    // only say "target / no target"; four arms around a dot can say range and
    // capability at the same time, in the spread-and-colour language every
    // player already reads.
    commands
        .spawn((
            FpsOwned,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: percent(50),
                width: px(0),
                height: px(0),
                ..default()
            },
            GlobalZIndex(21),
            Name::new("Reticle"),
        ))
        .with_children(|reticle| {
            reticle.spawn((
                Crosshair,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(-CROSSHAIR_THICKNESS / 2.0),
                    top: px(-CROSSHAIR_THICKNESS / 2.0),
                    width: px(CROSSHAIR_THICKNESS),
                    height: px(CROSSHAIR_THICKNESS),
                    ..default()
                },
                BackgroundColor(CROSSHAIR_IDLE),
                Name::new("Crosshair Dot"),
            ));
            for dir in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
                let arm = CrosshairArm { dir };
                let (left, top, width, height) = arm.layout(CROSSHAIR_GAP_MAX, CROSSHAIR_ARM);
                reticle.spawn((
                    arm,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(left),
                        top: px(top),
                        width: px(width),
                        height: px(height),
                        ..default()
                    },
                    BackgroundColor(CROSSHAIR_IDLE),
                    Name::new("Crosshair Arm"),
                ));
            }
        });
}

pub(crate) fn present_jail_overlay(
    world: Res<KineticWorld>,
    mut overlay: Query<(&mut Visibility, &mut BackgroundColor), With<JailOverlay>>,
    mut text: Query<&mut Text, With<JailText>>,
) {
    let jailed = world
        .observers
        .first()
        .is_some_and(|observer| observer.jailed);
    let survived = world.outcome == MatchOutcome::Survived;

    if let Ok((mut visibility, mut color)) = overlay.single_mut() {
        *visibility = if jailed || survived {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        // Outlasting a siege earns a different colour from losing one: the
        // same red panel for both would make a win read as a failure.
        color.0 = if survived {
            Color::srgba(0.02, 0.24, 0.16, 0.45)
        } else {
            Color::srgba(0.35, 0.02, 0.10, 0.45)
        };
    }
    if let Ok(mut text) = text.single_mut() {
        **text = if survived {
            format!(
                "SURVIVED\n\nYou outlasted {} waves.\n{} Guardians put into the architecture.\nPress R to reset.",
                world.waves_released, world.kills
            )
        } else if world.siege.enabled {
            format!(
                "OVERRUN\n\nA Guardian reached you on wave {}.\n{} Guardians put into the architecture.\nPress R to reset.",
                world.waves_released, world.kills
            )
        } else {
            "JAILED\n\nA Guardian reached you.\nPress R to reset.".to_string()
        };
    }
}

/// Spread and colour the reticle by what the trigger would actually do.
pub(crate) fn present_crosshair(
    world: Res<KineticWorld>,
    mut dot: Query<&mut BackgroundColor, (With<Crosshair>, Without<CrosshairArm>)>,
    mut arms: Query<(&CrosshairArm, &mut BackgroundColor, &mut Node)>,
) {
    let Some(observer) = world.observers.first() else {
        return;
    };
    let (color, gap, length) = crosshair_look(world.aim_state(observer));

    if let Ok(mut dot) = dot.single_mut() {
        dot.0 = color;
    }
    for (arm, mut background, mut node) in &mut arms {
        let (left, top, width, height) = arm.layout(gap, length);
        background.0 = color;
        node.left = px(left);
        node.top = px(top);
        node.width = px(width);
        node.height = px(height);
    }
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
    // Rebuild the board this run actually asked for. Resetting to the authored
    // rectangle would drop a siege back onto the proving plain.
    let rules = world.rules;
    *world = crate::build_world(rules);
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

    // Latch the shot. The *last* link of a chain is the interesting one — it is
    // where the cascade ended — but any link firing at all means the tool went
    // off, so the muzzle reads from the same latch.
    for event in &world.events {
        match event {
            KineticEvent::Shoved(resolution) => {
                let center = plate_center(resolution.to);
                runtime.last_shot = Some(ShotFeedback {
                    at: Vec3::new(center.x, FLOOR_TOP + 1.2, center.z),
                    fate: resolution.fate,
                    age: 0.0,
                    refused: false,
                });
            }
            KineticEvent::ToolRefused { .. } => {
                let eye = embodiment.body.eye(&embodiment.config);
                runtime.last_shot = Some(ShotFeedback {
                    at: eye + embodiment.body.look_dir() * 3.0,
                    fate: ShoveFate::Blocked,
                    age: 0.0,
                    refused: true,
                });
            }
            _ => {}
        }
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
            ShoveFate::Slammed => "SLAM - driven into the wall.".to_string(),
            ShoveFate::Transferred => "Struck another Guardian - momentum carries on.".to_string(),
            ShoveFate::Blocked => "Out of momentum against structure.".to_string(),
        },
        KineticEvent::GuardianDestroyed { by_retraction, .. } => {
            if *by_retraction {
                "A plate committed and took its passenger.".to_string()
            } else {
                "Minor Guardian committed to void.".to_string()
            }
        }
        KineticEvent::ToolRefused { refusal, .. } => match refusal {
            ToolRefusal::NoTarget => "No Guardian under the crosshair.".to_string(),
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
        KineticEvent::WaveReleased { index, size } => {
            format!("Wave {} incoming: {size} Guardians.", index + 1)
        }
        // Individually uninteresting next to the wave line that follows it.
        KineticEvent::MinorReleased { .. } => String::new(),
        KineticEvent::SiegeEnded { outcome } => match outcome {
            MatchOutcome::Survived => "You outlasted the siege.".to_string(),
            MatchOutcome::Lost => "The siege took you.".to_string(),
            MatchOutcome::Running => String::new(),
        },
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
        &'static mut Visibility,
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
        // The pool is bigger than the population: a slot with no Guardian in it
        // yet simply stays hidden until a wave fills it.
        let Some(minor) = world.minor(shell.0) else {
            *visibility = Visibility::Hidden;
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

    if let Ok((mut clockwork, mut transform, mut material, mut visibility)) = major.single_mut() {
        // Switched off by `--no-major`: out of play, so out of sight.
        *visibility = if world.major.enabled {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
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
    let resolved = world
        .observers
        .first()
        .and_then(|observer| world.preview_push(observer));
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

/// Outline every plate the tool can reach, and draw where a shot would land.
///
/// This replaced a line drawn straight down the Observer's facing face, which
/// was a leftover from lane targeting and actively taught the wrong rule: it
/// said "shots go this way" when selection has been a 45-degree cone against
/// real Guardian positions for two commits.
///
/// The reach is drawn as *plates*, not as a wedge swept from the aim vector,
/// for two reasons. It is stable — a footprint that swung with every mouse
/// movement would be noise rather than information — and a ground arc 42 metres
/// out sits within a couple of degrees of eye level in a first-person view,
/// where it reads as a horizon line and says nothing. Outlined plates lie under
/// the player's feet and ahead of them, at every depth, and answer the actual
/// question: *anything standing on one of these can be grabbed.* The crosshair
/// covers the other half, which is where within that footprint you are pointing.
pub(crate) fn draw_reach(world: Res<KineticWorld>, mut gizmos: Gizmos) {
    let Some(observer) = world.observers.first() else {
        return;
    };

    // A hand's width over the plate surface. `PLATE_THICKNESS` is how far a
    // plate hangs *below* `FLOOR_TOP`, not a surface offset — reading it as one
    // floated these lines 1.08 metres up, half a metre under the eye, where
    // every plate past about fifteen metres folded onto the horizon and the
    // whole footprint read as a stray skyline.
    let height = FLOOR_TOP + 0.05;
    let reach = Color::srgba(0.45, 0.92, 1.0, 0.5);

    for cell in world.standable_cells() {
        if lateral_distance(cell, observer.cell) > TOOL_RANGE {
            continue;
        }
        // A plate behind a wall is not reachable, and drawing it as if it were
        // would be the Legibility Contract broken in the player's favour and
        // then against it a second later.
        if !world.has_clear_line(observer.cell, cell) {
            continue;
        }
        let center = plate_center(cell);
        // Inset a little so neighbouring plates read as two edges, not one.
        let (x, z) = (PLATE_HALF_X - 0.45, PLATE_HALF_Z - 0.45);
        let corner = |dx: f32, dz: f32| Vec3::new(center.x + dx, height, center.z + dz);
        let corners = [corner(-x, -z), corner(x, -z), corner(x, z), corner(-x, z)];
        for pair in 0..4 {
            gizmos.line(corners[pair], corners[(pair + 1) % 4], reach);
        }
    }

    let Some(resolution) = world.preview_push(observer) else {
        return;
    };

    let color = fate_color(resolution.fate);

    // Walk the same faces the resolution walked, so the drawn path is the
    // resolved path rather than a straight line that might cut a corner. Drawn
    // just above the reach outlines so a shove reads over its own footprint.
    let mut cursor = resolution.from;
    let height = FLOOR_TOP + 0.12;
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

/// The lab's cues.
///
/// Every one of these is an existing CC0 `.ogg` from `assets/sounds`, generated
/// in-repo by `tools/generate_audio.py` from no external source material. They
/// are placeholders in the sense that they were authored for other events, not
/// in the sense that anything needs licensing or downloading.
#[derive(Resource)]
pub(crate) struct KineticCues {
    fire: Handle<AudioSource>,
    kill: Handle<AudioSource>,
    refused: Handle<AudioSource>,
    recharge: Handle<AudioSource>,
    jailed: Handle<AudioSource>,
}

/// Which generated `.ogg` stands in for each event.
///
/// Named as consts rather than written inline at the `load` calls so a test can
/// check every one has a file behind it. A missing asset is not an error the
/// player ever sees: Bevy logs a line and the sound simply never plays, which
/// is indistinguishable from the cue not having been written.
const CUE_FIRE: AssetSlot = observed_assets::TOOL_INTERACT;
const CUE_KILL: AssetSlot = observed_assets::COLLAPSE_STING;
const CUE_REFUSED: AssetSlot = observed_assets::UI_CLICK;
/// Not `CHIME`: that slot is declared but nothing was ever generated into it,
/// so the tool went quiet exactly when it had good news. `KEYSTONE` is a real
/// pickup signal and reads the same way.
const CUE_RECHARGE: AssetSlot = observed_assets::KEYSTONE;
const CUE_JAILED: AssetSlot = observed_assets::KLAXON;
#[cfg_attr(not(test), expect(dead_code, reason = "the test is the only consumer"))]
const CUE_SLOTS: [AssetSlot; 5] = [CUE_FIRE, CUE_KILL, CUE_REFUSED, CUE_RECHARGE, CUE_JAILED];

pub(crate) fn load_cues(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(KineticCues {
        fire: assets.load(CUE_FIRE.path),
        kill: assets.load(CUE_KILL.path),
        refused: assets.load(CUE_REFUSED.path),
        recharge: assets.load(CUE_RECHARGE.path),
        jailed: assets.load(CUE_JAILED.path),
    });
}

/// Sound the tick's events.
///
/// Reads `world.events` in the same schedule that produced them, so nothing is
/// missed between a fixed tick and a rendered frame.
pub(crate) fn play_cues(mut commands: Commands, world: Res<KineticWorld>, cues: Res<KineticCues>) {
    for event in &world.events {
        let (source, volume) = match event {
            // The shove itself, then a separate sting if it actually removed
            // something — so a kill is audibly different from a stagger without
            // needing two different shove sounds.
            KineticEvent::Shoved(resolution) => {
                if resolution.fate == ShoveFate::Transferred {
                    // A link in a chain, not a trigger pull. Silent: the first
                    // link already fired and a burst of these is a mess.
                    continue;
                }
                (cues.fire.clone(), 0.55)
            }
            KineticEvent::GuardianDestroyed { .. } => (cues.kill.clone(), 0.7),
            KineticEvent::ToolRefused { .. } => (cues.refused.clone(), 0.45),
            KineticEvent::ChargeRestored { .. } => (cues.recharge.clone(), 0.3),
            KineticEvent::ObserverCaptured { .. } => (cues.jailed.clone(), 0.8),
            _ => continue,
        };
        commands.spawn((
            FpsOwned,
            AudioPlayer(source),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Despawn,
                volume: bevy::audio::Volume::Linear(volume),
                ..PlaybackSettings::DESPAWN
            },
            Name::new("Kinetic Cue"),
        ));
    }
}

/// Put a ring under whatever the tool has selected.
///
/// With a 45-degree cone the crosshair alone no longer says *which* Guardian is
/// about to be grabbed. This says it on the Guardian, where the player is
/// already looking, and colours it by what the shot would do.
pub(crate) fn present_target_ring(
    world: Res<KineticWorld>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ring: Query<
        (
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<TargetRing>,
    >,
) {
    let Ok((mut transform, mut visibility, mut material)) = ring.single_mut() else {
        return;
    };
    let selected = world.observers.first().and_then(|observer| {
        let id = world.target_in_cone(observer)?;
        let minor = world.minor(id)?;
        Some((minor.cell, world.preview_push(observer)))
    });
    let Some((cell, preview)) = selected else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Inherited;
    let center = plate_center(cell);
    transform.translation = Vec3::new(center.x, FLOOR_TOP + 0.15, center.z);

    let color = preview.map_or(Color::srgb(0.6, 0.66, 0.74), |shot| fate_color(shot.fate));
    material.0 = emissive_material(&mut materials, color, 3.0);
}

// The disjointness filters are what let one system hold all three at once.
type ToolQuery<'w, 's> =
    Query<'w, 's, (&'static ViewModel, &'static mut Transform), Without<MuzzleFlash>>;
type MuzzleQuery<'w, 's> =
    Query<'w, 's, &'static mut PointLight, (With<MuzzleFlash>, Without<ImpactFlash>)>;
type ImpactQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut PointLight, &'static mut Transform),
    (With<ImpactFlash>, Without<ViewModel>),
>;

/// Drive the recoil, the muzzle and the impact light off the latched shot.
pub(crate) fn present_shot_feedback(
    time: Res<Time>,
    mut runtime: ResMut<FpsRuntime>,
    mut tool: ToolQuery,
    mut muzzle: MuzzleQuery,
    mut impact: ImpactQuery,
) {
    let delta = time.delta_secs();
    if let Some(shot) = runtime.last_shot.as_mut() {
        shot.age += delta;
    }
    let shot = runtime.last_shot;
    // 1 at the instant of firing, 0 once the flash has run its course.
    let intensity = shot.map_or(0.0, |shot| {
        (1.0 - (shot.age / FLASH_SECONDS).clamp(0.0, 1.0)).powi(2)
    });
    let refused = shot.is_some_and(|shot| shot.refused);

    if let Ok((model, mut transform)) = tool.single_mut() {
        // A refusal twitches rather than kicks: the tool did something, and it
        // was not a shot.
        let kick = if refused {
            RECOIL_DISTANCE * 0.25
        } else {
            RECOIL_DISTANCE
        };
        transform.translation = model.home + Vec3::new(0.0, 0.0, kick * intensity);
    }

    if let Ok(mut light) = muzzle.single_mut() {
        light.intensity = if refused { 0.0 } else { 120_000.0 * intensity };
    }

    if let Ok((mut light, mut transform)) = impact.single_mut() {
        match shot {
            Some(shot) if !shot.refused => {
                transform.translation = shot.at;
                light.color = fate_color(shot.fate);
                light.intensity = 220_000.0 * intensity;
            }
            _ => light.intensity = 0.0,
        }
    }
}

/// One colour per outcome. Green always means "this removes it".
fn fate_color(fate: ShoveFate) -> Color {
    match fate {
        ShoveFate::Void => Color::srgb(0.25, 1.0, 0.5),
        ShoveFate::Slammed => Color::srgb(0.45, 1.0, 0.35),
        ShoveFate::Doomed => Color::srgb(1.0, 0.72, 0.22),
        ShoveFate::Transferred => Color::srgb(0.55, 0.85, 1.0),
        ShoveFate::Rest => Color::srgb(0.6, 0.66, 0.74),
        ShoveFate::Blocked => Color::srgb(1.0, 0.28, 0.24),
    }
}

/// The siege clock, wave count and kills — the only score this lab keeps.
///
/// Empty when there is no siege, so the ordinary board's HUD is unchanged.
fn siege_line(world: &KineticWorld) -> String {
    if !world.siege.enabled {
        return String::new();
    }
    let remaining = world.siege_remaining();
    let seconds = remaining / crate::model::TICKS_PER_SECOND;
    format!(
        "SIEGE  {:01}:{:02} left   |   wave {}   |   killed {}\n",
        seconds / 60,
        seconds % 60,
        world.waves_released,
        world.kills,
    )
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
    // What the reticle is saying, in words, so a screenshot of the HUD and a
    // screenshot of the crosshair can be checked against each other.
    let aim = match world.aim_state(observer) {
        AimState::Empty => "nothing in view".to_string(),
        AimState::OutOfReach { cells } => {
            format!("out of reach ({cells} plates, tool reaches {TOOL_RANGE})")
        }
        AimState::Occluded => "blocked by architecture".to_string(),
        AimState::Reach { .. } => world.preview_push(observer).map_or_else(
            || "target".to_string(),
            |resolution| {
                format!(
                    "{:?} after {} cells",
                    resolution.fate, resolution.cells_travelled
                )
            },
        ),
    };

    **text = format!(
        "{}tick {}   |   cell {},{}   |   facing {:?}\n\
         charge {}/{}   |   power {}   |   rules {}\n\
         minors alive {}   |   major {}\n\
         aim: {}\n\
         resets {}   |   {}\n\
         {}",
        siege_line(&world),
        world.tick,
        observer.cell.q,
        observer.cell.r,
        embodiment.facing,
        observer.charge,
        MAX_CHARGE,
        if world.powered { "ON" } else { "OUT" },
        world.rules.summary(),
        world.living_minors(),
        if world.major.frozen {
            "FROZEN (observed)"
        } else {
            "awake"
        },
        aim,
        runtime.reset_count,
        if observer.jailed { "JAILED" } else { "free" },
        runtime.last_note,
    );
}

/// A Guardian's snap must finish well inside its step interval, or the clockwork
/// read turns into a glide. Enforced at compile time so tuning one without the
/// other cannot quietly soften the whole shape language.
const _: () = assert!(SNAP_SECONDS * 60.0 < crate::model::MINOR_STEP_TICKS as f32);

#[cfg(test)]
mod tests {
    use super::*;

    /// A cue with no file behind it is a silent failure: the tool just never
    /// makes that sound. `CHIME` was exactly this for the whole first pass.
    #[test]
    fn every_cue_has_a_file_behind_it() {
        let root = observed_assets::assets_root();
        for slot in CUE_SLOTS {
            let path = root.join(slot.path);
            assert!(
                path.is_file(),
                "cue `{}` points at {} which does not exist",
                slot.name,
                path.display()
            );
        }
    }

    /// The reticle's two readings must stay independent: range lives in the
    /// gap, capability in the colour. If a state got both from the same source
    /// the player would have no way to tell "walk closer" from "look elsewhere".
    #[test]
    fn the_reticle_says_range_and_capability_separately() {
        let (empty, empty_gap, _) = crosshair_look(AimState::Empty);
        let (blocked, blocked_gap, _) = crosshair_look(AimState::Occluded);
        let (target, target_gap, _) = crosshair_look(AimState::Reach { kills: 0 });

        assert_eq!(
            (empty_gap, blocked_gap),
            (CROSSHAIR_GAP_MAX, CROSSHAIR_GAP_MAX),
            "with nothing grabbable the reticle stays open"
        );
        assert_eq!(
            target_gap, CROSSHAIR_GAP_SHUT,
            "the reticle shuts the moment the tool can grab"
        );
        assert_ne!(
            empty, blocked,
            "an empty cone and a wall in the way must not look identical"
        );
        assert_ne!(empty, target);
    }

    /// Range is the gap, and it has to close monotonically as you walk in, or
    /// it is decoration rather than a meter.
    #[test]
    fn the_gap_closes_as_the_target_comes_into_reach() {
        let gap = |cells| crosshair_look(AimState::OutOfReach { cells }).1;
        let far = gap(TOOL_RANGE + 6);
        let near = gap(TOOL_RANGE + 1);
        assert!(
            far > near,
            "walking closer must tighten the reticle: {far} then {near}"
        );
        assert!(
            near > crosshair_look(AimState::Reach { kills: 0 }).1,
            "in reach must be tighter than merely nearly in reach"
        );
        assert_eq!(far, CROSSHAIR_GAP_MAX, "and it is clamped, not unbounded");
    }

    /// A bigger shot reads as a bigger reticle, not merely a differently
    /// coloured one.
    #[test]
    fn a_chain_lengthens_the_arms() {
        let arm = |kills| crosshair_look(AimState::Reach { kills }).2;
        assert_eq!(arm(1), CROSSHAIR_ARM);
        assert!(arm(3) > arm(1));
        assert_eq!(arm(9), arm(4), "and it is clamped so it cannot run away");
    }

    /// The four arms must be a symmetric cross. An off-by-one here puts the
    /// centre of aim somewhere other than the centre of the screen.
    #[test]
    fn the_four_arms_are_symmetric_about_the_centre() {
        let gap = 7.0;
        let length = 9.0;
        let up = CrosshairArm { dir: (0.0, -1.0) }.layout(gap, length);
        let down = CrosshairArm { dir: (0.0, 1.0) }.layout(gap, length);
        let left = CrosshairArm { dir: (-1.0, 0.0) }.layout(gap, length);
        let right = CrosshairArm { dir: (1.0, 0.0) }.layout(gap, length);

        assert_eq!(up.1, -(gap + length));
        assert_eq!(down.1, gap);
        assert_eq!(left.0, -(gap + length));
        assert_eq!(right.0, gap);
        // Vertical arms are tall and thin, horizontal ones wide and thin.
        assert_eq!((up.2, up.3), (CROSSHAIR_THICKNESS, length));
        assert_eq!((right.2, right.3), (length, CROSSHAIR_THICKNESS));
        // And the cross axis is centred on the screen centre in both.
        assert_eq!(up.0, -CROSSHAIR_THICKNESS / 2.0);
        assert_eq!(right.1, -CROSSHAIR_THICKNESS / 2.0);
    }
}
