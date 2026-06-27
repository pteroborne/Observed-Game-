//! outline_legibility_lab -- Phase A3 of the Bevy asset-integration roadmap.
//!
//! Proves the first half of the legibility overlay candidate with
//! `bevy_mod_outline 0.12`: critical objects receive semantic outline treatments
//! from `observed_style` and remain readable in a deliberately hostile neon-noir
//! scene. The researched `bevy_color_blindness` crate is not used as a dependency
//! because its only published version targets Bevy 0.8; this lab instead uses the
//! pure color-vision preview matrices in `observed_style` to test the same
//! contrast question without pulling an incompatible Bevy line.

use bevy::{
    app::AppExit,
    ecs::system::SystemParam,
    input::InputSystems,
    pbr::{DistanceFog, FogFalloff},
    post_process::bloom::Bloom,
    prelude::*,
    render::view::{
        Hdr,
        screenshot::{Screenshot, save_to_disk},
    },
    window::{PresentMode, WindowResolution},
};
use bevy_mod_outline::{OutlineMode, OutlinePlugin, OutlineStencil, OutlineVolume};
use observed_style::{
    ColorVisionMode, MarkerRole, OUTLINE_MIN_SIMULATED_LUMINANCE, OUTLINE_MIN_WIDTH, OutlineRole,
    SurfaceRole, Treatment, luminance, marker, outline, simulate_color_vision,
    simulate_linear_vision, surface,
};

#[derive(Component)]
pub struct OutlineLegibilityCam;

#[derive(Component)]
pub struct LabUiRoot;

#[derive(Component)]
pub struct LabSpawned;

#[derive(Component)]
struct DebugPanel;

#[derive(Component)]
struct HelpPanel;

#[derive(Component)]
struct DebugText;

#[derive(Component, Clone, Copy)]
struct SemanticMaterial {
    treatment: Treatment,
}

#[derive(Component, Clone, Copy)]
pub struct CriticalSignal {
    role: OutlineRole,
}

#[derive(Component, Clone, Copy)]
struct MotionCue {
    base: Vec3,
    axis: Vec3,
    amplitude: f32,
    speed: f32,
}

#[derive(Clone, Copy)]
struct BoxSpec {
    position: Vec3,
    size: Vec3,
    name: &'static str,
}

impl BoxSpec {
    fn new(position: Vec3, size: Vec3, name: &'static str) -> Self {
        Self {
            position,
            size,
            name,
        }
    }
}

#[derive(Resource)]
pub struct LabState {
    pub debug_visible: bool,
    pub preview_mode: ColorVisionMode,
    pub reset_count: u32,
    pub reset_requested: bool,
    pub preview_dirty: bool,
    pub camera_override: Option<Transform>,
}

impl Default for LabState {
    fn default() -> Self {
        Self {
            debug_visible: true,
            preview_mode: ColorVisionMode::Normal,
            reset_count: 0,
            reset_requested: false,
            preview_dirty: false,
            camera_override: None,
        }
    }
}

pub struct OutlineLegibilityLabPlugin;

impl Plugin for OutlineLegibilityLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LabState>()
            .add_systems(Startup, setup_scene)
            .add_systems(
                Update,
                (
                    handle_input.after(InputSystems),
                    perform_reset,
                    apply_preview_mode,
                    animate_motion,
                    present_camera,
                    update_debug_text,
                )
                    .chain(),
            );
    }
}

fn color_from_linear(c: bevy::color::LinearRgba) -> Color {
    Color::linear_rgba(c.red.max(0.0), c.green.max(0.0), c.blue.max(0.0), c.alpha)
}

fn preview_color(color: Color, mode: ColorVisionMode) -> Color {
    color_from_linear(simulate_color_vision(color, mode))
}

fn material_for(treatment: Treatment, mode: ColorVisionMode) -> StandardMaterial {
    StandardMaterial {
        base_color: preview_color(treatment.base_color, mode),
        emissive: simulate_linear_vision(treatment.emissive, mode),
        perceptual_roughness: 0.72,
        metallic: 0.05,
        ..default()
    }
}

fn fill_treatment(role: OutlineRole) -> Treatment {
    match role {
        OutlineRole::OpenDoor => marker(MarkerRole::Exit),
        OutlineRole::ClosedDoor => marker(MarkerRole::Collapse),
        OutlineRole::Interactable => marker(MarkerRole::Control),
        OutlineRole::Hazard => marker(MarkerRole::Collapse),
        OutlineRole::Rival => marker(MarkerRole::Rival),
        OutlineRole::ObjectiveBeacon => marker(MarkerRole::NextRoom),
        OutlineRole::Pickup => marker(MarkerRole::Teammate),
        OutlineRole::LocalPlayer => marker(MarkerRole::You),
    }
}

fn spawn_semantic_box(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    cube: &Handle<Mesh>,
    treatment: Treatment,
    mode: ColorVisionMode,
    spec: BoxSpec,
) -> Entity {
    let material = materials.add(material_for(treatment, mode));
    commands
        .spawn((
            LabSpawned,
            SemanticMaterial { treatment },
            Mesh3d(cube.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(spec.position).with_scale(spec.size),
            Name::new(spec.name),
        ))
        .id()
}

fn spawn_signal_box(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    cube: &Handle<Mesh>,
    role: OutlineRole,
    mode: ColorVisionMode,
    spec: BoxSpec,
) -> Entity {
    let treatment = fill_treatment(role);
    let outline_treatment = outline(role);
    let entity = spawn_semantic_box(commands, materials, cube, treatment, mode, spec);
    commands.entity(entity).insert((
        CriticalSignal { role },
        OutlineVolume {
            visible: true,
            width: outline_treatment.width,
            colour: preview_color(outline_treatment.color, mode),
        },
        OutlineStencil::default(),
        OutlineMode::ExtrudeFlat,
    ));
    entity
}

fn setup_scene(
    mut commands: Commands,
    state: Res<LabState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mode = state.preview_mode;
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.25, 0.35, 0.55),
        brightness: 85.0,
        ..default()
    });

    commands.spawn((
        LabSpawned,
        OutlineLegibilityCam,
        Camera3d::default(),
        Hdr,
        Bloom {
            intensity: 0.16,
            ..Bloom::NATURAL
        },
        DistanceFog {
            color: Color::srgb(0.008, 0.012, 0.026),
            falloff: FogFalloff::Linear {
                start: 6.0,
                end: 34.0,
            },
            ..default()
        },
        Transform::from_xyz(8.2, 7.0, 18.0).looking_at(Vec3::new(0.0, 1.6, -4.5), Vec3::Y),
        Name::new("Outline Legibility Camera"),
    ));

    commands.spawn((
        LabSpawned,
        DirectionalLight {
            illuminance: 950.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, -0.4, 0.0)),
        Name::new("Dim key light"),
    ));

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let floor = surface(SurfaceRole::Plain);
    let wall = surface(SurfaceRole::Wall);
    let ceiling = surface(SurfaceRole::Ceiling);
    let spine = surface(SurfaceRole::Spine);
    let trap = surface(SurfaceRole::TrapArmed);

    spawn_semantic_box(
        &mut commands,
        &mut materials,
        &cube,
        floor,
        mode,
        BoxSpec::new(
            Vec3::new(0.0, -0.08, -3.5),
            Vec3::new(16.0, 0.16, 44.0),
            "Foggy corridor floor",
        ),
    );
    spawn_semantic_box(
        &mut commands,
        &mut materials,
        &cube,
        wall,
        mode,
        BoxSpec::new(
            Vec3::new(-7.2, 2.0, -3.5),
            Vec3::new(0.35, 4.0, 44.0),
            "Left corridor wall",
        ),
    );
    spawn_semantic_box(
        &mut commands,
        &mut materials,
        &cube,
        wall,
        mode,
        BoxSpec::new(
            Vec3::new(7.2, 2.0, -3.5),
            Vec3::new(0.35, 4.0, 44.0),
            "Right corridor wall",
        ),
    );
    spawn_semantic_box(
        &mut commands,
        &mut materials,
        &cube,
        ceiling,
        mode,
        BoxSpec::new(
            Vec3::new(0.0, 4.15, -3.5),
            Vec3::new(16.0, 0.18, 44.0),
            "Dark ceiling",
        ),
    );
    spawn_semantic_box(
        &mut commands,
        &mut materials,
        &cube,
        spine,
        mode,
        BoxSpec::new(
            Vec3::new(0.0, 0.03, -7.0),
            Vec3::new(2.2, 0.08, 26.0),
            "Spine route stripe",
        ),
    );
    spawn_semantic_box(
        &mut commands,
        &mut materials,
        &cube,
        trap,
        mode,
        BoxSpec::new(
            Vec3::new(0.0, 3.0, -1.2),
            Vec3::new(7.0, 0.14, 0.18),
            "Bloom glare strip",
        ),
    );
    commands.spawn((
        LabSpawned,
        PointLight {
            color: marker(MarkerRole::Collapse).base_color,
            intensity: 48_000.0,
            range: 16.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 3.0, -1.2),
        Name::new("Bright hazard bloom light"),
    ));

    spawn_signal_box(
        &mut commands,
        &mut materials,
        &cube,
        OutlineRole::OpenDoor,
        mode,
        BoxSpec::new(
            Vec3::new(-2.8, 1.4, 9.5),
            Vec3::new(0.22, 2.8, 1.2),
            "Open door threshold left",
        ),
    );
    spawn_signal_box(
        &mut commands,
        &mut materials,
        &cube,
        OutlineRole::OpenDoor,
        mode,
        BoxSpec::new(
            Vec3::new(2.8, 1.4, 9.5),
            Vec3::new(0.22, 2.8, 1.2),
            "Open door threshold right",
        ),
    );
    spawn_signal_box(
        &mut commands,
        &mut materials,
        &cube,
        OutlineRole::ClosedDoor,
        mode,
        BoxSpec::new(
            Vec3::new(3.8, 1.45, 3.0),
            Vec3::new(2.4, 2.9, 0.28),
            "Closed side door",
        ),
    );
    spawn_signal_box(
        &mut commands,
        &mut materials,
        &cube,
        OutlineRole::Interactable,
        mode,
        BoxSpec::new(
            Vec3::new(-4.7, 0.8, 1.2),
            Vec3::new(1.2, 1.1, 1.0),
            "Route console",
        ),
    );
    let pickup = spawn_signal_box(
        &mut commands,
        &mut materials,
        &cube,
        OutlineRole::Pickup,
        mode,
        BoxSpec::new(
            Vec3::new(-3.45, 0.42, 0.7),
            Vec3::splat(0.72),
            "Carryable pickup",
        ),
    );
    commands.entity(pickup).insert(MotionCue {
        base: Vec3::new(-3.45, 0.42, 0.7),
        axis: Vec3::Y,
        amplitude: 0.16,
        speed: 2.8,
    });

    for x in [-2.0, 0.0, 2.0] {
        spawn_signal_box(
            &mut commands,
            &mut materials,
            &cube,
            OutlineRole::Hazard,
            mode,
            BoxSpec::new(
                Vec3::new(x, 1.2, -1.3),
                Vec3::new(0.22, 2.4, 0.26),
                "Pressure gate hazard bar",
            ),
        );
    }

    spawn_semantic_box(
        &mut commands,
        &mut materials,
        &cube,
        wall,
        mode,
        BoxSpec::new(
            Vec3::new(0.9, 1.75, -7.0),
            Vec3::new(1.2, 3.5, 1.2),
            "Foreground occluder pillar",
        ),
    );
    let rival = spawn_signal_box(
        &mut commands,
        &mut materials,
        &cube,
        OutlineRole::Rival,
        mode,
        BoxSpec::new(
            Vec3::new(1.4, 1.2, -8.4),
            Vec3::new(0.72, 2.4, 0.72),
            "Partly occluded rival",
        ),
    );
    commands.entity(rival).insert(MotionCue {
        base: Vec3::new(1.4, 1.2, -8.4),
        axis: Vec3::X,
        amplitude: 0.35,
        speed: 1.7,
    });

    spawn_signal_box(
        &mut commands,
        &mut materials,
        &cube,
        OutlineRole::ObjectiveBeacon,
        mode,
        BoxSpec::new(
            Vec3::new(0.0, 2.2, -18.0),
            Vec3::new(0.8, 4.4, 0.8),
            "Distant objective beacon",
        ),
    );
    spawn_signal_box(
        &mut commands,
        &mut materials,
        &cube,
        OutlineRole::LocalPlayer,
        mode,
        BoxSpec::new(
            Vec3::new(4.7, 1.05, 7.0),
            Vec3::new(0.8, 2.1, 0.8),
            "Local player proxy",
        ),
    );

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            LabSpawned,
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
            Name::new("Outline Legibility UI Root"),
        ))
        .with_children(|root| {
            root.spawn((
                DebugPanel,
                Node {
                    width: px(480),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.94)),
                BorderColor::all(marker(MarkerRole::NextRoom).base_color.with_alpha(0.65)),
                children![(
                    DebugText,
                    Text::new("Outline legibility diagnostics starting..."),
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
                BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.94)),
                BorderColor::all(marker(MarkerRole::Control).base_color.with_alpha(0.65)),
                children![(
                    Text::new(help_text()),
                    TextFont {
                        font_size: 13.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.95, 1.0)),
                )],
            ));
        });
}

fn help_text() -> String {
    let mut out = String::from(
        "OUTLINE LEGIBILITY LAB (Phase A3)\n\
         C preview mode  R reset  F1 overlay\n\n\
         Worst cases in one scene: foggy corridor, bloom glare,\n\
         overlapping console/pickup/hazard, partly occluded rival,\n\
         and a distant objective.\n\n\
         Outline legend:\n",
    );
    for (label, treatment) in observed_style::outline_legend() {
        out.push_str(&format!("  {:<18} width {:.1}\n", label, treatment.width));
    }
    out
}

fn handle_input(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<LabState>) {
    if keys.just_pressed(KeyCode::KeyC) {
        state.preview_mode = state.preview_mode.next();
        state.preview_dirty = true;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        state.reset_requested = true;
    }
    if keys.just_pressed(KeyCode::F1) {
        state.debug_visible = !state.debug_visible;
    }
}

fn perform_reset(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    spawned: Query<Entity, With<LabSpawned>>,
) {
    if !state.reset_requested {
        return;
    }
    state.reset_requested = false;
    state.reset_count += 1;
    state.preview_dirty = true;
    for entity in &spawned {
        commands.entity(entity).despawn();
    }
    commands.run_system_cached(setup_scene);
}

fn apply_preview_mode(
    mut state: ResMut<LabState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    semantic: Query<(&SemanticMaterial, &MeshMaterial3d<StandardMaterial>)>,
    mut outlines: Query<(&CriticalSignal, &mut OutlineVolume)>,
) {
    if !state.preview_dirty {
        return;
    }
    state.preview_dirty = false;
    for (semantic, material) in &semantic {
        if let Some(material) = materials.get_mut(&material.0) {
            *material = material_for(semantic.treatment, state.preview_mode);
        }
    }
    for (signal, mut volume) in &mut outlines {
        let treatment = outline(signal.role);
        volume.width = treatment.width;
        volume.colour = preview_color(treatment.color, state.preview_mode);
        volume.visible = true;
    }
}

fn animate_motion(time: Res<Time>, mut moving: Query<(&MotionCue, &mut Transform)>) {
    for (motion, mut transform) in &mut moving {
        let offset = (time.elapsed_secs() * motion.speed).sin() * motion.amplitude;
        transform.translation = motion.base + motion.axis * offset;
    }
}

fn present_camera(
    state: Res<LabState>,
    mut camera: Single<&mut Transform, With<OutlineLegibilityCam>>,
) {
    if let Some(pose) = state.camera_override {
        **camera = pose;
    }
}

fn min_outline_luminance() -> f32 {
    OutlineRole::ALL
        .iter()
        .flat_map(|role| {
            ColorVisionMode::ALL
                .iter()
                .map(move |mode| luminance(simulate_color_vision(outline(*role).color, *mode)))
        })
        .fold(f32::INFINITY, f32::min)
}

#[derive(SystemParam)]
struct DebugContext<'w, 's> {
    state: Res<'w, LabState>,
    cameras: Query<'w, 's, (), With<OutlineLegibilityCam>>,
    ui_roots: Query<'w, 's, (), With<LabUiRoot>>,
    signals: Query<'w, 's, (), With<CriticalSignal>>,
    outlines: Query<'w, 's, &'static OutlineVolume>,
    text: Single<'w, 's, &'static mut Text, With<DebugText>>,
    debug: Single<'w, 's, &'static mut Visibility, (With<DebugPanel>, Without<HelpPanel>)>,
    help: Single<'w, 's, &'static mut Visibility, (With<HelpPanel>, Without<DebugPanel>)>,
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
    let ui_count = context.ui_roots.iter().count();
    let signal_count = context.signals.iter().count();
    let outline_count = context
        .outlines
        .iter()
        .filter(|outline| outline.visible && outline.width >= OUTLINE_MIN_WIDTH)
        .count();
    let min_lum = min_outline_luminance();
    let healthy = camera_count == 1
        && ui_count == 1
        && signal_count >= OutlineRole::ALL.len()
        && outline_count >= OutlineRole::ALL.len()
        && min_lum >= OUTLINE_MIN_SIMULATED_LUMINANCE;

    *context.text.into_inner() = Text::new(format!(
        "OUTLINE LEGIBILITY  {}\n\
         outline plugin     bevy_mod_outline 0.12.1\n\
         preview mode       {}\n\
         critical signals   {}\n\
         active outlines    {}\n\
         min preview luma   {:.3} / {:.3}\n\
         min width          {:.1}px\n\
         cameras {}  UI {}  resets {}\n\n\
         bevy_color_blindness: not linked; published 0.2.0 targets Bevy 0.8.\n\
         This lab uses the shared style preview matrices instead.",
        if healthy { "[PASS]" } else { "[FAIL]" },
        context.state.preview_mode.label(),
        signal_count,
        outline_count,
        min_lum,
        OUTLINE_MIN_SIMULATED_LUMINANCE,
        OUTLINE_MIN_WIDTH,
        camera_count,
        ui_count,
        context.state.reset_count,
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.006, 0.009, 0.018)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Outline Legibility Lab (Phase A3)".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(OutlinePlugin)
        .add_plugins(OutlineLegibilityLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut state: ResMut<LabState>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        state.preview_mode = ColorVisionMode::Deuteranopia;
        state.preview_dirty = true;
        state.camera_override = Some(
            Transform::from_xyz(8.2, 7.0, 18.0).looking_at(Vec3::new(0.0, 1.6, -4.5), Vec3::Y),
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
    use bevy::{asset::AssetPlugin, input::InputPlugin};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), InputPlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins(OutlineLegibilityLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_ui_and_every_signal_outlined() {
        let mut app = test_app();
        assert_eq!(count::<OutlineLegibilityCam>(&mut app), 1);
        assert_eq!(count::<LabUiRoot>(&mut app), 1);
        assert!(count::<CriticalSignal>(&mut app) >= OutlineRole::ALL.len());
        assert!(count::<OutlineVolume>(&mut app) >= OutlineRole::ALL.len());
    }

    #[test]
    fn preview_mode_cycles_without_touching_simulation_state() {
        let mut app = test_app();
        assert_eq!(
            app.world().resource::<LabState>().preview_mode,
            ColorVisionMode::Normal
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyC);
        app.world_mut().run_schedule(Update);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyC);
        assert_eq!(
            app.world().resource::<LabState>().preview_mode,
            ColorVisionMode::Protanopia
        );
        assert_eq!(app.world().resource::<LabState>().reset_count, 0);
    }

    #[test]
    fn reset_rebuilds_the_scene_without_leaking() {
        let mut app = test_app();
        let baseline = count::<LabSpawned>(&mut app);
        assert!(baseline > 0);
        for expected in 1..=4 {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyR);
            app.world_mut().run_schedule(Update);
            {
                let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                keys.release(KeyCode::KeyR);
                keys.clear();
            }
            assert_eq!(count::<LabSpawned>(&mut app), baseline);
            assert_eq!(count::<OutlineLegibilityCam>(&mut app), 1);
            assert_eq!(count::<LabUiRoot>(&mut app), 1);
            assert_eq!(app.world().resource::<LabState>().reset_count, expected);
        }
    }

    #[test]
    fn semantic_outlines_pass_preview_contrast_floor() {
        assert!(min_outline_luminance() >= OUTLINE_MIN_SIMULATED_LUMINANCE);
    }
}
