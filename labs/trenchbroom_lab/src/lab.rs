//! The Bevy projection of the imported facility: presentation, first-person
//! traversal, debug overlays, reset, and the evidence capture.
//!
//! Simulation/presentation separation is preserved: the domain [`Facility`] (from
//! [`crate::project`]) is the source of truth; this module only *renders* it (boxes
//! through `observed_style`), feeds its collision to the shared `observed_traversal`
//! controller, and draws gizmo overlays for the otherwise-invisible imported
//! metadata (room bounds, ports, door state, spawn).

use std::path::Path;

use bevy::{
    app::AppExit,
    input::{InputSystems, mouse::AccumulatedMouseMotion},
    pbr::{DistanceFog, FogFalloff},
    post_process::bloom::Bloom,
    prelude::*,
    render::view::{
        Hdr,
        screenshot::{Screenshot, save_to_disk},
    },
    window::{CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow, WindowResolution},
};
use bevy_trenchbroom::config::WriteTrenchBroomConfigOnStartPlugin;
use bevy_trenchbroom::prelude::{TrenchBroomConfig, TrenchBroomPlugins, TrenchBroomServer};
use bevy_trenchbroom::qmap::QuakeMap;
use observed_style::{MarkerRole, SurfaceRole, Treatment, marker, surface};
use observed_traversal::{Aabb3, FIXED_DT, FpsArena, FpsBody, FpsConfig, step_body};
use player_input::PlayerIntent;

use crate::map_source;
use crate::project::{DoorState, Facility, RoomKind, project};

const MOUSE_SENS: f32 = 0.06;
/// Absolute asset root for this lab. Bevy resolves a relative `AssetPlugin.file_path`
/// against `CARGO_MANIFEST_DIR` (the package dir) at runtime, so an absolute path
/// derived from it is unambiguous regardless of the working directory.
const ASSET_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
/// Asset-relative path the loader resolves under [`ASSET_DIR`].
const MAP_PATH: &str = "maps/facility.map";

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct PlayerCam;

#[derive(Component)]
pub struct LabUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

/// Spawned structural geometry (imported brushes). Despawned on reset.
#[derive(Component)]
struct ImportedGeometry;

/// A spawned door-leaf mesh (only present while the door is closed).
#[derive(Component)]
struct DoorLeafMesh;

/// Query filter for everything `apply_geometry` (re)spawns from the import.
type SpawnedGeometry = Or<(With<ImportedGeometry>, With<DoorLeafMesh>)>;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// The handle for the asynchronously-loaded authored map.
#[derive(Resource)]
struct MapHandle(Handle<QuakeMap>);

#[derive(Resource)]
pub struct Lab {
    pub facility: Option<Facility>,
    pub door_states: Vec<DoorState>,
    pub arena: FpsArena,
    pub config: FpsConfig,
    pub body: FpsBody,
    pub debug_visible: bool,
    pub reset_count: u32,
    pub last_event: String,
    /// Set when geometry needs to be (re)spawned (after load / reset / door toggle).
    geometry_dirty: bool,
    /// Capture-only fixed camera.
    pub camera_override: Option<Transform>,
}

impl Default for Lab {
    fn default() -> Self {
        Self {
            facility: None,
            door_states: Vec::new(),
            arena: FpsArena {
                solids: Vec::new(),
                floor_y: 0.0,
                floor_half: 20.0,
            },
            config: FpsConfig::default(),
            body: FpsBody::spawned(Vec3::ZERO, 0.0),
            debug_visible: true,
            reset_count: 0,
            last_event: "Loading authored map…".to_string(),
            geometry_dirty: false,
            camera_override: None,
        }
    }
}

impl Lab {
    pub fn loaded(&self) -> bool {
        self.facility.is_some()
    }

    /// Adopt a freshly-projected facility (from the asset load), reset to its authored
    /// state, and mark presentation dirty.
    pub fn adopt(&mut self, facility: Facility) {
        self.door_states = facility.doors.iter().map(|d| d.state).collect();
        self.facility = Some(facility);
        self.reset_to_authored();
    }

    fn reset_to_authored(&mut self) {
        // Capture authored values up front so the `&self.facility` borrow ends before
        // we mutate `self`.
        let Some((door_states, spawn_ground, floor_y, spawn_yaw)) =
            self.facility.as_ref().map(|f| {
                (
                    f.doors.iter().map(|d| d.state).collect::<Vec<_>>(),
                    f.spawn_ground,
                    f.floor_y,
                    f.spawn_yaw,
                )
            })
        else {
            return;
        };
        self.door_states = door_states;
        self.rebuild_arena();
        self.body = FpsBody::spawned(
            Vec3::new(
                spawn_ground.x,
                floor_y + self.config.half_height,
                spawn_ground.z,
            ),
            spawn_yaw,
        );
        self.geometry_dirty = true;
        self.last_event = "Imported facility ready. WASD to walk it.".to_string();
    }

    fn rebuild_arena(&mut self) {
        let Some(facility) = &self.facility else {
            return;
        };
        let mut solids = facility.solids.clone();
        let floor_y = facility.floor_y;
        let floor_half = facility.floor_half;
        let leaves: Vec<(usize, Aabb3)> = facility
            .doors
            .iter()
            .enumerate()
            .map(|(i, d)| (i, d.leaf))
            .collect();
        for (i, leaf) in leaves {
            if self.door_states.get(i) == Some(&DoorState::Closed) {
                solids.push(leaf);
            }
        }
        self.arena = FpsArena {
            solids,
            floor_y,
            floor_half,
        };
    }

    pub fn toggle_doors(&mut self) {
        if !self.loaded() {
            return;
        }
        for s in &mut self.door_states {
            *s = match s {
                DoorState::Open => DoorState::Closed,
                DoorState::Closed => DoorState::Open,
            };
        }
        self.rebuild_arena();
        self.geometry_dirty = true;
        self.last_event = "Toggled the imported doors (collision follows the model).".to_string();
    }

    pub fn reset(&mut self) {
        self.reset_count += 1;
        self.reset_to_authored();
        self.last_event = format!("Reset #{} — back to the authored map.", self.reset_count);
    }

    pub fn step(&mut self, intent: PlayerIntent) {
        if self.facility.is_some() {
            // Disjoint field borrows: body (mut) and arena/config (shared) are
            // distinct fields, so the controller steps without cloning the arena.
            let config = self.config;
            step_body(&mut self.body, intent, &self.arena, &config, FIXED_DT);
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin & app
// ---------------------------------------------------------------------------

pub struct TrenchBroomLabPlugin;

impl Plugin for TrenchBroomLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Lab>()
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.45, 0.55, 0.8),
                brightness: 320.0,
                ..default()
            })
            // The map-file write and async asset load live in `run()`, not here, so
            // the plugin can be exercised headlessly (the lifecycle tests inject a
            // projected facility directly).
            .add_systems(Startup, setup_lab)
            .add_systems(FixedUpdate, simulate)
            .add_systems(
                Update,
                (
                    await_load,
                    handle_toggles.after(InputSystems),
                    toggle_grab,
                    apply_geometry,
                    present_camera,
                    draw_debug,
                    update_debug_text,
                )
                    .chain(),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.01, 0.012, 0.02)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Observed 2 — TrenchBroom Lab (Phase A1)".to_string(),
                        resolution: WindowResolution::new(1440, 900),
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: ASSET_DIR.to_string(),
                    ..default()
                }),
        )
        // bevy_trenchbroom as the importer. We never spawn its scene — every brush is
        // textured `__TB_empty` so it does no material work — but loading the asset
        // runs the full parse pipeline we read `entities` from. Disable the config
        // writer so the lab never writes a TrenchBroom config file.
        .add_plugins(
            TrenchBroomPlugins(
                TrenchBroomConfig::new("observed2_trenchbroom_lab")
                    .assets_path(ASSET_DIR)
                    .suppress_invalid_entity_definitions(true),
            )
            .build()
            .disable::<WriteTrenchBroomConfigOnStartPlugin>(),
        )
        .add_plugins(TrenchBroomLabPlugin)
        // App-only wiring: materialise the map artifact, then queue the async load.
        .add_systems(Startup, ((ensure_map_file, load_map).chain(), grab_cursor));

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

/// Materialise the generated `.map` so the asset loader has it. The map is a pure
/// function of [`map_source::facility_map`]; we only write when the on-disk content
/// differs, so the committed artifact stays authoritative but can never drift.
fn ensure_map_file() {
    let path = Path::new(ASSET_DIR).join(MAP_PATH);
    let wanted = map_source::facility_map();
    let current = std::fs::read_to_string(&path).ok();
    if current.as_deref() != Some(wanted.as_str()) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(err) = std::fs::write(&path, wanted) {
            warn!("could not write {}: {err}", path.display());
        }
    }
}

fn setup_lab(mut commands: Commands) {
    commands.spawn((
        PlayerCam,
        Camera3d::default(),
        // Neon-noir atmosphere, matching the assembled game's Match camera.
        Hdr,
        Bloom {
            intensity: 0.08,
            ..Bloom::NATURAL
        },
        DistanceFog {
            color: Color::srgb(0.01, 0.015, 0.03),
            falloff: FogFalloff::Linear {
                start: 18.0,
                end: 80.0,
            },
            ..default()
        },
        Transform::from_xyz(0.0, 1.6, 0.0),
        Name::new("First-Person Camera"),
    ));
    spawn_ui(&mut commands);
}

fn load_map(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle = asset_server.load::<QuakeMap>(MAP_PATH);
    commands.insert_resource(MapHandle(handle));
}

fn grab_cursor(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

// ---------------------------------------------------------------------------
// Loading → projection
// ---------------------------------------------------------------------------

/// Once the asynchronously-loaded `QuakeMap` is available, project it into the domain
/// [`Facility`] and adopt it. Runs until the map is loaded, then is a no-op.
fn await_load(
    handle: Option<Res<MapHandle>>,
    maps: Option<Res<Assets<QuakeMap>>>,
    server: Option<Res<TrenchBroomServer>>,
    mut lab: ResMut<Lab>,
) {
    if lab.loaded() {
        return;
    }
    let (Some(handle), Some(maps), Some(server)) = (handle, maps, server) else {
        return;
    };
    let Some(map) = maps.get(&handle.0) else {
        return;
    };
    let facility = project(&map.entities, &server.config);
    info!(
        "imported facility: {} rooms, {} ports, {} doors, {} solids",
        facility.rooms.len(),
        facility.ports.len(),
        facility.doors.len(),
        facility.solids.len(),
    );
    lab.adopt(facility);
}

// ---------------------------------------------------------------------------
// Presentation
// ---------------------------------------------------------------------------

fn treatment_material(t: Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: t.base_color,
        emissive: t.emissive,
        perceptual_roughness: 0.85,
        metallic: 0.1,
        ..default()
    }
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    aabb: Aabb3,
    treatment: Treatment,
    tag: impl Bundle,
) {
    let size = aabb.max - aabb.min;
    let center = (aabb.min + aabb.max) * 0.5;
    commands.spawn((
        tag,
        Mesh3d(meshes.add(Cuboid::new(size.x.abs(), size.y.abs(), size.z.abs()))),
        MeshMaterial3d(materials.add(treatment_material(treatment))),
        Transform::from_translation(center),
    ));
}

/// (Re)spawn the imported geometry whenever it is marked dirty: structural solids as
/// neon-noir surfaces, and a leaf mesh for each *closed* door (open doors leave the
/// gap empty). All through `observed_style` — no map material decides a colour.
fn apply_geometry(
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    existing: Query<Entity, SpawnedGeometry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !lab.geometry_dirty {
        return;
    }
    lab.geometry_dirty = false;
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let Some(facility) = lab.facility.clone() else {
        return;
    };

    // Structural geometry. The floor reads as the plain floor; everything raised reads
    // as a wall (the stairs/platform are structural too).
    for solid in &facility.solids {
        let role = if solid.max.y <= facility.floor_y + 0.05 {
            SurfaceRole::Plain
        } else {
            SurfaceRole::Wall
        };
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            *solid,
            surface(role),
            ImportedGeometry,
        );
    }

    // Closed doors get a solid leaf in the gap.
    for (i, door) in facility.doors.iter().enumerate() {
        if lab.door_states.get(i) == Some(&DoorState::Closed) {
            spawn_box(
                &mut commands,
                &mut meshes,
                &mut materials,
                door.leaf,
                surface(SurfaceRole::Wall),
                DoorLeafMesh,
            );
        }
    }
}

fn present_camera(lab: Res<Lab>, mut camera: Single<&mut Transform, With<PlayerCam>>) {
    if let Some(pose) = lab.camera_override {
        **camera = pose;
        return;
    }
    if !lab.loaded() {
        return;
    }
    let eye = lab.body.eye(&lab.config);
    **camera = Transform::from_translation(eye).looking_to(lab.body.look_dir(), Vec3::Y);
}

// ---------------------------------------------------------------------------
// Simulation (fixed timestep)
// ---------------------------------------------------------------------------

fn intent_from_keys(keys: &ButtonInput<KeyCode>, mouse: Vec2) -> PlayerIntent {
    let axis =
        |neg: KeyCode, pos: KeyCode| (keys.pressed(pos) as i32 - keys.pressed(neg) as i32) as f32;
    PlayerIntent {
        movement: Vec2::new(
            axis(KeyCode::KeyA, KeyCode::KeyD),
            axis(KeyCode::KeyS, KeyCode::KeyW),
        ),
        look: Vec2::new(
            axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) + mouse.x * MOUSE_SENS,
            axis(KeyCode::ArrowUp, KeyCode::ArrowDown) + mouse.y * MOUSE_SENS,
        ),
        jump_pressed: keys.pressed(KeyCode::Space),
        sprint_held: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        ..Default::default()
    }
}

fn simulate(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    mut lab: ResMut<Lab>,
) {
    if !lab.loaded() {
        return;
    }
    let mouse = mouse_motion.map(|m| m.delta).unwrap_or(Vec2::ZERO);
    let intent = intent_from_keys(&keys, mouse);
    lab.step(intent);
}

// ---------------------------------------------------------------------------
// Input toggles
// ---------------------------------------------------------------------------

fn handle_toggles(keys: Res<ButtonInput<KeyCode>>, mut lab: ResMut<Lab>) {
    if keys.just_pressed(KeyCode::KeyR) {
        lab.reset();
    }
    if keys.just_pressed(KeyCode::F1) {
        lab.debug_visible = !lab.debug_visible;
    }
    if keys.just_pressed(KeyCode::KeyO) {
        lab.toggle_doors();
    }
}

fn toggle_grab(
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

// ---------------------------------------------------------------------------
// Debug overlay
// ---------------------------------------------------------------------------

fn draw_box_gizmo(gizmos: &mut Gizmos, b: &Aabb3, color: Color) {
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

fn draw_debug(lab: Res<Lab>, mut gizmos: Gizmos) {
    if !lab.debug_visible {
        return;
    }
    let Some(facility) = &lab.facility else {
        return;
    };

    // Room bounds: corridors gold (the spine), rooms cool blue.
    for room in &facility.rooms {
        let aabb = Aabb3 {
            min: room.min,
            max: room.max,
        };
        let color = match room.kind {
            RoomKind::Room => Color::srgb(0.2, 0.55, 1.0),
            RoomKind::Corridor => Color::srgb(1.0, 0.78, 0.3),
        };
        draw_box_gizmo(&mut gizmos, &aabb, color);
    }

    // Ports: a vertical control-coloured marker where two rooms connect.
    let port_color = marker(MarkerRole::Control).base_color;
    for port in &facility.ports {
        gizmos.line(
            port.pos - Vec3::Y * 0.5,
            port.pos + Vec3::Y * 2.5,
            port_color,
        );
        gizmos.cross(port.pos + Vec3::Y * 1.2, 0.6, port_color);
    }

    // Doors: outline at the leaf, green when open (passable), red when closed.
    for (i, door) in facility.doors.iter().enumerate() {
        let open = lab.door_states.get(i) == Some(&DoorState::Open);
        let color = if open {
            Color::srgb(0.2, 1.0, 0.4)
        } else {
            Color::srgb(1.0, 0.25, 0.2)
        };
        draw_box_gizmo(&mut gizmos, &door.leaf, color);
    }

    // Spawn marker.
    let spawn = facility.spawn_ground;
    gizmos.cross(
        spawn + Vec3::Y * 0.1,
        0.8,
        marker(MarkerRole::You).base_color,
    );

    // The player as a vertical marker.
    let p = lab.body.position;
    gizmos.line(
        Vec3::new(p.x, p.y - lab.config.half_height, p.z),
        Vec3::new(p.x, p.y + lab.config.half_height, p.z),
        marker(MarkerRole::You).base_color,
    );
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            LabUiRoot,
            Name::new("TrenchBroom Lab UI Root"),
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
                BackgroundColor(Color::srgba(0.01, 0.02, 0.03, 0.94)),
                BorderColor::all(Color::srgba(0.4, 0.7, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Importing authored map…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.93, 1.0)),
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
                BackgroundColor(Color::srgba(0.01, 0.02, 0.03, 0.94)),
                BorderColor::all(Color::srgba(0.4, 0.7, 1.0, 0.6)),
                children![(
                    Text::new(
                        "TRENCHBROOM LAB (Phase A1 feasibility)\n\
                         WASD       Move (relative to facing)\n\
                         ← → ↑ ↓    Look (yaw / pitch)\n\
                         Shift      Sprint    Space  Jump\n\
                         O          Toggle the imported doors\n\
                         R reset · F1 debug · Esc free cursor\n\n\
                         An authored TrenchBroom .map is imported by bevy_trenchbroom\n\
                         (parser only) and projected into RoomId / PortId / door state\n\
                         and collision. You walk the imported brushes with the shared\n\
                         deterministic controller; presentation flows through the\n\
                         neon-noir style module. Blue = rooms, gold = corridor,\n\
                         purple = ports, green/red = open/closed doors.",
                    ),
                    TextFont {
                        font_size: 13.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.93, 1.0)),
                )],
            ));
        });
}

fn update_debug_text(
    lab: Res<Lab>,
    cams: Query<(), With<PlayerCam>>,
    ui_roots: Query<(), With<LabUiRoot>>,
    imported: Query<(), With<ImportedGeometry>>,
    text: Single<&mut Text, With<DebugText>>,
    mut panel: Single<&mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    mut help: Single<&mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
) {
    let visibility = if lab.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **panel = visibility;
    **help = visibility;

    let cam_count = cams.iter().count();
    let ui_count = ui_roots.iter().count();
    let imported_count = imported.iter().count();

    let (rooms, ports, doors, solids) = lab
        .facility
        .as_ref()
        .map(|f| (f.rooms.len(), f.ports.len(), f.doors.len(), f.solids.len()))
        .unwrap_or((0, 0, 0, 0));

    let healthy = cam_count == 1 && ui_count == 1 && lab.loaded() && rooms > 0 && solids > 0;

    let open_doors = lab
        .door_states
        .iter()
        .filter(|s| **s == DoorState::Open)
        .count();

    *text.into_inner() = Text::new(format!(
        "TRENCHBROOM IMPORT  {}\n\
         status            {}\n\
         rooms             {rooms}\n\
         ports             {ports}\n\
         doors             {doors}  ({open_doors} open)\n\
         imported solids   {solids}\n\
         spawned meshes    {imported_count}\n\
         collision solids  {}\n\
         position          ({:.1}, {:.1}, {:.1})\n\
         grounded          {}\n\
         camera {cam_count}  UI {ui_count}   resets {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if lab.loaded() {
            "imported"
        } else {
            "loading…"
        },
        lab.arena.solids.len(),
        lab.body.position.x,
        lab.body.position.y,
        lab.body.position.z,
        if lab.body.grounded { "yes" } else { "no" },
        lab.reset_count,
        lab.last_event,
    ));
}

// ---------------------------------------------------------------------------
// Evidence capture
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut lab: ResMut<Lab>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    asset_server: Res<AssetServer>,
    handle: Option<Res<MapHandle>>,
) {
    // Wait for the async import before composing the shot — but never hang: if the
    // map has not loaded in time, report its load state and exit.
    if !lab.loaded() {
        if time.elapsed_secs() > 20.0 {
            if let Some(handle) = &handle {
                warn!(
                    "capture timed out before the map loaded; load state = {:?}",
                    asset_server.get_load_state(handle.0.id())
                );
            }
            exit.write(AppExit::Success);
        }
        return;
    }
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // An angled overview that takes in both rooms, the corridor, and the stairs.
        let facility = lab.facility.as_ref().unwrap();
        let center = (facility.bounds_min + facility.bounds_max) * 0.5;
        lab.camera_override = Some(
            Transform::from_translation(center + Vec3::new(10.0, 16.0, 12.0))
                .looking_at(center, Vec3::Y),
        );
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.8 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.6 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};
    use bevy_trenchbroom::qmap::QuakeMapEntities;
    use observed_core::RoomId;

    /// Project the authored map synchronously — the same data path the asset loader
    /// produces, without an async load or a GPU.
    fn test_facility() -> Facility {
        let config = TrenchBroomConfig::new("observed2_trenchbroom_lab");
        let qmap = quake_map::parse(&mut std::io::Cursor::new(map_source::facility_map()))
            .expect("authored map parses");
        let entities = QuakeMapEntities::from_quake_map(qmap, &config);
        project(&entities, &config)
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .add_plugins(TrenchBroomLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn adopting_projects_the_facility_and_spawns_in_room_a() {
        let mut lab = Lab::default();
        assert!(!lab.loaded());
        lab.adopt(test_facility());
        assert!(lab.loaded());

        let facility = lab.facility.clone().unwrap();
        let room_a = facility.room(RoomId(1)).unwrap();
        assert!(lab.body.position.x >= room_a.min.x && lab.body.position.x <= room_a.max.x);
        assert!(lab.body.position.z >= room_a.min.z && lab.body.position.z <= room_a.max.z);
        assert!((lab.body.position.y - (facility.floor_y + lab.config.half_height)).abs() < 1e-3);
    }

    #[test]
    fn a_doors_collision_follows_its_model_state_and_reset_restores_it() {
        let mut lab = Lab::default();
        lab.adopt(test_facility());
        // Door 1 is authored open: its leaf is absent from the collision set.
        let door1_leaf = lab.facility.as_ref().unwrap().doors[0].leaf;
        assert!(!lab.arena.solids.contains(&door1_leaf));
        // Toggle: door 1 closes and its imported leaf now blocks the gap.
        lab.toggle_doors();
        assert!(lab.arena.solids.contains(&door1_leaf));
        // Reset returns to the authored state — open again, leaf gone.
        lab.reset();
        assert!(!lab.arena.solids.contains(&door1_leaf));
    }

    #[test]
    fn walking_forward_traverses_the_imported_collision_and_a_closed_door_stops_it() {
        let mut lab = Lab::default();
        lab.adopt(test_facility());
        let start_z = lab.body.position.z;
        let forward = PlayerIntent {
            movement: Vec2::new(0.0, 1.0),
            ..Default::default()
        };
        for _ in 0..400 {
            lab.step(forward);
        }
        // Walked forward (−Z) through room A's open door into the corridor…
        assert!(
            lab.body.position.z < start_z - 5.0,
            "moved into the facility: {:?}",
            lab.body.position
        );
        // …but the authored-closed second door blocked entry to room B, so the player
        // never reaches it (room B starts well past z = −24).
        let room_b = lab.facility.as_ref().unwrap().room(RoomId(3)).unwrap();
        assert!(
            lab.body.position.z > room_b.max.z + 0.5,
            "the closed imported door stopped the player short of room B: {:?}",
            lab.body.position
        );
    }

    #[test]
    fn boots_with_one_camera_and_one_ui_root() {
        let mut app = test_app();
        assert_eq!(count::<PlayerCam>(&mut app), 1);
        assert_eq!(count::<LabUiRoot>(&mut app), 1);
    }

    #[test]
    fn adopting_spawns_imported_geometry_and_reset_does_not_leak() {
        let mut app = test_app();
        let facility = test_facility();
        let expected = facility.solids.len();
        assert!(expected > 0);

        for round in 1..=5 {
            {
                let mut lab = app.world_mut().resource_mut::<Lab>();
                if round == 1 {
                    lab.adopt(facility.clone());
                } else {
                    lab.reset();
                }
            }
            app.update();
            assert_eq!(
                count::<ImportedGeometry>(&mut app),
                expected,
                "structural geometry neither missing nor leaked on round {round}"
            );
            assert_eq!(count::<PlayerCam>(&mut app), 1);
            assert_eq!(count::<LabUiRoot>(&mut app), 1);
        }
    }
}
