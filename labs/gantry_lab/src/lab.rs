use bevy::{
    pbr::{DistanceFog, FogFalloff},
    prelude::*,
};
use observed_style::{SurfaceRole, Treatment, surface};
use observed_traversal::FpsConfig;
use observed_traversal::gantry::{
    GANTRY_EXPANSE_DECK_Y, GANTRY_EXPANSE_LENGTH, GANTRY_EXPANSE_WIDTH, GantryBridgeSpan,
    GantryDeck, GantryExpanseCourse, GantryExpanseRoute, GantryExpanseRunResult,
    GantryExpanseRunState, GantryHexColumn,
};

const LAB_SEED: u64 = 0x4741_4E54_5259_0002;

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
    pub course: GantryExpanseCourse,
    pub config: FpsConfig,
    pub run: GantryExpanseRunState,
    pub route: GantryExpanseRoute,
    pub result: Option<GantryExpanseRunResult>,
    pub trail: Vec<Vec3>,
    pub reset_count: u32,
}

impl Default for GantryRuntime {
    fn default() -> Self {
        let course = GantryExpanseCourse::generate(LAB_SEED);
        let config = FpsConfig::default();
        let route = GantryExpanseRoute::JumpLine;
        Self {
            run: GantryExpanseRunState::new(route, &course, &config),
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
        self.run = GantryExpanseRunState::new(self.route, &self.course, &self.config);
        self.result = None;
        self.trail.clear();
        self.reset_count += 1;
    }

    pub fn set_route(&mut self, route: GantryExpanseRoute) {
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
        DistanceFog {
            color: Color::srgb(0.008, 0.013, 0.024),
            falloff: FogFalloff::Linear {
                start: 24.0,
                end: 78.0,
            },
            ..default()
        },
        Transform::from_xyz(-38.0, 60.0, -68.0).looking_at(Vec3::new(-12.0, 30.0, -20.0), Vec3::Y),
        Name::new("Gantry route camera"),
    ));
    commands.spawn((
        GantrySpawned,
        DirectionalLight {
            illuminance: 6_200.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.75, -0.55, 0.0)),
        Name::new("Gantry high cold light"),
    ));
    commands.spawn((
        GantrySpawned,
        PointLight {
            color: Color::srgb(0.20, 0.52, 1.0),
            intensity: 1_400_000.0,
            range: 54.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-55.0, GANTRY_EXPANSE_DECK_Y + 6.0, -34.0),
        Name::new("Gantry entry practical"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.13, 0.18, 0.28),
        brightness: 28.0,
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
        Name::new("Gantry route runner"),
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
    yaw: f32,
    name: &'static str,
) {
    commands.spawn((
        GantrySpawned,
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(material),
        Transform::from_translation(center)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(size),
        Name::new(name),
    ));
}

fn spawn_course(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    course: &GantryExpanseCourse,
) {
    let floor = materials.add(material(&surface(SurfaceRole::Understory)));
    let deck = materials.add(material(&surface(SurfaceRole::GantryDeck)));
    let edge = materials.add(material(&surface(SurfaceRole::GantryEdge)));
    let bridge = materials.add(material(&surface(SurfaceRole::SafeBypass)));
    let column = materials.add(material(&surface(SurfaceRole::Wall)));

    spawn_box(
        commands,
        meshes,
        floor,
        Vec3::new(0.0, -0.08, 0.0),
        Vec3::new(GANTRY_EXPANSE_WIDTH, 0.16, GANTRY_EXPANSE_LENGTH),
        0.0,
        "Gantry understory floor",
    );
    spawn_deck(
        commands,
        meshes,
        deck.clone(),
        edge.clone(),
        course.entry_deck,
    );
    for jump_deck in &course.jump_decks {
        spawn_deck(commands, meshes, deck.clone(), edge.clone(), *jump_deck);
    }
    spawn_deck(commands, meshes, deck, edge.clone(), course.upper_exit_deck);
    for span in &course.bridge_spans {
        spawn_bridge(commands, meshes, bridge.clone(), edge.clone(), *span);
    }
    for hex_column in &course.columns {
        spawn_column(commands, meshes, column.clone(), *hex_column);
    }
    spawn_threshold_markers(commands, meshes, materials, course);
}

fn spawn_deck(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    deck: Handle<StandardMaterial>,
    edge: Handle<StandardMaterial>,
    platform: GantryDeck,
) {
    let height = platform.top_y - platform.bottom_y;
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
        platform.yaw,
        "Gantry high deck",
    );
    for side in [-1.0, 1.0] {
        let local = Vec3::new(side * platform.half.x, platform.top_y + 0.065, 0.0);
        let translation = Transform::from_rotation(Quat::from_rotation_y(platform.yaw))
            .transform_point(local)
            + Vec3::new(platform.center.x, 0.0, platform.center.y);
        spawn_box(
            commands,
            meshes,
            edge.clone(),
            translation,
            Vec3::new(0.10, 0.13, platform.half.y * 2.0),
            platform.yaw,
            "Gantry deck edge signal",
        );
    }
}

fn spawn_bridge(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    bridge: Handle<StandardMaterial>,
    edge: Handle<StandardMaterial>,
    span: GantryBridgeSpan,
) {
    let center = span.center();
    let height = span.top_y - span.bottom_y;
    spawn_box(
        commands,
        meshes,
        bridge,
        Vec3::new(center.x, span.bottom_y + height * 0.5, center.y),
        Vec3::new(span.width, height, span.length()),
        span.yaw(),
        "Gantry connected bridge span",
    );
    let direction = (span.end - span.start).normalize_or_zero();
    let tangent = Vec2::new(-direction.y, direction.x);
    for side in [-1.0, 1.0] {
        let edge_center = center + tangent * (span.width * 0.5 - 0.07) * side;
        spawn_box(
            commands,
            meshes,
            edge.clone(),
            Vec3::new(edge_center.x, span.top_y + 0.06, edge_center.y),
            Vec3::new(0.10, 0.12, span.length()),
            span.yaw(),
            "Gantry bridge edge signal",
        );
    }
}

fn spawn_column(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    column: GantryHexColumn,
) {
    let height = column.top_y - column.bottom_y;
    commands.spawn((
        GantrySpawned,
        Mesh3d(meshes.add(Cylinder::new(column.radius, height).mesh().resolution(6))),
        MeshMaterial3d(material),
        Transform::from_xyz(
            column.center.x,
            column.bottom_y + height * 0.5,
            column.center.y,
        ),
        Name::new("Gantry hexagonal megacolumn"),
    ));
}

fn spawn_threshold_markers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    course: &GantryExpanseCourse,
) {
    let marker = materials.add(material(&observed_style::marker(
        observed_style::MarkerRole::Exit,
    )));
    for threshold in &course.thresholds {
        let yaw = threshold.normal.x.atan2(-threshold.normal.y);
        spawn_box(
            commands,
            meshes,
            marker.clone(),
            Vec3::new(
                threshold.center.x,
                threshold.floor_y + 1.35,
                threshold.center.y,
            ),
            Vec3::new(threshold.width, 2.7, 0.16),
            yaw,
            "Gantry threshold endpoint",
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
            width: px(510),
            padding: UiRect::all(px(12)),
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.92)),
        BorderColor::all(Color::srgba(0.25, 0.9, 1.0, 0.55)),
        children![(
            DebugText,
            Text::new("Gantry Expanse"),
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
        runtime.set_route(GantryExpanseRoute::JumpLine);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        runtime.set_route(GantryExpanseRoute::HighBridge);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        runtime.set_route(GantryExpanseRoute::UnderstoryRecovery);
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
        runtime.trail.push(Vec3::new(p.x, p.y, p.z));
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
    let offset = if runtime.route == GantryExpanseRoute::UnderstoryRecovery {
        Vec3::new(-22.0, 18.0, -28.0)
    } else {
        Vec3::new(-14.0, 8.0, -20.0)
    };
    *transform =
        Transform::from_translation(p + offset).looking_at(p + Vec3::new(0.0, 1.2, 6.0), Vec3::Y);
}

pub fn draw_debug(runtime: Res<GantryRuntime>, mut gizmos: Gizmos) {
    let route_color = |route| match route {
        GantryExpanseRoute::JumpLine => Color::srgb(1.0, 0.72, 0.16),
        GantryExpanseRoute::HighBridge => Color::srgb(0.2, 0.85, 1.0),
        GantryExpanseRoute::UnderstoryRecovery => Color::srgb(0.25, 1.0, 0.55),
    };
    for path in &runtime.course.routes {
        let color = route_color(path.route);
        let points = path
            .nodes
            .iter()
            .map(|node| node.position + Vec3::Y * 0.18)
            .collect::<Vec<_>>();
        gizmos.linestrip(points, color);
        for node in &path.nodes {
            let center = node.position + Vec3::Y * 0.25;
            gizmos.line(center - Vec3::X * 0.24, center + Vec3::X * 0.24, color);
            gizmos.line(center - Vec3::Z * 0.24, center + Vec3::Z * 0.24, color);
        }
    }
    if runtime.trail.len() > 1 {
        gizmos.linestrip(runtime.trail.iter().copied(), Color::srgb(1.0, 0.95, 0.75));
    }
    for threshold in &runtime.course.thresholds {
        let p = Vec3::new(
            threshold.center.x,
            threshold.floor_y + 0.05,
            threshold.center.y,
        );
        gizmos.line(p, p + Vec3::Y * 3.0, Color::srgb(0.9, 0.3, 1.0));
    }
}

pub fn update_debug_text(runtime: Res<GantryRuntime>, mut text: Query<&mut Text, With<DebugText>>) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let feet = runtime.run.body.position.y - runtime.config.half_height;
    let path = runtime.course.route(runtime.route);
    let status = match &runtime.result {
        Some(result) => format!(
            "[PASS] exit={} time={:.2}s",
            result.exit.label(),
            result.seconds
        ),
        None => "[ ... ] running".to_string(),
    };
    **text = format!(
        "GANTRY EXPANSE  {status}\n\
         seed           0x{:016X}\n\
         route          {}\n\
         route node     {} / {}\n\
         route length   {:.1} m\n\
         feet height    {:.1} m / deck {:.1} m\n\
         footprint      {:.0} x {:.0} m\n\
         hex columns    {}\n\
         resets         {}\n\n\
         1 JUMP (amber, fast) | 2 BRIDGE (cyan, medium)\n\
         3 UNDERSTORY (green, slow) | R reset\n\
         purple = threshold endpoint; white = runner trail",
        runtime.course.seed,
        runtime.route.label(),
        runtime.run.waypoint_index + 1,
        path.nodes.len(),
        path.horizontal_length(),
        feet,
        GANTRY_EXPANSE_DECK_Y,
        GANTRY_EXPANSE_WIDTH,
        GANTRY_EXPANSE_LENGTH,
        runtime.course.columns.len(),
        runtime.reset_count,
    );
}

#[cfg(test)]
pub(crate) fn endpoint_for(
    route: GantryExpanseRoute,
) -> observed_traversal::gantry::GantryExpanseExit {
    use observed_traversal::gantry::GantryExpanseExit;
    match route {
        GantryExpanseRoute::JumpLine | GantryExpanseRoute::HighBridge => {
            GantryExpanseExit::UpperExit
        }
        GantryExpanseRoute::UnderstoryRecovery => GantryExpanseExit::LowerExit,
    }
}
