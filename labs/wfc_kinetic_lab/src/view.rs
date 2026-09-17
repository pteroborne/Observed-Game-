//! Presentation. Reads the simulation; never decides anything in it.
//!
//! The structural pass is deliberately the same one `hex_wfc_lab` uses for its
//! walkthrough: the same projected hulls, the same per-register shell surface,
//! the same weave. A lab that reproduced the solver's geometry and then painted
//! it in its own greys would be previewing a different building.

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::Hdr;
use bevy::camera::visibility::Visibility;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::widget::Text;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexArchetype;
use observed_hex::{HexCoord, face_edge, hex_origin};
use observed_match::hex_wfc::{HexStructurePiece, HexStructureRole};
use observed_style::ArchitectureSurfaceRole;
use observed_style::kinetic::{self, Role};
use observed_traversal::ColliderShape;

use crate::model::{Action, ActorId, Behavior, Event, Kind, Mode, Outcome, Refusal};
use crate::runtime::Runtime;

#[derive(Component)]
struct Eye;
#[derive(Component)]
struct Tool;
#[derive(Component)]
struct FacilityVisual;
/// A visual belonging to one lattice cell, so a retraction can hide exactly it.
#[derive(Component)]
struct CellVisual(HexCoord);
#[derive(Component)]
struct ActorVisual(ActorId);
#[derive(Component)]
struct PowerPart;
#[derive(Component)]
struct DeviceLabel {
    position: Vec3,
}
#[derive(Component)]
enum Flash {
    Muzzle,
    Impact,
}
#[derive(Component)]
enum HudField {
    Summary,
    Aim,
    Status,
    Debug,
}
#[derive(Component, Clone, Copy)]
enum UiAction {
    Practice,
    Encounter,
    Reset,
    Pause,
    Debug,
    NextFloor,
}

const ROLES: [Role; 14] = [
    Role::Floor,
    Role::Wall,
    Role::Catwalk,
    Role::Landing,
    Role::Hazard,
    Role::Minor,
    Role::Prop,
    Role::Target,
    Role::Push,
    Role::Pull,
    Role::Powered,
    Role::Unpowered,
    Role::Text,
    Role::Panel,
];

#[derive(Resource)]
struct Art {
    cube: Handle<Mesh>,
    materials: Vec<Handle<StandardMaterial>>,
}

impl Art {
    fn material(&self, role: Role) -> Handle<StandardMaterial> {
        self.materials[ROLES
            .iter()
            .position(|candidate| *candidate == role)
            .expect("every role used here is in ROLES")]
        .clone()
    }
}

#[derive(Resource, Default)]
struct ViewState {
    generation: Option<u32>,
    actors: BTreeMap<ActorId, Entity>,
    kick: f32,
    pulse: f32,
    impact: Vec3,
    pull: bool,
    message: String,
    message_until: u64,
}

fn color(role: Role) -> Color {
    kinetic::treatment(role).base_color
}

fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ViewState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                crate::evidence::advance,
                buttons,
                input,
                rebuild,
                spawn_actors,
                sync,
                warn_retraction,
                draw,
                events,
                feedback,
                labels,
                hud,
            )
                .chain(),
        );
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let art = Art {
        cube: meshes.add(Cuboid::from_length(1.)),
        materials: ROLES
            .iter()
            .map(|role| {
                let treatment = kinetic::treatment(*role);
                materials.add(StandardMaterial {
                    base_color: treatment.base_color,
                    emissive: treatment.emissive,
                    perceptual_roughness: 0.65,
                    metallic: 0.15,
                    ..default()
                })
            })
            .collect(),
    };
    commands.spawn((
        Eye,
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        Transform::default(),
        children![(
            Tool,
            Mesh3d(art.cube.clone()),
            MeshMaterial3d(art.material(Role::Wall)),
            Transform::from_xyz(0.30, -0.25, -0.65).with_scale(Vec3::new(0.15, 0.15, 0.38)),
            children![(
                Mesh3d(art.cube.clone()),
                MeshMaterial3d(art.material(Role::Powered)),
                Transform::from_xyz(0., 0., -0.55).with_scale(Vec3::new(0.8, 0.35, 0.1)),
            )],
        )],
    ));
    for flash in [Flash::Muzzle, Flash::Impact] {
        commands.spawn((
            flash,
            PointLight {
                intensity: 0.,
                range: 5.,
                ..default()
            },
            Transform::default(),
        ));
    }
    commands.insert_resource(ClearColor(color(Role::Panel)));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(30),
            top: px(24),
            ..default()
        },
        children![(
            Text::new("KINETIC / 02\nTHE SOLVED FLOOR"),
            font(23.),
            TextColor(color(Role::Text)),
        )],
    ));
    commands.spawn((
        HudField::Summary,
        Text::default(),
        font(16.),
        TextColor(color(Role::Text)),
        Node {
            position_type: PositionType::Absolute,
            right: px(30),
            top: px(24),
            ..default()
        },
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: percent(0),
            top: percent(48),
            width: percent(100),
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![
            (Text::new("+"), font(24.), TextColor(color(Role::Text))),
            (
                HudField::Aim,
                Text::default(),
                font(14.),
                TextColor(color(Role::Text))
            ),
        ],
    ));
    commands.spawn((
        HudField::Status,
        BackgroundColor(color(Role::Panel).with_alpha(0.94)),
        Text::default(),
        font(17.),
        TextColor(color(Role::Text)),
        Node {
            position_type: PositionType::Absolute,
            left: px(30),
            bottom: px(112),
            padding: UiRect::all(px(8)),
            ..default()
        },
    ));
    commands.spawn((
        HudField::Debug,
        Text::default(),
        font(13.),
        TextColor(color(Role::Text)),
        Node {
            position_type: PositionType::Absolute,
            left: px(30),
            top: px(100),
            ..default()
        },
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(30),
            bottom: px(28),
            right: px(30),
            column_gap: px(10),
            align_items: AlignItems::Center,
            ..default()
        },
        children![
            button("1  PRACTICE", UiAction::Practice),
            button("2  ENCOUNTER", UiAction::Encounter),
            button("R  RESET", UiAction::Reset),
            button("P  RESUME / PAUSE", UiAction::Pause),
            button("F3  INSPECT", UiAction::Debug),
            button("G  NEXT FLOOR", UiAction::NextFloor),
        ],
    ));
    commands.insert_resource(art);
}

fn button(label: &str, action: UiAction) -> impl Bundle {
    (
        Button,
        action,
        Node {
            padding: UiRect::axes(px(14), px(12)),
            ..default()
        },
        BackgroundColor(color(Role::Panel)),
        children![(Text::new(label), font(14.), TextColor(color(Role::Text)),)],
    )
}

fn apply(action: UiAction, runtime: &mut Runtime) {
    match action {
        UiAction::Practice => runtime.reset(Mode::Practice),
        UiAction::Encounter => runtime.reset(Mode::Encounter),
        UiAction::Reset => {
            let mode = runtime.world.mode;
            runtime.reset(mode);
        }
        UiAction::Pause => {
            if runtime.paused {
                runtime.paused = false;
            } else {
                runtime.pause();
            }
        }
        UiAction::Debug => runtime.diagnostics = !runtime.diagnostics,
        UiAction::NextFloor => {
            // Solve the next floor along. This is the one control that deals a
            // different board rather than restoring this one.
            let mode = runtime.world.mode;
            let next = runtime.site.requested_seed.wrapping_add(1);
            match crate::site::Site::solve(next, &crate::site::load_content()) {
                Ok(site) => runtime.reseat(std::sync::Arc::new(site), mode),
                Err(error) => eprintln!("could not solve the next floor: {error}"),
            }
        }
    }
}

fn buttons(
    mut runtime: ResMut<Runtime>,
    mut interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
) {
    for (interaction, action) in &mut interactions {
        if *interaction == Interaction::Pressed {
            apply(*action, &mut runtime);
        }
    }
}

fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut runtime: ResMut<Runtime>,
    mut window: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    capture: Option<Res<crate::evidence::Capture>>,
) {
    if capture.is_some() {
        return;
    }
    let Ok((window, mut cursor)) = window.single_mut() else {
        return;
    };
    for (key, action) in [
        (KeyCode::Digit1, UiAction::Practice),
        (KeyCode::Digit2, UiAction::Encounter),
        (KeyCode::KeyR, UiAction::Reset),
        (KeyCode::KeyP, UiAction::Pause),
        (KeyCode::F3, UiAction::Debug),
        (KeyCode::KeyG, UiAction::NextFloor),
    ] {
        if keys.just_pressed(key) {
            apply(action, &mut runtime);
        }
    }
    if keys.just_pressed(KeyCode::Escape) || !window.focused {
        runtime.pause();
    }
    if keys.just_pressed(KeyCode::KeyN) && runtime.paused {
        runtime.step_once = true;
    }
    if runtime.world.outcome != Outcome::Playing {
        runtime.pause();
    }
    if runtime.paused {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        return;
    }
    if cursor.grab_mode == CursorGrabMode::None {
        if mouse.just_pressed(MouseButton::Left)
            && window
                .cursor_position()
                .is_some_and(|point| point.y > 100. && point.y < window.height() - 100.)
        {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
        return;
    }
    let axis =
        |positive, negative| f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative));
    runtime.movement.movement = Vec2::new(
        axis(KeyCode::KeyD, KeyCode::KeyA),
        axis(KeyCode::KeyW, KeyCode::KeyS),
    );
    runtime.movement.look += Vec2::new(motion.delta.x, motion.delta.y) * 0.075;
    runtime.movement.sprint_held = keys.pressed(KeyCode::ShiftLeft);
    runtime.movement.jump_pressed |= keys.just_pressed(KeyCode::Space);
    for (pressed, action) in [
        (mouse.just_pressed(MouseButton::Left), Action::Push),
        (mouse.just_pressed(MouseButton::Right), Action::Pull),
        (keys.just_pressed(KeyCode::KeyE), Action::Interact),
    ] {
        if pressed {
            runtime.pending.push_back(action);
        }
    }
}

/// Rebuild the facility whenever the runtime deals a new floor or resets.
#[allow(clippy::too_many_arguments)]
fn rebuild(
    mut commands: Commands,
    runtime: Res<Runtime>,
    art: Res<Art>,
    mut view: ResMut<ViewState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    visuals: Query<Entity, With<FacilityVisual>>,
    labels: Query<Entity, With<DeviceLabel>>,
) {
    if view.generation == Some(runtime.generation) {
        return;
    }
    view.generation = Some(runtime.generation);
    view.actors.clear();
    for entity in visuals.iter().chain(labels.iter()) {
        commands.entity(entity).despawn();
    }
    let site = &runtime.site;

    // One weave image per register for the whole rebuild, not one per hull.
    let mut weaves: BTreeMap<ArchitectureRegister, Option<Handle<Image>>> = BTreeMap::new();
    for piece in &site.snapshot.pieces {
        let Some(mesh) = piece_mesh(piece) else {
            continue;
        };
        let register = site.register(piece.source_cell);
        let look = observed_style::hex_shell_surface(register, architecture_role(piece.role));
        let material = StandardMaterial {
            base_color: look.base_color,
            base_color_texture: if look.textured {
                weave(&mut images, &mut weaves, register)
            } else {
                None
            },
            emissive: look.emissive,
            perceptual_roughness: observed_style::architecture(register).surface_roughness,
            ..default()
        };
        commands.spawn((
            FacilityVisual,
            CellVisual(piece.source_cell),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(material)),
            Transform::from_translation(piece.center)
                .with_rotation(Quat::from_array(piece.rotation)),
            Name::new(format!("{:?} hull {}", piece.role, piece.id.0)),
        ));
    }

    // The tiles' own authored practicals, on the same light budget the game
    // gives them. Colour, intensity and shadow policy are style-owned; this
    // lab only says where the fixtures are and how many share a cell.
    let mut per_cell: BTreeMap<HexCoord, usize> = BTreeMap::new();
    for light in &site.snapshot.lights {
        *per_cell.entry(light.source_cell).or_default() += 1;
    }
    for light in &site.snapshot.lights {
        let register = site.register(light.source_cell);
        let practical = observed_style::hex_practical_light(
            register,
            composition(site, light.source_cell),
            per_cell.get(&light.source_cell).copied().unwrap_or(1),
        );
        commands.spawn((
            FacilityVisual,
            CellVisual(light.source_cell),
            PointLight {
                color: practical.color,
                intensity: practical.intensity,
                range: practical.range,
                radius: practical.radius,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(light.position),
            Name::new("Authored tile practical"),
        ));
    }

    // An unpowered floor costs range, never legibility, so the fill light stays
    // above the atmosphere ceiling and the devices below keep their emission.
    commands.spawn((
        FacilityVisual,
        DirectionalLight {
            illuminance: 900.,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-18., 34., 12.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: color(Role::Text),
        brightness: 46.,
        ..default()
    });

    // Devices. These are lab equipment, not solver output, so they are drawn in
    // the kinetic legend's own colours rather than the district's.
    for (position, role, label) in [
        (site.generator, Role::Powered, "GENERATOR / E"),
        (site.station, Role::Powered, "RECHARGE STATION"),
        (site.panel, Role::Hazard, "TILE CONTROL / E"),
    ] {
        let mut device = commands.spawn((
            FacilityVisual,
            Mesh3d(art.cube.clone()),
            MeshMaterial3d(art.material(role)),
            Transform::from_translation(position + Vec3::Y * 0.55)
                .with_scale(Vec3::new(0.8, 1.1, 0.6)),
            Name::new(label.to_string()),
        ));
        if role == Role::Powered {
            device.insert(PowerPart);
        }
        commands.spawn((
            DeviceLabel {
                position: position + Vec3::Y * 1.9,
            },
            Text::new(label),
            font(12.),
            TextColor(color(Role::Text)),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(color(Role::Panel)),
        ));
    }
}

fn spawn_actors(
    mut commands: Commands,
    runtime: Res<Runtime>,
    art: Res<Art>,
    mut view: ResMut<ViewState>,
) {
    for (id, actor) in &runtime.world.actors {
        if view.actors.contains_key(id) {
            continue;
        }
        let role = if actor.kind == Kind::Minor {
            Role::Minor
        } else {
            Role::Prop
        };
        let size = if actor.kind == Kind::Minor { 1.1 } else { 0.9 };
        let entity = commands
            .spawn((
                FacilityVisual,
                ActorVisual(*id),
                Mesh3d(art.cube.clone()),
                MeshMaterial3d(art.material(role)),
                Transform::from_scale(Vec3::splat(size)),
                Name::new(format!("{:?} {}", actor.kind, id.0)),
            ))
            .id();
        view.actors.insert(*id, entity);
    }
}

/// Actor transforms, filtered off the cell visuals so both can hold `Visibility`.
type ActorPoses<'w, 's> = Query<
    'w,
    's,
    (
        &'static ActorVisual,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    Without<CellVisual>,
>;
/// The camera, filtered off both visual families it shares `Transform` with.
type EyePose<'w, 's> =
    Query<'w, 's, &'static mut Transform, (With<Eye>, Without<ActorVisual>, Without<CellVisual>)>;
/// Per-cell visibility, so a retraction can hide exactly one tile.
type CellVisibility<'w, 's> =
    Query<'w, 's, (&'static CellVisual, &'static mut Visibility), Without<ActorVisual>>;

fn sync(
    runtime: Res<Runtime>,
    art: Res<Art>,
    time: Res<Time<Fixed>>,
    mut bodies: ActorPoses,
    mut camera: EyePose,
    mut cells: CellVisibility,
    mut power: Query<&mut MeshMaterial3d<StandardMaterial>, With<PowerPart>>,
) {
    let alpha = if runtime.paused {
        1.
    } else {
        time.overstep_fraction()
    };
    for (actor, mut transform, mut visible) in &mut bodies {
        let Some(state) = runtime.world.actors.get(&actor.0) else {
            continue;
        };
        *visible = if state.alive {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let now = runtime.world.pose(actor.0);
        let previous = runtime.previous.get(&actor.0).copied().unwrap_or(now);
        transform.translation = previous.position.lerp(now.position, alpha);
        transform.rotation = previous.rotation.slerp(now.rotation, alpha);
    }
    for mut transform in &mut camera {
        let world = &runtime.world;
        let mut body = world.player;
        body.position = runtime.previous_player.lerp(body.position, alpha);
        *transform = Transform::from_translation(body.eye(&world.player_config))
            .looking_to(world.player.look_dir(), Vec3::Y);
    }
    // A retracted tile stops being drawn on the tick its colliders go away.
    if !runtime.world.cell_present {
        let retracted = runtime.site.retracting;
        for (cell, mut visible) in &mut cells {
            if cell.0 == retracted {
                *visible = Visibility::Hidden;
            }
        }
    }
    for mut material in &mut power {
        material.0 = art.material(if runtime.world.powered {
            Role::Powered
        } else {
            Role::Unpowered
        });
    }
}

fn warn_retraction(runtime: Res<Runtime>, mut gizmos: Gizmos) {
    if runtime.world.retract_warning.is_none() || !runtime.world.cell_present {
        return;
    }
    // The tile that is about to stop existing, outlined in the hazard stripe on
    // a one-second blink. This is a Legibility Contract signal: it must read
    // from across the floor and through the fog.
    let role = if (runtime.world.tick / 10).is_multiple_of(2) {
        Role::Hazard
    } else {
        Role::Text
    };
    outline_cell(&mut gizmos, runtime.site.retracting, color(role), 0.2);
}

/// Draw a cell's hexagonal footprint at a height above its floor.
fn outline_cell(gizmos: &mut Gizmos, cell: HexCoord, color: Color, lift: f32) {
    let origin = Vec3::from_array(hex_origin(cell)) + Vec3::Y * lift;
    let corners: Vec<Vec3> = observed_hex::CORNERS
        .iter()
        .map(|(x, z)| origin + Vec3::new(*x as f32, 0., *z as f32))
        .collect();
    for index in 0..corners.len() {
        gizmos.line(corners[index], corners[(index + 1) % corners.len()], color);
    }
}

fn draw(runtime: Res<Runtime>, mut gizmos: Gizmos) {
    let world = &runtime.world;
    if let Ok(target) = world.target() {
        gizmos.cube(
            Transform::from_translation(world.pose(target.id).position)
                .with_scale(Vec3::splat(1.24)),
            color(Role::Target),
        );
    }
    // Unsafe edges are a permanent signal, not a debug one: a face that opens
    // onto void is the tool's answer to the horde and has to be findable.
    for ledge in &runtime.site.ledges {
        let origin = Vec3::from_array(hex_origin(ledge.cell));
        let [a, b] = face_edge(ledge.face);
        let lift = Vec3::Y * 0.12;
        let (start, end) = (
            origin + Vec3::new(a.0 as f32, 0., a.1 as f32) + lift,
            origin + Vec3::new(b.0 as f32, 0., b.1 as f32) + lift,
        );
        // A broken stripe, so it reads as an edge rather than a railing.
        for step in 0..6 {
            let from = start.lerp(end, step as f32 / 6.);
            let to = start.lerp(end, (step as f32 + 0.55) / 6.);
            gizmos.line(from, to, color(Role::Hazard));
        }
    }
    if !runtime.diagnostics {
        return;
    }
    for cell in &runtime.site.cells {
        outline_cell(&mut gizmos, *cell, color(Role::Landing), 0.05);
    }
    for cell in &runtime.site.voids {
        outline_cell(&mut gizmos, *cell, color(Role::Hazard), 0.05);
    }
    gizmos.line(
        world.eye(),
        world.eye() + world.player.look_dir() * world.config.reach,
        color(Role::Target),
    );
    for waypoint in &runtime.site.nav {
        gizmos.line(*waypoint, *waypoint + Vec3::Y * 0.3, color(Role::Catwalk));
    }
    for (id, actor) in &world.actors {
        if actor.alive {
            let position = world.pose(*id).position;
            gizmos.line(
                position,
                position + world.velocity(*id) * 0.2,
                color(Role::Push),
            );
        }
    }
}

fn events(mut runtime: ResMut<Runtime>, mut view: ResMut<ViewState>) {
    let tick = runtime.world.tick;
    // The queue survives FixedUpdate catch-up, so no feedback is lost between
    // rendered frames.
    let queued = std::mem::take(&mut runtime.events);
    for event in queued {
        let message = match &event {
            Event::Fired(action, _, point) => {
                view.kick = 1.;
                view.pulse = 1.;
                view.impact = *point;
                view.pull = *action == Action::Pull;
                None
            }
            Event::Refused(reason) => Some(refusal(*reason).to_string()),
            Event::Eliminated(_, Some(cell)) => Some(format!(
                "minor committed to void at ({}, {})",
                cell.q, cell.r
            )),
            Event::Eliminated(_, None) => Some("minor committed to void".to_string()),
            Event::Power(true) => Some("generator on".to_string()),
            Event::Power(false) => Some("generator off — the station is dead".to_string()),
            Event::Retracting => Some("tile retracting in two seconds".to_string()),
            Event::Retracted(cell) => Some(format!("tile ({}, {}) is gone", cell.q, cell.r)),
            Event::Wave(wave) => Some(format!("wave {wave}")),
            Event::Recharge => Some("charge full".to_string()),
            Event::Ended(outcome) => Some(
                match outcome {
                    Outcome::Cleared => "CLEARED",
                    Outcome::Captured => "CAUGHT",
                    Outcome::Fell => "FELL OUT OF THE FACILITY",
                    Outcome::Playing => "",
                }
                .to_string(),
            ),
        };
        if let Some(message) = message {
            view.message = message;
            view.message_until = tick + 150;
        }
    }
}

fn feedback(
    mut view: ResMut<ViewState>,
    time: Res<Time>,
    mut tool: Query<&mut Transform, With<Tool>>,
    mut flashes: Query<(&Flash, &mut PointLight, &mut Transform), Without<Tool>>,
    camera: Query<&GlobalTransform, With<Eye>>,
) {
    for mut transform in &mut tool {
        let kick = view.kick * 0.10 * if view.pull { -1. } else { 1. };
        transform.translation.z = -0.65 + kick;
    }
    if let Ok(eye) = camera.single() {
        for (flash, mut light, mut transform) in &mut flashes {
            match flash {
                Flash::Muzzle => {
                    light.intensity = view.kick * 9_000.;
                    light.color = color(if view.pull { Role::Pull } else { Role::Push });
                    transform.translation = eye.translation() + eye.forward() * 0.6;
                }
                Flash::Impact => {
                    light.intensity = view.pulse * 16_000.;
                    light.color = color(Role::Target);
                    transform.translation = view.impact;
                }
            }
        }
    }
    // Recoil and flash decay in real time, not simulation time: they are
    // presentation, and nothing downstream of them reaches the model.
    let decay = (1. - time.delta_secs() * 6.).clamp(0., 1.);
    view.kick *= decay;
    view.pulse *= decay;
}

fn labels(
    runtime: Res<Runtime>,
    camera: Query<(&Camera, &GlobalTransform), With<Eye>>,
    mut labels: Query<(&DeviceLabel, &mut Node)>,
) {
    let Ok((camera, transform)) = camera.single() else {
        return;
    };
    for (label, mut node) in &mut labels {
        if transform.translation().distance(label.position) < 16.
            && runtime
                .world
                .line_clear(transform.translation(), label.position)
            && let Ok(point) = camera.world_to_viewport(transform, label.position)
        {
            node.display = Display::Flex;
            node.left = px(point.x - 65.);
            node.top = px(point.y);
        } else {
            node.display = Display::None;
        }
    }
}

fn refusal(reason: Refusal) -> &'static str {
    match reason {
        Refusal::Empty => "no target",
        Refusal::Blocked => "the solver's architecture blocks the ray",
        Refusal::TooFar => "out of reach",
        Refusal::Cooldown => "tool recovering",
        Refusal::EmptyCharge => "recharge at the station",
    }
}

fn hud(runtime: Res<Runtime>, view: Res<ViewState>, mut texts: Query<(&mut Text, &HudField)>) {
    let world = &runtime.world;
    let site = &runtime.site;
    for (mut text, field) in &mut texts {
        text.0 = match field {
            HudField::Summary => {
                let charge = if world.mode == Mode::Practice {
                    "unlimited (practice)".to_string()
                } else {
                    format!("{:.0}", world.charge)
                };
                format!(
                    "SEED {}  ({} requested)\nPLAN {}\nCHARGE {}\nWAVE {} / 3    REMOVED {}\nGENERATOR {}",
                    site.seed,
                    site.requested_seed,
                    site.plan(),
                    charge,
                    world.wave,
                    world.kills,
                    if world.powered { "ON" } else { "OFF" },
                )
            }
            HudField::Aim => match world.fire_ready() {
                Ok(target) => format!("{:.1} m", target.distance),
                Err(reason) => refusal(reason).to_string(),
            },
            HudField::Status => {
                let mut lines = Vec::new();
                if runtime.paused {
                    lines.push(if world.outcome == Outcome::Playing {
                        "PAUSED — P to resume, then click to capture the mouse".to_string()
                    } else {
                        format!("{:?} — 1 or 2 to start again", world.outcome)
                    });
                }
                if let Some(prompt) = world.interaction() {
                    lines.push(format!("E  {prompt}"));
                }
                if world.tick < view.message_until && !view.message.is_empty() {
                    lines.push(view.message.clone());
                }
                lines.join("\n")
            }
            HudField::Debug => {
                if !runtime.diagnostics {
                    String::new()
                } else {
                    let pursuing = world
                        .actors
                        .values()
                        .filter(|actor| actor.alive && actor.behavior == Behavior::Pursue)
                        .count();
                    format!(
                        "tick {}  digest {:016x}\ncells {}  voids {}  ledges {}\nhulls {}  waypoints {}\npursuing {}  tile present {}\nretracting ({}, {})",
                        world.tick,
                        world.digest(),
                        site.cells.len(),
                        site.voids.len(),
                        site.ledges.len(),
                        site.colliders().len(),
                        site.nav.len(),
                        pursuing,
                        world.cell_present,
                        site.retracting.q,
                        site.retracting.r,
                    )
                }
            }
        };
    }
}

/// How a reader perceives a cell: a place, a way between places, or a joint
/// that changes floor. Style owns the light budget that follows from it.
fn composition(site: &crate::site::Site, cell: HexCoord) -> observed_style::HexComposition {
    let Some(placement) = site.placement(cell) else {
        return observed_style::HexComposition::Hall;
    };
    match placement.archetype {
        HexArchetype::Room | HexArchetype::Expanse => observed_style::HexComposition::Room,
        HexArchetype::RampUp | HexArchetype::RampHead | HexArchetype::Shaft => {
            observed_style::HexComposition::Vertical
        }
        _ => observed_style::HexComposition::Hall,
    }
}

fn architecture_role(role: HexStructureRole) -> ArchitectureSurfaceRole {
    match role {
        HexStructureRole::Room | HexStructureRole::Hall | HexStructureRole::Shaft => {
            ArchitectureSurfaceRole::Wall
        }
        HexStructureRole::Ramp => ArchitectureSurfaceRole::Floor,
        HexStructureRole::Boundary => ArchitectureSurfaceRole::Ceiling,
    }
}

fn piece_mesh(piece: &HexStructurePiece) -> Option<Mesh> {
    match &piece.shape {
        ColliderShape::Cuboid { half } => Some(Cuboid::from_size(*half * 2.0).mesh().build()),
        ColliderShape::ConvexHull { points } => hull_mesh(points),
    }
}

/// The same convex hull the shell draws, built by the same code — normals and
/// UVs included, so the register's weave survives onto the mesh.
fn hull_mesh(hull: &[Vec3]) -> Option<Mesh> {
    let data = observed_traversal::ConvexRenderMesh::from_convex_hull(hull)?;
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
        .with_inserted_indices(Indices::U32(data.indices)),
    )
}

fn weave(
    images: &mut Assets<Image>,
    cache: &mut BTreeMap<ArchitectureRegister, Option<Handle<Image>>>,
    register: ArchitectureRegister,
) -> Option<Handle<Image>> {
    if let Some(existing) = cache.get(&register) {
        return existing.clone();
    }
    let pattern = observed_style::architecture_weave(register);
    let made = if pattern.weave == observed_style::SurfaceWeave::None || pattern.lines == 0 {
        None
    } else {
        const N: usize = 128;
        let pitch = N as f32 / pattern.lines as f32;
        let half = (pitch * pattern.weight * 0.5).max(0.6);
        let on_line = |value: usize| ((value as f32 % pitch) - pitch * 0.5).abs() <= half;
        let struck_value = ((1.0 - pattern.depth) * 255.0).clamp(0.0, 255.0) as u8;
        let mut data = Vec::with_capacity(N * N * 4);
        for y in 0..N {
            for x in 0..N {
                let struck = match pattern.weave {
                    observed_style::SurfaceWeave::Courses => on_line(y),
                    observed_style::SurfaceWeave::Staves => on_line(x),
                    observed_style::SurfaceWeave::Grid => on_line(x) || on_line(y),
                    observed_style::SurfaceWeave::None => false,
                };
                let value = if struck { struck_value } else { 255 };
                data.extend_from_slice(&[value, value, value, 255]);
            }
        }
        let mut image = Image::new(
            Extent3d {
                width: N as u32,
                height: N as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..default()
        });
        Some(images.add(image))
    };
    cache.insert(register, made.clone());
    made
}
