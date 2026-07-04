use bevy::prelude::*;
use observed_style::{SurfaceRole, Treatment, surface};
use observed_traversal::gantry::{
    GANTRY_LENGTH, GANTRY_WIDTH, GantryCourse, GantryExit, GantryPlatform, GantryRoute,
    GantryRunResult, GantryRunState, SAFE_BYPASS_X, UNDERSTORY_SIDE_EXIT_Z, UPPER_DECK_Y,
};
use observed_traversal::{FIXED_DT, FpsConfig};

#[derive(Component)]
pub struct GantryCam;

#[derive(Component)]
pub struct GantryUiRoot;

#[derive(Component)]
pub struct GantryRunner;

#[derive(Component)]
pub(crate) struct GantrySpawned;

#[derive(Component)]
pub(crate) struct DebugText;

#[derive(Resource, Clone, Debug)]
pub struct GantryRuntime {
    pub course: GantryCourse,
    pub config: FpsConfig,
    pub run: GantryRunState,
    pub route: GantryRoute,
    pub result: Option<GantryRunResult>,
    pub trail: Vec<Vec3>,
    pub reset_count: u32,
}

impl Default for GantryRuntime {
    fn default() -> Self {
        let course = GantryCourse::authored();
        let config = FpsConfig::default();
        let route = GantryRoute::CleanJump;
        Self {
            run: GantryRunState::new(route, &course, &config),
            course,
            config,
            route,
            result: None,
            trail: Vec::new(),
            reset_count: 0,
        }
    }
}

impl GantryRuntime {
    pub fn reset(&mut self) {
        self.run = GantryRunState::new(self.route, &self.course, &self.config);
        self.result = None;
        self.trail.clear();
        self.reset_count += 1;
    }

    pub fn set_route(&mut self, route: GantryRoute) {
        if self.route != route {
            self.route = route;
            self.reset();
        } else if self.result.is_some() {
            self.reset();
        }
    }
}

pub fn setup_lab(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        GantryCam,
        GantrySpawned,
        Camera3d::default(),
        Transform::from_xyz(0.0, 13.5, -24.0).looking_at(Vec3::new(0.0, 1.2, 0.0), Vec3::Y),
        Name::new("Gantry overview camera"),
    ));
    commands.spawn((
        GantrySpawned,
        DirectionalLight {
            illuminance: 4_500.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.45, 0.0)),
        Name::new("Gantry key light"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.34, 0.42, 0.58),
        brightness: 55.0,
        ..default()
    });

    let runtime = GantryRuntime::default();
    spawn_course(&mut commands, &mut meshes, &mut materials, &runtime.course);
    commands.spawn((
        GantryRunner,
        GantrySpawned,
        Mesh3d(meshes.add(Capsule3d::new(0.34, 1.1))),
        MeshMaterial3d(materials.add(material(&observed_style::marker(
            observed_style::MarkerRole::You,
        )))),
        Transform::from_translation(runtime.run.body.position),
        Name::new("Gantry runner"),
    ));
    commands.insert_resource(runtime);
    spawn_ui(&mut commands);
}

fn material(treatment: &Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        perceptual_roughness: 0.82,
        ..default()
    }
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
    name: &'static str,
) {
    commands.spawn((
        GantrySpawned,
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(material),
        Transform::from_translation(center).with_scale(size),
        Name::new(name),
    ));
}

fn spawn_course(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    course: &GantryCourse,
) {
    let plain = materials.add(material(&surface(SurfaceRole::Plain)));
    let deck = materials.add(material(&surface(SurfaceRole::GantryDeck)));
    let edge = materials.add(material(&surface(SurfaceRole::GantryEdge)));
    let understory = materials.add(material(&surface(SurfaceRole::Understory)));
    let bypass = materials.add(material(&surface(SurfaceRole::SafeBypass)));
    let wall = materials.add(material(&surface(SurfaceRole::Wall)));

    spawn_box(
        commands,
        meshes,
        plain.clone(),
        Vec3::new(0.0, -0.03, 0.0),
        Vec3::new(GANTRY_WIDTH, 0.06, GANTRY_LENGTH),
        "Gantry lower floor",
    );
    spawn_box(
        commands,
        meshes,
        bypass,
        Vec3::new(SAFE_BYPASS_X, 0.03, 0.0),
        Vec3::new(2.4, 0.08, GANTRY_LENGTH - 1.2),
        "Safe bypass lane",
    );
    spawn_box(
        commands,
        meshes,
        understory,
        Vec3::new(2.45, 0.04, UNDERSTORY_SIDE_EXIT_Z * 0.5 - 2.0),
        Vec3::new(4.6, 0.09, UNDERSTORY_SIDE_EXIT_Z + 8.4),
        "Visible understory recovery floor",
    );
    for x in [-GANTRY_WIDTH * 0.5, GANTRY_WIDTH * 0.5] {
        spawn_box(
            commands,
            meshes,
            wall.clone(),
            Vec3::new(x, 1.9, 0.0),
            Vec3::new(0.28, 3.8, GANTRY_LENGTH),
            "Gantry side wall",
        );
    }
    for platform in &course.platforms {
        spawn_platform(commands, meshes, deck.clone(), edge.clone(), *platform);
    }
    spawn_platform(
        commands,
        meshes,
        deck.clone(),
        edge.clone(),
        course.upper_landing,
    );
    spawn_platform(
        commands,
        meshes,
        deck.clone(),
        edge.clone(),
        course.entry_landing,
    );
    spawn_threshold_markers(commands, meshes, materials, course);
}

fn spawn_platform(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    deck: Handle<StandardMaterial>,
    edge: Handle<StandardMaterial>,
    platform: GantryPlatform,
) {
    let height = (platform.top_y - platform.bottom_y).max(0.02);
    spawn_box(
        commands,
        meshes,
        deck,
        Vec3::new(
            platform.center.x,
            platform.bottom_y + height * 0.5,
            platform.center.y,
        ),
        Vec3::new(platform.half.x * 2.0, height, platform.half.y * 2.0),
        "Gantry platform",
    );
    let y = platform.top_y + 0.055;
    for x in [-platform.half.x, platform.half.x] {
        spawn_box(
            commands,
            meshes,
            edge.clone(),
            Vec3::new(platform.center.x + x, y, platform.center.y),
            Vec3::new(0.09, 0.11, platform.half.y * 2.0 + 0.12),
            "Gantry lit side edge",
        );
    }
    for z in [-platform.half.y, platform.half.y] {
        spawn_box(
            commands,
            meshes,
            edge.clone(),
            Vec3::new(platform.center.x, y, platform.center.y + z),
            Vec3::new(platform.half.x * 2.0 + 0.12, 0.11, 0.09),
            "Gantry lit jump edge",
        );
    }
}

fn spawn_threshold_markers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    course: &GantryCourse,
) {
    let marker = materials.add(material(&observed_style::marker(
        observed_style::MarkerRole::Exit,
    )));
    for exit in [
        GantryExit::UpperExit,
        GantryExit::UnderstorySideExit,
        GantryExit::SafeBypassExit,
        GantryExit::UnderstoryReturn,
    ] {
        let threshold = course.threshold(exit);
        spawn_box(
            commands,
            meshes,
            marker.clone(),
            Vec3::new(
                threshold.center.x,
                threshold.floor_y + 0.8,
                threshold.center.y,
            ),
            Vec3::new(threshold.width, 1.6, 0.12),
            "Gantry threshold marker",
        );
    }
}

fn spawn_ui(commands: &mut Commands) {
    commands.spawn((
        GantryUiRoot,
        GantrySpawned,
        Node {
            position_type: PositionType::Absolute,
            top: px(14),
            left: px(14),
            width: px(430),
            padding: UiRect::all(px(12)),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.92)),
        BorderColor::all(Color::srgba(0.25, 0.9, 1.0, 0.55)),
        children![(
            DebugText,
            Text::new("Gantry"),
            TextFont {
                font_size: 15.0,
                ..default()
            },
            TextColor(Color::srgb(0.86, 0.95, 1.0)),
        )],
    ));
}

pub fn handle_input(keys: Res<ButtonInput<KeyCode>>, mut runtime: ResMut<GantryRuntime>) {
    if keys.just_pressed(KeyCode::Digit1) {
        runtime.set_route(GantryRoute::CleanJump);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        runtime.set_route(GantryRoute::FallRecover);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        runtime.set_route(GantryRoute::SafeBypass);
    }
    if keys.just_pressed(KeyCode::KeyR) {
        runtime.reset();
    }
}

pub fn simulate(mut runtime: ResMut<GantryRuntime>) {
    if runtime.result.is_some() {
        return;
    }
    let course = runtime.course.clone();
    let config = runtime.config;
    let result = runtime.run.step(&course, &config);
    if runtime.run.ticks.is_multiple_of(3) {
        let p = runtime.run.body.position;
        runtime.trail.push(Vec3::new(p.x, 0.16, p.z));
    }
    runtime.result = result;
}

pub fn present_runner(
    runtime: Res<GantryRuntime>,
    mut runner: Query<&mut Transform, With<GantryRunner>>,
) {
    let Ok(mut transform) = runner.single_mut() else {
        return;
    };
    transform.translation = runtime.run.body.position;
}

pub fn present_camera(
    runtime: Res<GantryRuntime>,
    mut camera: Query<&mut Transform, With<GantryCam>>,
) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    let p = runtime.run.body.position;
    *transform = Transform::from_xyz(p.x * 0.18, 13.5, p.z - 22.0)
        .looking_at(Vec3::new(p.x, 1.3, p.z + 4.0), Vec3::Y);
}

pub fn draw_debug(runtime: Res<GantryRuntime>, mut gizmos: Gizmos) {
    if runtime.trail.len() > 1 {
        gizmos.linestrip(runtime.trail.iter().copied(), Color::srgb(0.95, 0.82, 0.25));
    }
    for threshold in &runtime.course.thresholds {
        let p = Vec3::new(
            threshold.center.x,
            threshold.floor_y + 0.05,
            threshold.center.y,
        );
        gizmos.line(p, p + Vec3::Y * 2.0, Color::srgb(0.2, 1.0, 0.6));
    }
}

pub fn update_debug_text(runtime: Res<GantryRuntime>, mut text: Query<&mut Text, With<DebugText>>) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let feet = runtime.run.body.position.y - runtime.config.half_height;
    let status = match &runtime.result {
        Some(result) => format!(
            "[PASS] exit={} time={:.2}s fell={}",
            result.exit.label(),
            result.seconds,
            result.fell_to_understory
        ),
        None => "[ ... ] running".to_string(),
    };
    **text = format!(
        "GANTRY LAB  {status}\n\
         route          {}\n\
         ticks          {} ({:.2}s)\n\
         feet height    {:.2} / upper {:.2}\n\
         resets         {}\n\n\
         1 clean jump | 2 fall recover | 3 safe bypass | R reset\n\
         targets: clean fast, fall medium, bypass slow\n\
         lower landing has a navigable return/side exit",
        runtime.route.label(),
        runtime.run.ticks,
        runtime.run.ticks as f32 * FIXED_DT,
        feet,
        UPPER_DECK_Y,
        runtime.reset_count,
    );
}
