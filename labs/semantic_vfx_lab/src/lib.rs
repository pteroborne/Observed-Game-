//! semantic_vfx_lab -- Phase A4 of the Bevy asset-integration roadmap.
//!
//! The lab proves `bevy_hanabi 0.18` as a lab-local semantic VFX renderer. It
//! projects deterministic gameplay events into particle effects while keeping
//! doors, hazards, players, objectives, and equipment markers visible through the
//! shared `observed_style` semantic treatments.

mod model;

pub use model::{
    EventPosition, SEQUENCE_SECONDS, ScheduledVfx, SemanticVfxRole, VfxSpec, VfxTimeline,
    canonical_sequence, crowded_sequence,
};

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
use bevy_hanabi::prelude::*;
use observed_style::{MarkerRole, SurfaceRole, Treatment, marker, surface};

#[derive(Component)]
pub struct SemanticVfxCam;

#[derive(Component)]
pub struct SemanticVfxUiRoot;

#[derive(Component)]
pub struct LabSpawned;

#[derive(Component)]
pub struct CriticalAnchor {
    role: &'static str,
}

#[derive(Component)]
pub struct ActiveVfx {
    role: SemanticVfxRole,
    remaining_secs: f32,
}

#[derive(Component)]
struct ContinuousEquipmentPower;

#[derive(Component)]
struct DebugPanel;

#[derive(Component)]
struct HelpPanel;

#[derive(Component)]
struct DebugText;

#[derive(Resource)]
pub struct LabState {
    pub debug_visible: bool,
    pub vfx_enabled: bool,
    pub reset_requested: bool,
    pub reset_count: u32,
    pub sequence_clock: f32,
    pub timeline: VfxTimeline,
    pub auto_sequence: bool,
    pub crowded_request: bool,
    pub projected_events: u32,
    pub spawned_events: u32,
    pub skipped_events: u32,
    pub last_event: &'static str,
    pub vfx_toggle_dirty: bool,
    pub camera_override: Option<Transform>,
}

impl Default for LabState {
    fn default() -> Self {
        Self {
            debug_visible: true,
            vfx_enabled: true,
            reset_requested: false,
            reset_count: 0,
            sequence_clock: 0.0,
            timeline: VfxTimeline::default(),
            auto_sequence: true,
            crowded_request: false,
            projected_events: 0,
            spawned_events: 0,
            skipped_events: 0,
            last_event: "none",
            vfx_toggle_dirty: true,
            camera_override: None,
        }
    }
}

#[derive(Resource, Clone)]
struct VfxHandles {
    door_open: Handle<EffectAsset>,
    door_slam: Handle<EffectAsset>,
    reroute_flash: Handle<EffectAsset>,
    pressure_gate: Handle<EffectAsset>,
    keystone_pickup: Handle<EffectAsset>,
    exit_unlock: Handle<EffectAsset>,
    equipment_power: Handle<EffectAsset>,
}

impl VfxHandles {
    fn get(&self, role: SemanticVfxRole) -> Handle<EffectAsset> {
        match role {
            SemanticVfxRole::DoorOpen => self.door_open.clone(),
            SemanticVfxRole::DoorSlam => self.door_slam.clone(),
            SemanticVfxRole::RerouteFlash => self.reroute_flash.clone(),
            SemanticVfxRole::PressureGate => self.pressure_gate.clone(),
            SemanticVfxRole::KeystonePickup => self.keystone_pickup.clone(),
            SemanticVfxRole::ExitUnlock => self.exit_unlock.clone(),
            SemanticVfxRole::EquipmentPower => self.equipment_power.clone(),
        }
    }
}

#[derive(Clone, Copy)]
struct AnchorSpec {
    position: Vec3,
    size: Vec3,
    treatment: Treatment,
    name: &'static str,
}

pub struct SemanticVfxLabPlugin;

impl Plugin for SemanticVfxLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LabState>()
            .add_systems(Startup, setup_scene)
            .add_systems(
                Update,
                (
                    handle_input.after(InputSystems),
                    perform_reset,
                    drive_semantic_timeline,
                    apply_vfx_enabled,
                    cleanup_finished_vfx,
                    present_camera,
                    update_debug_text,
                )
                    .chain(),
            );
    }
}

fn vec3(position: EventPosition) -> Vec3 {
    Vec3::new(position.x, position.y, position.z)
}

fn material_for(treatment: Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        perceptual_roughness: 0.74,
        metallic: 0.08,
        ..default()
    }
}

fn particle_rgba(treatment: Treatment, alpha: f32) -> Vec4 {
    let c = treatment.emissive;
    if c.red + c.green + c.blue > 0.0 {
        let max_channel = c.red.max(c.green).max(c.blue).max(1.0);
        Vec4::new(
            c.red / max_channel,
            c.green / max_channel,
            c.blue / max_channel,
            alpha,
        )
    } else {
        let s = treatment.base_color.to_srgba();
        Vec4::new(s.red, s.green, s.blue, alpha)
    }
}

fn create_effect_asset(role: SemanticVfxRole) -> EffectAsset {
    let spec = role.spec();
    let treatment = role.treatment();
    let mut color_gradient = bevy_hanabi::Gradient::new();
    color_gradient.add_key(0.0, particle_rgba(treatment, 0.32));
    color_gradient.add_key(0.65, particle_rgba(treatment, 0.08));
    color_gradient.add_key(1.0, Vec4::ZERO);

    let mut size_gradient = bevy_hanabi::Gradient::new();
    size_gradient.add_key(0.0, Vec3::splat(spec.size_px));
    size_gradient.add_key(0.7, Vec3::splat(spec.size_px * 0.65));
    size_gradient.add_key(1.0, Vec3::ZERO);

    let writer = ExprWriter::new();
    let age = writer.lit(0.0).expr();
    let init_age = SetAttributeModifier::new(Attribute::AGE, age);
    let lifetime = writer.lit(spec.lifetime_secs).expr();
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, lifetime);
    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(spec.radius).expr(),
        dimension: ShapeDimension::Volume,
    };
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(spec.speed).expr(),
    };
    let drag = LinearDragModifier::new(writer.lit(2.1).expr());
    let lift = match role {
        SemanticVfxRole::DoorSlam | SemanticVfxRole::PressureGate => Vec3::Y * -0.8,
        SemanticVfxRole::EquipmentPower => Vec3::Y * 0.35,
        _ => Vec3::Y * 0.7,
    };
    let accel = AccelModifier::new(writer.lit(lift).expr());
    let mut module = writer.finish();
    let round = RoundModifier::constant(&mut module, 1.0);

    let spawner = if spec.continuous {
        SpawnerSettings::rate((spec.particle_count as f32).into())
    } else {
        SpawnerSettings::once((spec.particle_count as f32).into())
    };

    EffectAsset::new((spec.particle_count * 16).max(256), spawner, module)
        .with_name(role.label())
        .with_alpha_mode(bevy_hanabi::AlphaMode::Premultiply)
        .with_simulation_space(SimulationSpace::Global)
        .init(init_age)
        .init(init_lifetime)
        .init(init_pos)
        .init(init_vel)
        .update(drag)
        .update(accel)
        .render(ColorOverLifetimeModifier::new(color_gradient))
        .render(SizeOverLifetimeModifier {
            gradient: size_gradient,
            screen_space_size: true,
        })
        .render(round)
}

fn create_effect_handles(effects: &mut Assets<EffectAsset>) -> VfxHandles {
    VfxHandles {
        door_open: effects.add(create_effect_asset(SemanticVfxRole::DoorOpen)),
        door_slam: effects.add(create_effect_asset(SemanticVfxRole::DoorSlam)),
        reroute_flash: effects.add(create_effect_asset(SemanticVfxRole::RerouteFlash)),
        pressure_gate: effects.add(create_effect_asset(SemanticVfxRole::PressureGate)),
        keystone_pickup: effects.add(create_effect_asset(SemanticVfxRole::KeystonePickup)),
        exit_unlock: effects.add(create_effect_asset(SemanticVfxRole::ExitUnlock)),
        equipment_power: effects.add(create_effect_asset(SemanticVfxRole::EquipmentPower)),
    }
}

fn spawn_anchor(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    cube: &Handle<Mesh>,
    spec: AnchorSpec,
) {
    let material = materials.add(material_for(spec.treatment));
    let srgb = spec.treatment.base_color.to_srgba();
    commands
        .spawn((
            LabSpawned,
            CriticalAnchor { role: spec.name },
            Mesh3d(cube.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(spec.position).with_scale(spec.size),
            Name::new(spec.name),
        ))
        .with_children(|parent| {
            parent.spawn((
                PointLight {
                    color: Color::srgb(srgb.red, srgb.green, srgb.blue),
                    intensity: if spec.treatment.signal {
                        5_500.0
                    } else {
                        700.0
                    },
                    range: 6.5,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.8, 0.0),
            ));
        });
}

fn setup_scene(
    mut commands: Commands,
    state: Res<LabState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut effects: ResMut<Assets<EffectAsset>>,
) {
    let handles = create_effect_handles(&mut effects);
    commands.insert_resource(handles.clone());
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.24, 0.34, 0.52),
        brightness: 75.0,
        ..default()
    });

    commands.spawn((
        LabSpawned,
        SemanticVfxCam,
        Camera3d::default(),
        Hdr,
        Bloom {
            intensity: 0.07,
            ..Bloom::NATURAL
        },
        DistanceFog {
            color: surface(SurfaceRole::Ceiling).base_color,
            falloff: FogFalloff::Linear {
                start: 7.0,
                end: 36.0,
            },
            ..default()
        },
        Transform::from_xyz(7.5, 6.2, 17.0).looking_at(Vec3::new(0.0, 1.4, -4.0), Vec3::Y),
        Name::new("Semantic VFX Camera"),
    ));

    commands.spawn((
        LabSpawned,
        DirectionalLight {
            illuminance: 1_100.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.75, -0.35, 0.0)),
        Name::new("Dim facility key light"),
    ));

    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let plain = surface(SurfaceRole::Plain);
    let wall = surface(SurfaceRole::Wall);
    let spine = surface(SurfaceRole::Spine);
    let trap = surface(SurfaceRole::TrapArmed);

    for spec in [
        AnchorSpec {
            position: Vec3::new(0.0, -0.08, -3.5),
            size: Vec3::new(16.0, 0.16, 42.0),
            treatment: plain,
            name: "dark floor",
        },
        AnchorSpec {
            position: Vec3::new(-7.2, 2.0, -3.5),
            size: Vec3::new(0.35, 4.0, 42.0),
            treatment: wall,
            name: "left wall",
        },
        AnchorSpec {
            position: Vec3::new(7.2, 2.0, -3.5),
            size: Vec3::new(0.35, 4.0, 42.0),
            treatment: wall,
            name: "right wall",
        },
        AnchorSpec {
            position: Vec3::new(0.0, 0.03, -6.0),
            size: Vec3::new(2.0, 0.08, 24.0),
            treatment: spine,
            name: "route spine",
        },
        AnchorSpec {
            position: Vec3::new(0.0, 1.25, -0.9),
            size: Vec3::new(6.4, 2.5, 0.16),
            treatment: trap,
            name: "pressure gate hazard",
        },
        AnchorSpec {
            position: Vec3::new(-2.9, 1.35, 5.8),
            size: Vec3::new(0.25, 2.7, 1.1),
            treatment: marker(MarkerRole::Exit),
            name: "open door signal",
        },
        AnchorSpec {
            position: Vec3::new(3.6, 1.45, 2.9),
            size: Vec3::new(2.3, 2.9, 0.25),
            treatment: marker(MarkerRole::Collapse),
            name: "slamming door signal",
        },
        AnchorSpec {
            position: Vec3::new(-3.8, 0.75, 0.9),
            size: Vec3::splat(0.75),
            treatment: marker(MarkerRole::Control),
            name: "keystone pickup signal",
        },
        AnchorSpec {
            position: Vec3::new(0.0, 2.05, -14.0),
            size: Vec3::new(1.1, 4.1, 0.55),
            treatment: marker(MarkerRole::Exit),
            name: "exit objective signal",
        },
        AnchorSpec {
            position: Vec3::new(4.4, 1.05, 6.2),
            size: Vec3::new(0.75, 2.1, 0.75),
            treatment: marker(MarkerRole::You),
            name: "local player signal",
        },
        AnchorSpec {
            position: Vec3::new(4.4, 0.65, 2.1),
            size: Vec3::new(1.15, 0.9, 1.15),
            treatment: marker(MarkerRole::Teammate),
            name: "powered equipment signal",
        },
    ] {
        spawn_anchor(&mut commands, &mut materials, &cube, spec);
    }

    let mut equipment = ParticleEffect::new(handles.get(SemanticVfxRole::EquipmentPower));
    equipment.prng_seed = Some(70_707);
    commands.spawn((
        LabSpawned,
        ContinuousEquipmentPower,
        equipment,
        Transform::from_translation(Vec3::new(4.4, 1.35, 2.1)),
        if state.vfx_enabled {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
        Name::new("equipment power semantic VFX"),
    ));

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            LabSpawned,
            SemanticVfxUiRoot,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
            Name::new("Semantic VFX UI Root"),
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
                BorderColor::all(marker(MarkerRole::NextRoom).base_color.with_alpha(0.7)),
                children![(
                    DebugText,
                    Text::new("Semantic VFX diagnostics starting..."),
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
                    width: px(440),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(surface(SurfaceRole::Wall).base_color.with_alpha(0.94)),
                BorderColor::all(marker(MarkerRole::Control).base_color.with_alpha(0.7)),
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
        "SEMANTIC VFX LAB (Phase A4)\n\
         Space crowded burst  V toggle VFX  R reset  F1 overlay\n\n\
         Hanabi is projection-only: the timeline keeps advancing when VFX is off.\n\n\
         Event legend:\n",
    );
    for role in SemanticVfxRole::ALL {
        let spec = role.spec();
        let mode = if spec.continuous { "rate" } else { "burst" };
        out.push_str(&format!(
            "  {:<17} {:>5} particles  {:<5} {:.2}s\n",
            role.label(),
            spec.particle_count,
            mode,
            spec.lifetime_secs
        ));
    }
    out
}

fn handle_input(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<LabState>) {
    if keys.just_pressed(KeyCode::Space) {
        state.crowded_request = true;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        state.vfx_enabled = !state.vfx_enabled;
        state.vfx_toggle_dirty = true;
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
    state.sequence_clock = 0.0;
    state.timeline.reset();
    state.last_event = "reset";
    state.vfx_toggle_dirty = true;
    for entity in &spawned {
        commands.entity(entity).despawn();
    }
    commands.run_system_cached(setup_scene);
}

fn drive_semantic_timeline(
    mut commands: Commands,
    time: Res<Time>,
    handles: Option<Res<VfxHandles>>,
    mut state: ResMut<LabState>,
) {
    let delta = time.delta_secs();
    if state.auto_sequence {
        state.sequence_clock += delta;
        if state.sequence_clock > SEQUENCE_SECONDS {
            state.sequence_clock = 0.0;
            state.timeline.reset();
        }
    }

    let sequence_clock = state.sequence_clock;
    let mut events = state.timeline.ready_until(sequence_clock);
    if state.crowded_request {
        state.crowded_request = false;
        events.extend(crowded_sequence());
    }
    if events.is_empty() {
        return;
    }

    state.projected_events += events.len() as u32;
    for event in events {
        state.last_event = event.role.label();
        if state.vfx_enabled {
            if let Some(handles) = handles.as_ref() {
                spawn_particle_projection(&mut commands, handles, event);
                state.spawned_events += 1;
            }
        } else {
            state.skipped_events += 1;
        }
    }
}

fn spawn_particle_projection(commands: &mut Commands, handles: &VfxHandles, event: ScheduledVfx) {
    let spec = event.role.spec();
    let mut effect = ParticleEffect::new(handles.get(event.role));
    effect.prng_seed = Some(event.seed);
    commands.spawn((
        LabSpawned,
        ActiveVfx {
            role: event.role,
            remaining_secs: spec.cleanup_secs,
        },
        effect,
        Transform::from_translation(vec3(event.position)),
        Name::new(format!("{} semantic VFX", event.role.label())),
    ));
}

fn apply_vfx_enabled(
    mut commands: Commands,
    mut state: ResMut<LabState>,
    active: Query<Entity, With<ActiveVfx>>,
    mut continuous: Query<
        (&mut Visibility, Option<&mut EffectSpawner>),
        With<ContinuousEquipmentPower>,
    >,
) {
    if !state.vfx_toggle_dirty {
        return;
    }
    state.vfx_toggle_dirty = false;
    if !state.vfx_enabled {
        for entity in &active {
            commands.entity(entity).despawn();
        }
    }
    for (mut visibility, spawner) in &mut continuous {
        *visibility = if state.vfx_enabled {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Some(mut spawner) = spawner {
            spawner.active = state.vfx_enabled;
        }
    }
}

fn cleanup_finished_vfx(
    time: Res<Time>,
    mut commands: Commands,
    mut active: Query<(Entity, &mut ActiveVfx)>,
) {
    let delta = time.delta_secs();
    for (entity, mut vfx) in &mut active {
        vfx.remaining_secs -= delta;
        if vfx.remaining_secs <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn present_camera(state: Res<LabState>, mut camera: Single<&mut Transform, With<SemanticVfxCam>>) {
    if let Some(transform) = state.camera_override {
        **camera = transform;
    }
}

#[derive(SystemParam)]
struct DebugContext<'w, 's> {
    state: Res<'w, LabState>,
    cameras: Query<'w, 's, (), With<SemanticVfxCam>>,
    ui_roots: Query<'w, 's, (), With<SemanticVfxUiRoot>>,
    anchors: Query<'w, 's, &'static CriticalAnchor>,
    active: Query<'w, 's, &'static ActiveVfx>,
    continuous: Query<'w, 's, (), With<ContinuousEquipmentPower>>,
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
    let anchor_count = context.anchors.iter().count();
    let active_count = context.active.iter().count();
    let continuous_count = context.continuous.iter().count();
    let active_roles: Vec<&'static str> = context
        .active
        .iter()
        .map(|vfx| vfx.role.label())
        .take(5)
        .collect();
    let sample_anchor = context
        .anchors
        .iter()
        .next()
        .map(|anchor| anchor.role)
        .unwrap_or("none");
    let healthy = camera_count == 1
        && ui_count == 1
        && anchor_count >= 7
        && continuous_count == 1
        && SemanticVfxRole::ALL
            .iter()
            .all(|role| role.treatment().signal && role.spec().size_px <= 4.2);

    *context.text.into_inner() = Text::new(format!(
        "SEMANTIC VFX  {}\n\
         particle plugin    bevy_hanabi 0.18.0\n\
         VFX projection     {}\n\
         projected events   {}\n\
         spawned / skipped  {} / {}\n\
         active bursts      {}\n\
         continuous power   {}\n\
         last event         {}\n\
         active roles       {}\n\
         anchors            {} (sample: {})\n\
         cameras {}  UI {}  resets {}\n\n\
         Effects are semantic projections; toggling VFX off does not reset the\n\
         event timeline or remove the gameplay signal anchors.",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if context.state.vfx_enabled {
            "enabled"
        } else {
            "disabled"
        },
        context.state.projected_events,
        context.state.spawned_events,
        context.state.skipped_events,
        active_count,
        continuous_count,
        context.state.last_event,
        if active_roles.is_empty() {
            "none".to_string()
        } else {
            active_roles.join(", ")
        },
        anchor_count,
        sample_anchor,
        camera_count,
        ui_count,
        context.state.reset_count,
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(surface(SurfaceRole::Ceiling).base_color))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 - Semantic VFX Lab (Phase A4)".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(HanabiPlugin)
        .add_plugins(SemanticVfxLabPlugin);

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
        state.auto_sequence = false;
        state.vfx_enabled = true;
        state.vfx_toggle_dirty = true;
        state.crowded_request = true;
        state.camera_override = Some(
            Transform::from_xyz(7.5, 6.2, 17.0).looking_at(Vec3::new(0.0, 1.4, -4.0), Vec3::Y),
        );
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.45 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.0 {
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
            .init_asset::<EffectAsset>()
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins(SemanticVfxLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_ui_anchors_and_continuous_effect() {
        let mut app = test_app();
        assert_eq!(count::<SemanticVfxCam>(&mut app), 1);
        assert_eq!(count::<SemanticVfxUiRoot>(&mut app), 1);
        assert!(count::<CriticalAnchor>(&mut app) >= 7);
        assert_eq!(count::<ContinuousEquipmentPower>(&mut app), 1);
    }

    #[test]
    fn crowded_trigger_projects_every_discrete_event() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.world_mut().run_schedule(Update);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Space);
        assert_eq!(
            count::<ActiveVfx>(&mut app),
            SemanticVfxRole::DISCRETE.len()
        );
        assert_eq!(
            app.world().resource::<LabState>().spawned_events as usize,
            SemanticVfxRole::DISCRETE.len()
        );
    }

    #[test]
    fn vfx_toggle_does_not_stop_event_projection() {
        let mut app = test_app();
        {
            let mut state = app.world_mut().resource_mut::<LabState>();
            state.vfx_enabled = false;
            state.vfx_toggle_dirty = true;
            state.crowded_request = true;
        }
        app.world_mut().run_schedule(Update);
        assert_eq!(count::<ActiveVfx>(&mut app), 0);
        let state = app.world().resource::<LabState>();
        assert_eq!(
            state.projected_events as usize,
            SemanticVfxRole::DISCRETE.len()
        );
        assert_eq!(
            state.skipped_events as usize,
            SemanticVfxRole::DISCRETE.len()
        );
    }

    #[test]
    fn reset_rebuilds_scene_without_entity_leaks() {
        let mut app = test_app();
        let baseline = count::<LabSpawned>(&mut app);
        assert!(baseline > 0);
        for expected in 1..=3 {
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
            assert_eq!(count::<SemanticVfxCam>(&mut app), 1);
            assert_eq!(count::<SemanticVfxUiRoot>(&mut app), 1);
            assert_eq!(app.world().resource::<LabState>().reset_count, expected);
        }
    }
}
