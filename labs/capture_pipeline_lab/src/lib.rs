//! capture_pipeline_lab -- Phase A5 of the Bevy asset-integration roadmap.
//!
//! The lab proves `bevy_image_export 0.16` as a lab-local offscreen evidence
//! capture path. It exports one deterministic still frame and one short sequence
//! from a presentation snapshot without mutating the fixed-step simulation clock.

mod model;

pub use model::{
    CapturePass, CaptureSnapshot, CaptureTimeline, DEFAULT_CAPTURE_BASE_DIR, LOOP_TICKS,
    SEQUENCE_TICKS, STILL_TICK, output_path_is_lab_scoped, snapshot_at,
};

use std::path::{Path, PathBuf};

use bevy::{
    app::AppExit,
    camera::RenderTarget,
    ecs::system::SystemParam,
    input::InputSystems,
    pbr::{DistanceFog, FogFalloff},
    post_process::bloom::Bloom,
    prelude::*,
    render::{
        RenderPlugin,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        view::Hdr,
    },
    window::{PresentMode, WindowResolution},
};
use bevy_image_export::{ImageExport, ImageExportPlugin, ImageExportSettings, ImageExportSource};
use observed_style::{MarkerRole, SurfaceRole, Treatment, marker, surface};

const EXPORT_WIDTH: u32 = 960;
const EXPORT_HEIGHT: u32 = 540;
const CAPTURE_GRACE_FRAMES: u32 = 12;
const STILL_READY_FRAME: u32 = 1;
const SEQUENCE_READY_START: u32 = 4;
const CAPTURE_EXIT_PAD_FRAMES: u32 = 4;

#[derive(Component)]
pub struct CapturePipelineCam;

#[derive(Component)]
pub struct CapturePipelineExportCam;

#[derive(Component)]
pub struct CapturePipelineUiRoot;

#[derive(Component)]
pub struct LabSpawned;

#[derive(Component)]
struct DoorPanel {
    closed_position: Vec3,
    open_direction: f32,
}

#[derive(Component)]
struct RouteSpine;

#[derive(Component)]
struct RerouteFlash;

#[derive(Component)]
struct RunnerMarker {
    start: Vec3,
    end: Vec3,
}

#[derive(Component)]
struct DebugPanel;

#[derive(Component)]
struct HelpPanel;

#[derive(Component)]
struct DebugText;

type DoorPanelFilter = (
    Without<RouteSpine>,
    Without<RerouteFlash>,
    Without<RunnerMarker>,
);
type RouteSpineFilter = (
    With<RouteSpine>,
    Without<DoorPanel>,
    Without<RerouteFlash>,
    Without<RunnerMarker>,
);
type FlashFilter = (
    With<RerouteFlash>,
    Without<DoorPanel>,
    Without<RouteSpine>,
    Without<RunnerMarker>,
);
type RunnerFilter = (
    Without<DoorPanel>,
    Without<RouteSpine>,
    Without<RerouteFlash>,
);

#[derive(Resource)]
pub struct LabState {
    pub debug_visible: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub capture_snapshot: Option<CaptureSnapshot>,
    pub last_capture_status: String,
}

impl Default for LabState {
    fn default() -> Self {
        Self {
            debug_visible: true,
            reset_requested: false,
            reset_count: 0,
            capture_snapshot: None,
            last_capture_status: "interactive; no exporter active".to_string(),
        }
    }
}

#[derive(Resource, Default)]
pub struct SimulationClock {
    pub timeline: CaptureTimeline,
}

#[derive(Resource, Clone)]
struct CaptureTexture {
    handle: Handle<Image>,
}

#[derive(Resource)]
struct RenderFrameGate {
    raw_frame: u32,
    ready_frame: u32,
    grace_frames: u32,
}

impl RenderFrameGate {
    fn new(grace_frames: u32) -> Self {
        Self {
            raw_frame: 0,
            ready_frame: 0,
            grace_frames,
        }
    }

    fn tick(&mut self) {
        self.raw_frame = self.raw_frame.wrapping_add(1);
        if self.raw_frame > self.grace_frames {
            self.ready_frame = self.raw_frame - self.grace_frames;
        }
    }
}

#[derive(Resource)]
struct CaptureController {
    base_dir: PathBuf,
    active_exporter: Option<Entity>,
    still_done: bool,
    sequence_done: bool,
}

impl CaptureController {
    fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            active_exporter: None,
            still_done: false,
            sequence_done: false,
        }
    }
}

pub struct CapturePipelineLabPlugin;

impl Plugin for CapturePipelineLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LabState>()
            .init_resource::<SimulationClock>()
            .add_systems(Startup, setup_scene)
            .add_systems(FixedUpdate, advance_simulation)
            .add_systems(
                Update,
                (
                    handle_input.after(InputSystems),
                    perform_reset,
                    drive_capture_pipeline,
                    present_scene,
                    update_debug_text,
                )
                    .chain(),
            );
    }
}

fn material_for(treatment: Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        perceptual_roughness: 0.72,
        metallic: 0.04,
        ..default()
    }
}

fn transparent_material(treatment: Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment.base_color.with_alpha(0.0),
        emissive: treatment.emissive,
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.45,
        ..default()
    }
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    treatment: Treatment,
    position: Vec3,
    scale: Vec3,
    name: &'static str,
) -> Entity {
    commands
        .spawn((
            LabSpawned,
            Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
            MeshMaterial3d(materials.add(material_for(treatment))),
            Transform::from_translation(position).with_scale(scale),
            Name::new(name),
        ))
        .id()
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let output_texture = create_export_texture(&mut images);
    commands.insert_resource(CaptureTexture {
        handle: output_texture.clone(),
    });
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.24, 0.34, 0.52),
        brightness: 75.0,
        ..default()
    });

    let camera_transform =
        Transform::from_xyz(8.5, 6.3, 14.0).looking_at(Vec3::new(0.0, 1.2, -1.0), Vec3::Y);

    commands.spawn((
        LabSpawned,
        CapturePipelineCam,
        Camera3d::default(),
        Hdr,
        Bloom {
            intensity: 0.08,
            ..Bloom::NATURAL
        },
        DistanceFog {
            color: surface(SurfaceRole::Ceiling).base_color,
            falloff: FogFalloff::Linear {
                start: 8.0,
                end: 36.0,
            },
            ..default()
        },
        camera_transform,
        Name::new("Capture Pipeline Camera"),
    ));

    commands.spawn((
        LabSpawned,
        CapturePipelineExportCam,
        Camera3d::default(),
        RenderTarget::Image(output_texture.into()),
        Camera {
            clear_color: ClearColorConfig::Custom(surface(SurfaceRole::Ceiling).base_color),
            ..default()
        },
        camera_transform,
        Name::new("Capture Pipeline Export Camera"),
    ));

    commands.spawn((
        LabSpawned,
        DirectionalLight {
            illuminance: 1_100.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.65, -0.5, 0.0)),
        Name::new("Capture key light"),
    ));

    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        surface(SurfaceRole::Plain),
        Vec3::new(0.0, -0.08, -2.5),
        Vec3::new(14.0, 0.16, 24.0),
        "capture floor",
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        surface(SurfaceRole::Wall),
        Vec3::new(-6.2, 1.8, -2.5),
        Vec3::new(0.3, 3.6, 24.0),
        "left capture wall",
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        surface(SurfaceRole::Wall),
        Vec3::new(6.2, 1.8, -2.5),
        Vec3::new(0.3, 3.6, 24.0),
        "right capture wall",
    );
    let route_spine = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        surface(SurfaceRole::Spine),
        Vec3::new(0.0, 0.03, -2.5),
        Vec3::new(1.5, 0.08, 20.0),
        "route spine",
    );
    commands.entity(route_spine).insert(RouteSpine);

    let left_door = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        marker(MarkerRole::Collapse),
        Vec3::new(-1.35, 1.4, 3.2),
        Vec3::new(2.7, 2.8, 0.28),
        "left door panel",
    );
    commands.entity(left_door).insert(DoorPanel {
        closed_position: Vec3::new(-1.35, 1.4, 3.2),
        open_direction: -1.0,
    });
    let right_door = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        marker(MarkerRole::Collapse),
        Vec3::new(1.35, 1.4, 3.2),
        Vec3::new(2.7, 2.8, 0.28),
        "right door panel",
    );
    commands.entity(right_door).insert(DoorPanel {
        closed_position: Vec3::new(1.35, 1.4, 3.2),
        open_direction: 1.0,
    });

    let flash_material = materials.add(transparent_material(marker(MarkerRole::NextRoom)));
    commands.spawn((
        LabSpawned,
        RerouteFlash,
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(flash_material),
        Transform::from_translation(Vec3::new(0.0, 1.55, -3.8))
            .with_scale(Vec3::new(5.5, 0.1, 11.0)),
        Visibility::Hidden,
        Name::new("reroute flash plane"),
    ));

    commands.spawn((
        LabSpawned,
        RunnerMarker {
            start: Vec3::new(-3.9, 0.55, 8.2),
            end: Vec3::new(3.9, 0.55, -8.8),
        },
        Mesh3d(meshes.add(Sphere { radius: 0.42 })),
        MeshMaterial3d(materials.add(material_for(marker(MarkerRole::You)))),
        Transform::from_translation(Vec3::new(-3.9, 0.55, 8.2)),
        Name::new("fixed-tick runner marker"),
    ));

    for (position, role, name) in [
        (
            Vec3::new(-4.5, 0.65, 0.2),
            MarkerRole::Control,
            "capture console",
        ),
        (
            Vec3::new(4.4, 0.75, -6.5),
            MarkerRole::Exit,
            "capture objective",
        ),
        (
            Vec3::new(3.8, 0.55, 4.8),
            MarkerRole::Teammate,
            "capture pickup",
        ),
    ] {
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            marker(role),
            position,
            Vec3::splat(0.75),
            name,
        );
    }

    spawn_ui(&mut commands);
}

fn create_export_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let size = Extent3d {
        width: EXPORT_WIDTH,
        height: EXPORT_HEIGHT,
        ..default()
    };
    let mut export_texture = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("capture_pipeline_export_texture"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::COPY_DST
                | TextureUsages::COPY_SRC
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    export_texture.resize(size);
    images.add(export_texture)
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            LabSpawned,
            CapturePipelineUiRoot,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
            Name::new("Capture Pipeline UI Root"),
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
                BackgroundColor(surface(SurfaceRole::Wall).base_color.with_alpha(0.94)),
                BorderColor::all(marker(MarkerRole::NextRoom).base_color.with_alpha(0.72)),
                children![(
                    DebugText,
                    Text::new("Capture pipeline diagnostics starting..."),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.95, 1.0)),
                )],
            ));
            root.spawn((
                HelpPanel,
                Node {
                    width: px(430),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(surface(SurfaceRole::Wall).base_color.with_alpha(0.94)),
                BorderColor::all(marker(MarkerRole::Control).base_color.with_alpha(0.72)),
                children![(
                    Text::new(help_text()),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.95, 1.0)),
                )],
            ));
        });
}

fn help_text() -> String {
    format!(
        "CAPTURE PIPELINE LAB (Phase A5)\n\
         R reset  Space restart transition  F1 overlay\n\n\
         Env capture writes:\n\
         {base}/still/00001.png\n\
         {base}/sequence/00001.png..00006.png\n\n\
         Capture snapshots are presentation-only; the fixed timeline keeps ticking.",
        base = DEFAULT_CAPTURE_BASE_DIR
    )
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<LabState>,
    mut sim: ResMut<SimulationClock>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        state.reset_requested = true;
    }
    if keys.just_pressed(KeyCode::Space) {
        sim.timeline.reset();
    }
    if keys.just_pressed(KeyCode::F1) {
        state.debug_visible = !state.debug_visible;
    }
}

fn perform_reset(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    mut sim: ResMut<SimulationClock>,
    spawned: Query<Entity, With<LabSpawned>>,
) {
    if !state.reset_requested {
        return;
    }
    state.reset_requested = false;
    state.reset_count += 1;
    state.capture_snapshot = None;
    state.last_capture_status = "interactive reset".to_string();
    sim.timeline.reset();
    for entity in &spawned {
        commands.entity(entity).despawn();
    }
    commands.run_system_cached(setup_scene);
}

fn advance_simulation(mut sim: ResMut<SimulationClock>) {
    sim.timeline.advance_fixed_tick();
}

fn drive_capture_pipeline(
    mut commands: Commands,
    gate: Option<Res<RenderFrameGate>>,
    texture: Option<Res<CaptureTexture>>,
    mut export_sources: Option<ResMut<Assets<ImageExportSource>>>,
    mut controller: Option<ResMut<CaptureController>>,
    mut state: ResMut<LabState>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(controller) = controller.as_mut() else {
        state.capture_snapshot = None;
        return;
    };
    let Some(gate) = gate else {
        state.last_capture_status = "capture requested but frame gate is missing".to_string();
        return;
    };
    let ready_frame = gate.ready_frame;
    if ready_frame == 0 {
        state.last_capture_status = format!(
            "warming render pipeline ({}/{})",
            gate.raw_frame, gate.grace_frames
        );
        return;
    }

    let Some(texture) = texture else {
        state.last_capture_status = "capture requested but export texture is missing".to_string();
        return;
    };
    let Some(export_sources) = export_sources.as_mut() else {
        state.last_capture_status =
            "capture requested but ImageExportPlugin is missing".to_string();
        return;
    };

    let sequence_end = SEQUENCE_READY_START + CapturePass::Sequence.frame_count() as u32;

    if ready_frame == STILL_READY_FRAME {
        ensure_exporter(
            &mut commands,
            export_sources,
            &texture,
            controller,
            CapturePass::Still,
        );
        state.capture_snapshot = CapturePass::Still.snapshot_for_frame(0);
        state.last_capture_status = format!(
            "exporting still -> {}",
            output_dir_string(&controller.base_dir, CapturePass::Still)
        );
    } else if ready_frame == STILL_READY_FRAME + 1 {
        despawn_exporter(&mut commands, controller);
        controller.still_done = true;
        state.capture_snapshot = None;
        state.last_capture_status = "still capture complete".to_string();
    } else if (SEQUENCE_READY_START..sequence_end).contains(&ready_frame) {
        ensure_exporter(
            &mut commands,
            export_sources,
            &texture,
            controller,
            CapturePass::Sequence,
        );
        let frame_index = (ready_frame - SEQUENCE_READY_START) as usize;
        state.capture_snapshot = CapturePass::Sequence.snapshot_for_frame(frame_index);
        state.last_capture_status = format!(
            "exporting sequence frame {}/{} -> {}",
            frame_index + 1,
            CapturePass::Sequence.frame_count(),
            output_dir_string(&controller.base_dir, CapturePass::Sequence)
        );
    } else if ready_frame == sequence_end {
        despawn_exporter(&mut commands, controller);
        controller.sequence_done = true;
        state.capture_snapshot = None;
        state.last_capture_status = "sequence capture complete; waiting for writer".to_string();
    } else if ready_frame >= sequence_end + CAPTURE_EXIT_PAD_FRAMES {
        exit.write(AppExit::Success);
    }
}

fn ensure_exporter(
    commands: &mut Commands,
    export_sources: &mut Assets<ImageExportSource>,
    texture: &CaptureTexture,
    controller: &mut CaptureController,
    pass: CapturePass,
) {
    if controller.active_exporter.is_some() {
        return;
    }
    let output_dir = output_dir_string(&controller.base_dir, pass);
    let entity = commands
        .spawn((
            LabSpawned,
            ImageExport(export_sources.add(ImageExportSource(texture.handle.clone()))),
            ImageExportSettings {
                output_dir,
                extension: "png".to_string(),
            },
            Name::new(format!("{} image exporter", pass.label())),
        ))
        .id();
    controller.active_exporter = Some(entity);
}

fn despawn_exporter(commands: &mut Commands, controller: &mut CaptureController) {
    if let Some(entity) = controller.active_exporter.take() {
        commands.entity(entity).despawn();
    }
}

fn output_dir_string(base_dir: &Path, pass: CapturePass) -> String {
    pass.output_dir(base_dir)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(SystemParam)]
struct PresentationContext<'w, 's> {
    door_panels: Query<'w, 's, (&'static DoorPanel, &'static mut Transform), DoorPanelFilter>,
    route_spines: Query<'w, 's, &'static mut Transform, RouteSpineFilter>,
    flash: Query<
        'w,
        's,
        (
            &'static MeshMaterial3d<StandardMaterial>,
            &'static mut Transform,
            &'static mut Visibility,
        ),
        FlashFilter,
    >,
    runners: Query<'w, 's, (&'static RunnerMarker, &'static mut Transform), RunnerFilter>,
}

fn present_scene(
    state: Res<LabState>,
    sim: Res<SimulationClock>,
    mut context: PresentationContext,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let snapshot = state
        .capture_snapshot
        .unwrap_or_else(|| sim.timeline.snapshot());

    for (panel, mut transform) in &mut context.door_panels {
        transform.translation = panel.closed_position
            + Vec3::X * panel.open_direction * snapshot.door_open_fraction * 1.75;
    }

    for mut transform in &mut context.route_spines {
        transform.scale.x = 1.5 + snapshot.route_shift_fraction * 1.15;
    }

    for (material, mut transform, mut visibility) in &mut context.flash {
        transform.scale.y = 0.1 + snapshot.reroute_flash_alpha * 0.36;
        *visibility = if snapshot.reroute_flash_alpha > 0.01 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Some(material) = materials.get_mut(&material.0) {
            material.base_color = marker(MarkerRole::NextRoom)
                .base_color
                .with_alpha(snapshot.reroute_flash_alpha * 0.55);
        }
    }

    for (runner, mut transform) in &mut context.runners {
        transform.translation = runner.start.lerp(runner.end, snapshot.route_shift_fraction);
    }
}

#[derive(SystemParam)]
struct DebugContext<'w, 's> {
    state: Res<'w, LabState>,
    sim: Res<'w, SimulationClock>,
    controller: Option<Res<'w, CaptureController>>,
    cameras: Query<'w, 's, (), With<CapturePipelineCam>>,
    export_cameras: Query<'w, 's, (), With<CapturePipelineExportCam>>,
    ui_roots: Query<'w, 's, (), With<CapturePipelineUiRoot>>,
    door_panels: Query<'w, 's, (), With<DoorPanel>>,
    debug: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpPanel>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpPanel>, Without<DebugPanel>)>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
}

fn update_debug_text(mut context: DebugContext) {
    let visibility = if context.state.debug_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    **context.debug = visibility;
    **context.help = visibility;

    let camera_count = context.cameras.iter().count();
    let export_camera_count = context.export_cameras.iter().count();
    let ui_count = context.ui_roots.iter().count();
    let door_count = context.door_panels.iter().count();
    let snapshot = context
        .state
        .capture_snapshot
        .unwrap_or_else(|| context.sim.timeline.snapshot());
    let capture_active = context
        .controller
        .as_ref()
        .is_some_and(|capture| capture.active_exporter.is_some());
    let capture_done = context
        .controller
        .as_ref()
        .is_some_and(|capture| capture.still_done && capture.sequence_done);
    let healthy = camera_count == 1 && export_camera_count == 1 && ui_count == 1 && door_count == 2;

    **context.text = Text::new(format!(
        "CAPTURE PIPELINE  {}\n\
         exporter          bevy_image_export 0.16.0\n\
         fixed sim tick    {}\n\
         presented tick    {} ({})\n\
         door open         {:.2}\n\
         reroute flash     {:.2}\n\
         capture active    {}\n\
         capture complete  {}\n\
         status            {}\n\
         cameras {} + export {}  UI {}  doors {}  resets {}\n\n\
         Capture frames sample a presentation snapshot; they do not advance or\n\
         pause the fixed-step timeline.",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.sim.timeline.tick(),
        snapshot.tick,
        snapshot.phase_label,
        snapshot.door_open_fraction,
        snapshot.reroute_flash_alpha,
        capture_active,
        capture_done,
        context.state.last_capture_status,
        camera_count,
        export_camera_count,
        ui_count,
        door_count,
        context.state.reset_count,
    ));
}

fn tick_render_frame_gate(mut gate: ResMut<RenderFrameGate>) {
    gate.tick();
}

pub fn run() {
    let export_plugin = ImageExportPlugin::default();
    let export_threads = export_plugin.threads.clone();

    let mut app = App::new();
    app.insert_resource(ClearColor(surface(SurfaceRole::Ceiling).base_color))
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Observed 2 - Capture Pipeline Lab (Phase A5)".to_string(),
                        resolution: WindowResolution::new(1280, 720),
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(RenderPlugin {
                    synchronous_pipeline_compilation: true,
                    ..default()
                }),
            export_plugin,
            CapturePipelineLabPlugin,
        ));

    if let Ok(base_dir) = std::env::var("OBSERVED2_CAPTURE_PIPELINE") {
        app.insert_resource(CaptureController::new(base_dir))
            .insert_resource(RenderFrameGate::new(CAPTURE_GRACE_FRAMES))
            .add_systems(PreUpdate, tick_render_frame_gate);
    }

    app.run();
    export_threads.finish();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, input::InputPlugin};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), InputPlugin))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins(CapturePipelineLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_window_camera_export_camera_ui_and_door_panels() {
        let mut app = test_app();
        assert_eq!(count::<CapturePipelineCam>(&mut app), 1);
        assert_eq!(count::<CapturePipelineExportCam>(&mut app), 1);
        assert_eq!(count::<CapturePipelineUiRoot>(&mut app), 1);
        assert_eq!(count::<DoorPanel>(&mut app), 2);
    }

    #[test]
    fn reset_rebuilds_the_projection_without_leaking_entities() {
        let mut app = test_app();
        let baseline = count::<LabSpawned>(&mut app);
        assert!(baseline > 0);

        for expected_reset in 1..=3 {
            app.world_mut().resource_mut::<LabState>().reset_requested = true;
            app.update();
            assert_eq!(count::<LabSpawned>(&mut app), baseline);
            assert_eq!(
                app.world().resource::<LabState>().reset_count,
                expected_reset
            );
            assert_eq!(app.world().resource::<SimulationClock>().timeline.tick(), 0);
        }
    }

    #[test]
    fn capture_snapshot_does_not_pause_fixed_simulation() {
        let mut app = test_app();
        app.world_mut().resource_mut::<LabState>().capture_snapshot = Some(snapshot_at(STILL_TICK));
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(app.world().resource::<SimulationClock>().timeline.tick(), 1);
        assert_eq!(
            app.world()
                .resource::<LabState>()
                .capture_snapshot
                .unwrap()
                .tick,
            STILL_TICK
        );
    }
}
