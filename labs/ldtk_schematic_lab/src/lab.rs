//! Bevy presentation for the LDtk schematic projection.
//!
//! The rendering is deliberately simple: sprites and gizmos projected from the
//! pure [`crate::project::Schematic`]. LDtk entities/layers are never spawned as
//! gameplay entities.

use std::path::Path;

use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};
use bevy_ecs_ldtk::prelude::{LdtkPlugin, LdtkProject};
use observed_style::{MarkerRole, SurfaceRole, marker, surface};

use crate::{
    ldtk_source,
    project::{self, Schematic, SchematicRoomKind, SchematicSymbol},
};

const ASSET_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
const LDTK_PATH: &str = "schematic.ldtk";
const DISPLAY_SCALE: f32 = 2.0;

#[derive(Component)]
pub struct SchematicCam;

#[derive(Component)]
pub struct LabUiRoot;

#[derive(Component)]
struct DebugText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct DebugPanel;

#[derive(Component)]
pub struct SchematicVisual;

#[derive(Resource)]
struct LdtkHandle(Handle<LdtkProject>);

#[derive(Resource)]
pub struct LabState {
    pub schematic: Option<Schematic>,
    pub debug_visible: bool,
    pub reset_count: u32,
    pub last_event: String,
    dirty: bool,
}

impl Default for LabState {
    fn default() -> Self {
        Self {
            schematic: None,
            debug_visible: true,
            reset_count: 0,
            last_event: "Loading LDtk schematic...".to_string(),
            dirty: false,
        }
    }
}

impl LabState {
    pub fn loaded(&self) -> bool {
        self.schematic.is_some()
    }

    pub fn adopt(&mut self, schematic: Schematic) {
        self.schematic = Some(schematic);
        self.dirty = true;
        self.last_event = "LDtk project imported and projected into schematic data.".to_string();
    }

    pub fn reset_from_source(&mut self) {
        self.reset_count += 1;
        match project::project(&ldtk_source::parse_ldtk_json()) {
            Ok(schematic) => {
                self.adopt(schematic);
                self.last_event = format!(
                    "Reset #{} - reprojected from the authored LDtk source.",
                    self.reset_count
                );
            }
            Err(err) => {
                self.last_event = format!("Reset failed: {err}");
            }
        }
    }

    pub fn health(&self) -> bool {
        self.schematic
            .as_ref()
            .is_some_and(|s| s.rooms.len() == 3 && s.ports.len() == 2 && !s.cells.is_empty())
    }
}

pub struct LdtkSchematicLabPlugin;

impl Plugin for LdtkSchematicLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LabState>()
            .add_systems(Startup, (setup_camera, setup_ui))
            .add_systems(
                Update,
                (
                    await_load,
                    handle_input.after(InputSystems),
                    apply_visuals,
                    draw_debug,
                    update_debug_text,
                )
                    .chain(),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.022)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Observed 2 - LDtk Schematic Lab (Phase A2)".to_string(),
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
        // Use bevy_ecs_ldtk as the LDtk importer. The lab does not spawn its world
        // bundle or tile renderer; it only reads the loaded project data.
        .add_plugins(LdtkPlugin)
        .add_plugins(LdtkSchematicLabPlugin)
        .add_systems(Startup, (ensure_ldtk_file, load_ldtk_project).chain());

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

fn ensure_ldtk_file() {
    let path = Path::new(ASSET_DIR).join(LDTK_PATH);
    let wanted = ldtk_source::ldtk_json();
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

fn load_ldtk_project(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(LdtkHandle(asset_server.load::<LdtkProject>(LDTK_PATH)));
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        SchematicCam,
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 1.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Name::new("LDtk Schematic Camera"),
    ));
}

fn await_load(
    handle: Option<Res<LdtkHandle>>,
    projects: Option<Res<Assets<LdtkProject>>>,
    mut state: ResMut<LabState>,
) {
    if state.loaded() {
        return;
    }
    let (Some(handle), Some(projects)) = (handle, projects) else {
        return;
    };
    let Some(project) = projects.get(&handle.0) else {
        return;
    };
    match project::project(project.json_data()) {
        Ok(schematic) => state.adopt(schematic),
        Err(err) => state.last_event = format!("LDtk projection failed: {err}"),
    }
}

fn handle_input(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<LabState>) {
    if keys.just_pressed(KeyCode::KeyR) {
        state.reset_from_source();
    }
    if keys.just_pressed(KeyCode::F1) {
        state.debug_visible = !state.debug_visible;
    }
}

fn world_from_px(pos: Vec2, level_size: Vec2) -> Vec3 {
    Vec3::new(
        (pos.x - level_size.x * 0.5) * DISPLAY_SCALE,
        (level_size.y * 0.5 - pos.y) * DISPLAY_SCALE,
        0.0,
    )
}

fn rect_center_from_px(min: Vec2, size: Vec2, level_size: Vec2) -> Vec3 {
    world_from_px(min + size * 0.5, level_size)
}

fn symbol_color(symbol: SchematicSymbol) -> Color {
    match symbol {
        SchematicSymbol::Room => surface(SurfaceRole::Plain).base_color,
        SchematicSymbol::Corridor => surface(SurfaceRole::Spine).base_color,
        SchematicSymbol::DoorThreshold => marker(MarkerRole::Control).base_color,
        SchematicSymbol::Spawn => marker(MarkerRole::You).base_color,
        SchematicSymbol::Objective => marker(MarkerRole::Exit).base_color,
    }
}

fn room_color(kind: SchematicRoomKind) -> Color {
    match kind {
        SchematicRoomKind::Room => surface(SurfaceRole::SafeBypass).base_color.with_alpha(0.42),
        SchematicRoomKind::Corridor => surface(SurfaceRole::Spine).base_color.with_alpha(0.50),
    }
}

fn spawn_rect(
    commands: &mut Commands,
    color: Color,
    center: Vec3,
    size: Vec2,
    z: f32,
    name: &'static str,
) {
    commands.spawn((
        SchematicVisual,
        Sprite::from_color(color, size * DISPLAY_SCALE),
        Transform::from_translation(center + Vec3::Z * z),
        Name::new(name),
    ));
}

fn apply_visuals(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    visuals: Query<Entity, With<SchematicVisual>>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;
    for entity in &visuals {
        commands.entity(entity).despawn();
    }
    let Some(schematic) = state.schematic.clone() else {
        return;
    };

    spawn_rect(
        &mut commands,
        surface(SurfaceRole::Ceiling).base_color,
        Vec3::ZERO,
        schematic.level_px_size + Vec2::splat(48.0),
        -4.0,
        "Schematic Backdrop",
    );

    for room in &schematic.rooms {
        spawn_rect(
            &mut commands,
            room_color(room.kind),
            rect_center_from_px(room.px_min, room.px_size, schematic.level_px_size),
            room.px_size - Vec2::splat(6.0),
            -2.0,
            "LDtk Room Entity",
        );
    }

    let cell = Vec2::splat(schematic.cell_size as f32);
    for schematic_cell in &schematic.cells {
        let px = schematic_cell.grid.as_vec2() * schematic.cell_size as f32;
        spawn_rect(
            &mut commands,
            symbol_color(schematic_cell.symbol),
            rect_center_from_px(px, cell, schematic.level_px_size),
            cell - Vec2::splat(3.0),
            0.0,
            "LDtk IntGrid Symbol",
        );
    }

    for port in &schematic.ports {
        spawn_rect(
            &mut commands,
            marker(MarkerRole::Control).base_color,
            world_from_px(port.pos_px, schematic.level_px_size) + Vec3::Z * 2.0,
            Vec2::splat(schematic.cell_size as f32 * 0.42),
            2.0,
            "LDtk Port Entity",
        );
    }
}

fn draw_rect(gizmos: &mut Gizmos, center: Vec3, size: Vec2, color: Color) {
    let half = size * DISPLAY_SCALE * 0.5;
    let z = 10.0;
    let a = center + Vec3::new(-half.x, -half.y, z);
    let b = center + Vec3::new(half.x, -half.y, z);
    let c = center + Vec3::new(half.x, half.y, z);
    let d = center + Vec3::new(-half.x, half.y, z);
    gizmos.line(a, b, color);
    gizmos.line(b, c, color);
    gizmos.line(c, d, color);
    gizmos.line(d, a, color);
}

fn draw_debug(state: Res<LabState>, mut gizmos: Gizmos) {
    if !state.debug_visible {
        return;
    }
    let Some(schematic) = &state.schematic else {
        return;
    };

    for room in &schematic.rooms {
        let color = match room.kind {
            SchematicRoomKind::Room => marker(MarkerRole::Teammate).base_color,
            SchematicRoomKind::Corridor => marker(MarkerRole::NextRoom).base_color,
        };
        draw_rect(
            &mut gizmos,
            rect_center_from_px(room.px_min, room.px_size, schematic.level_px_size),
            room.px_size,
            color,
        );
    }

    let port_color = marker(MarkerRole::Control).base_color;
    for port in &schematic.ports {
        let p = world_from_px(port.pos_px, schematic.level_px_size);
        let arm = 18.0;
        gizmos.line(
            p + Vec3::new(-arm, 0.0, 10.0),
            p + Vec3::new(arm, 0.0, 10.0),
            port_color,
        );
        gizmos.line(
            p + Vec3::new(0.0, -arm, 10.0),
            p + Vec3::new(0.0, arm, 10.0),
            port_color,
        );
    }
}

fn setup_ui(mut commands: Commands) {
    commands
        .spawn((
            LabUiRoot,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
            Name::new("LDtk Schematic UI Root"),
        ))
        .with_children(|root| {
            root.spawn((
                DebugPanel,
                Node {
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.03, 0.94)),
                BorderColor::all(Color::srgba(0.4, 0.7, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("Loading LDtk schematic..."),
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
                        "LDTK SCHEMATIC LAB (Phase A2)\n\
                         R reset - F1 debug\n\n\
                         LDtk entities define RoomId / PortId graph metadata.\n\
                         LDtk IntGrid cells define tactical-map symbols.\n\
                         The rendered view is a projection of that model, not\n\
                         LDtk-spawned gameplay state.\n\n\
                         Cyan/blue = rooms, gold = corridor,\n\
                         violet = door ports, pale = spawn, green = objective.",
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
    state: Res<LabState>,
    cams: Query<(), With<SchematicCam>>,
    ui_roots: Query<(), With<LabUiRoot>>,
    visuals: Query<(), With<SchematicVisual>>,
    mut text: Query<&mut Text, With<DebugText>>,
    mut panel: Query<&mut Visibility, (With<DebugPanel>, Without<HelpText>)>,
    mut help: Query<&mut Visibility, (With<HelpText>, Without<DebugPanel>)>,
) {
    let visibility = if state.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if let Ok(mut panel) = panel.single_mut() {
        *panel = visibility;
    }
    if let Ok(mut help) = help.single_mut() {
        *help = visibility;
    }

    let (rooms, corridors, ports, symbols, doors) = state
        .schematic
        .as_ref()
        .map(|schematic| {
            (
                schematic
                    .rooms
                    .iter()
                    .filter(|room| room.kind == SchematicRoomKind::Room)
                    .count(),
                schematic
                    .rooms
                    .iter()
                    .filter(|room| room.kind == SchematicRoomKind::Corridor)
                    .count(),
                schematic.ports.len(),
                schematic.cells.len(),
                schematic.cells_with(SchematicSymbol::DoorThreshold),
            )
        })
        .unwrap_or((0, 0, 0, 0, 0));

    let healthy = state.health() && cams.iter().count() == 1 && ui_roots.iter().count() == 1;
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    *text = Text::new(format!(
        "LDTK SCHEMATIC IMPORT  {}\n\
         status          {}\n\
         rooms           {rooms} + {corridors} corridor\n\
         ports           {ports}\n\
         symbols         {symbols} ({doors} door thresholds)\n\
         spawned visuals {}\n\
         camera {}  UI {}\n\
         resets          {}\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if state.loaded() {
            "projected"
        } else {
            "loading..."
        },
        visuals.iter().count(),
        cams.iter().count(),
        ui_roots.iter().count(),
        state.reset_count,
        state.last_event,
    ));
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    state: Res<LabState>,
    mut request: ResMut<CaptureRequest>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if !state.loaded() {
        if time.elapsed_secs() > 20.0 {
            exit.write(AppExit::Success);
        }
        return;
    }
    let elapsed = time.elapsed_secs();
    if request.phase == 0 && elapsed >= 0.8 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 1.6 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};

    fn test_schematic() -> Schematic {
        project::project(&ldtk_source::parse_ldtk_json()).expect("test LDtk schematic projects")
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(LdtkSchematicLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_and_ui_without_projecting_ldtk_entities() {
        let mut app = test_app();
        assert_eq!(count::<SchematicCam>(&mut app), 1);
        assert_eq!(count::<LabUiRoot>(&mut app), 1);
        assert_eq!(count::<SchematicVisual>(&mut app), 0);
    }

    #[test]
    fn adopting_a_schematic_spawns_projected_visuals() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<LabState>()
            .adopt(test_schematic());
        app.update();
        assert!(count::<SchematicVisual>(&mut app) > 10);
        assert!(app.world().resource::<LabState>().health());
    }

    #[test]
    fn reset_reprojects_without_leaking_visuals() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<LabState>()
            .adopt(test_schematic());
        app.update();
        let baseline = count::<SchematicVisual>(&mut app);
        assert!(baseline > 0);

        for reset in 1..=5 {
            app.world_mut()
                .resource_mut::<LabState>()
                .reset_from_source();
            app.update();
            assert_eq!(
                count::<SchematicVisual>(&mut app),
                baseline,
                "reset {reset} should rebuild exactly the projected schematic visuals"
            );
            assert_eq!(count::<SchematicCam>(&mut app), 1);
            assert_eq!(count::<LabUiRoot>(&mut app), 1);
        }
    }
}
