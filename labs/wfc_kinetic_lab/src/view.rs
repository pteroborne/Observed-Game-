//! Presentation. Reads the simulation; never decides anything in it.
//!
//! The structural pass is deliberately the same one `hex_wfc_lab` uses for its
//! walkthrough: the same projected hulls, the same per-register shell surface,
//! the same weave. A lab that reproduced the solver's geometry and then painted
//! it in its own greys would be previewing a different building.

use std::collections::BTreeMap;

use bevy::anti_alias::fxaa::Fxaa;
use bevy::asset::RenderAssetUsages;
use bevy::camera::Hdr;
use bevy::camera::visibility::Visibility;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{
    DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::widget::Text;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexArchetype;
use observed_hex::{HexCoord, face_edge, hex_origin};
use observed_match::hex_wfc::HexStructurePiece;
use observed_style::ArchitectureSurfaceRole;
use observed_style::kinetic::{self, Role};
use observed_traversal::ColliderShape;

use crate::model::{Action, ActorId, Behavior, Event, Kind, Mode, Outcome, Refusal};
use crate::runtime::Runtime;

#[derive(Component)]
struct Eye;
#[derive(Component)]
struct Tool;
/// Everything a rebuild or a reset sweeps away, sound included.
#[derive(Component)]
pub struct FacilityVisual;
/// A visual belonging to the facility's cells, as opposed to an actor.
///
/// A marker rather than the cell it came from: geometry is rebuilt wholesale
/// when the floor changes, so nothing needs to find one cell's meshes and hide
/// them any more.
#[derive(Component)]
struct CellVisual;
#[derive(Component)]
struct ActorVisual(ActorId);
#[derive(Component)]
struct PowerPart;
/// The single key light, which follows the Observer's cell the way the game's
/// does rather than there being one per tile.
#[derive(Component)]
struct KeyLight;
/// One authored tile fixture: where it is, and whether its register lets it
/// cast.
///
/// The position is stored rather than read from `GlobalTransform`, because
/// transform propagation runs in `PostUpdate` and these are spawned in the same
/// `Update` chain that ranks them. Read live, every freshly spawned fixture
/// reports the origin, all distances tie, and the budget selects none — which
/// then latches until the Observer crosses a cell boundary.
#[derive(Component)]
struct Practical {
    at: Vec3,
    shadows_allowed: bool,
}
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
    ForceUp,
    ForceDown,
    MinorFaster,
    MinorSlower,
    /// Handled in the sound mixer, which owns the mute flag; the button exists
    /// so the control is discoverable without the keyboard.
    Mute,
    /// Swap play lighting for the studio's fill, to look at the building
    /// rather than stand in it.
    Inspect,
}

const ROLES: [Role; 15] = [
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
    Role::GravityWarning,
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
    generation: Option<(u32, u32)>,
    actors: BTreeMap<ActorId, Entity>,
    /// Studio lighting, for reviewing the floor.
    inspect: bool,
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
        .init_resource::<kinetic_lab::sound::SoundState>()
        .add_systems(
            Startup,
            (
                setup,
                kinetic_lab::guardian::setup,
                kinetic_lab::sound::setup,
            ),
        )
        .add_systems(
            Update,
            (
                crate::evidence::advance,
                buttons,
                input,
                rebuild,
                spawn_actors,
                sync,
                animate_guardians,
                atmosphere,
                practical_shadows,
                warn_retraction,
                draw,
                events,
                crate::sound::motion,
                crate::sound::mix,
                feedback,
                labels,
                hud,
            )
                .chain(),
        );
}

fn setup(
    mut view: ResMut<ViewState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // So a capture run can be taken under inspection lighting without a
    // keypress, which is the only way to put the two side by side in evidence.
    view.inspect = std::env::var("OBSERVED2_INSPECT").is_ok();
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
        // The ear rides the camera, so a Guardian behind a wall is behind it.
        bevy::audio::SpatialListener::new(0.3),
        Camera3d::default(),
        Hdr,
        Bloom {
            intensity: 0.08,
            ..Bloom::NATURAL
        },
        // Atmosphere is not decoration here. The tiles' look was developed in
        // `daydream_lab` and is shipped by the game's shell, and both light a
        // facility with district ambient, district fog and one key — the shell
        // explicitly zeroes the sun. Painting the same hulls with a neutral
        // ambient and a directional light, which is what this lab did, reads
        // flat and faintly outdoors and loses the hue that separates one
        // district from another.
        DistanceFog {
            color: color(Role::Panel),
            falloff: FogFalloff::Linear {
                start: 10.0,
                end: 28.0,
            },
            ..default()
        },
        // Ambient occlusion, and the prepasses it needs. Without it these
        // flat-shaded authored hulls have no contact darkening at all and the
        // whole facility reads as one milky wash — which is exactly what it
        // did. The game's shell runs it at Low for the same reason and on the
        // same geometry.
        Msaa::Off,
        Fxaa::default(),
        DepthPrepass,
        NormalPrepass,
        ScreenSpaceAmbientOcclusion {
            quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Low,
            ..default()
        },
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
            button("[ ]  FORCE", UiAction::ForceUp),
            button("-  =  SPEED", UiAction::MinorFaster),
            button("M  MUTE", UiAction::Mute),
            button("Q ARM / F PLUMB", UiAction::Mute),
            button("L  INSPECT LIGHT", UiAction::Inspect),
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
        // Tuning is live because the lab exists to find these numbers. They
        // survive a reset so a value can be tried across several attempts.
        UiAction::Mute => {}
        UiAction::Inspect => {}
        UiAction::ForceUp => runtime.tuning.scale_force(1.12),
        UiAction::ForceDown => runtime.tuning.scale_force(1. / 1.12),
        UiAction::MinorFaster => runtime.tuning.scale_minor_speed(1.12),
        UiAction::MinorSlower => runtime.tuning.scale_minor_speed(1. / 1.12),
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
    mut view: ResMut<ViewState>,
    mut interactions: Query<(&Interaction, &UiAction), Changed<Interaction>>,
) {
    for (interaction, action) in &mut interactions {
        if *interaction == Interaction::Pressed {
            if matches!(action, UiAction::Inspect) {
                view.inspect = !view.inspect;
            }
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
    mut view: ResMut<ViewState>,
    capture: Option<Res<crate::evidence::Capture>>,
) {
    if capture.is_some() {
        return;
    }
    if keys.just_pressed(KeyCode::KeyL) {
        view.inspect = !view.inspect;
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
        (KeyCode::BracketRight, UiAction::ForceUp),
        (KeyCode::BracketLeft, UiAction::ForceDown),
        (KeyCode::Equal, UiAction::MinorFaster),
        (KeyCode::Minus, UiAction::MinorSlower),
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
        (
            mouse.just_pressed(MouseButton::Middle) || keys.just_pressed(KeyCode::KeyF),
            Action::Plumb,
        ),
        (keys.just_pressed(KeyCode::KeyQ), Action::Arm),
        (keys.just_pressed(KeyCode::KeyC), Action::SelfPlumb),
        (keys.just_pressed(KeyCode::KeyX), Action::Release),
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
    // A relayout changes geometry without resetting the lab, so the trigger is
    // both: which attempt this is, and how many times the floor has moved.
    let stamp = (runtime.generation, runtime.world.geometry_generation);
    if view.generation == Some(stamp) {
        return;
    }
    view.generation = Some(stamp);
    view.actors.clear();
    for entity in visuals.iter().chain(labels.iter()) {
        commands.entity(entity).despawn();
    }
    let site = &runtime.site;
    let facility = &runtime.world;

    // One weave image per register for the whole rebuild, not one per hull.
    let mut weaves: BTreeMap<ArchitectureRegister, Option<Handle<Image>>> = BTreeMap::new();
    for piece in &facility.snapshot.pieces {
        let Some(mesh) = piece_mesh(piece) else {
            continue;
        };
        let register = facility
            .world
            .architecture
            .get(&piece.source_cell)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);
        let look = observed_style::hex_shell_surface(register, architecture_role(piece));
        let material = StandardMaterial {
            base_color: look.base_color,
            base_color_texture: if look.textured {
                weave(&mut images, &mut weaves, register)
            } else {
                None
            },
            // The authored emissive, as the game's own materials carry it. An
            // earlier attempt at the wash zeroed this; it did clear the wash,
            // but by deleting a district's identity rather than by fixing the
            // surface classification that was misapplying it.
            emissive: look.emissive,
            unlit: look.unlit,
            perceptual_roughness: observed_style::architecture(register).surface_roughness,
            ..default()
        };
        commands.spawn((
            FacilityVisual,
            CellVisual,
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
    for light in &facility.snapshot.lights {
        *per_cell.entry(light.source_cell).or_default() += 1;
    }
    for light in &facility.snapshot.lights {
        let register = facility
            .world
            .architecture
            .get(&light.source_cell)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);
        let practical = observed_style::hex_practical_light(
            register,
            composition(facility, light.source_cell),
            per_cell.get(&light.source_cell).copied().unwrap_or(1),
        );
        commands.spawn((
            FacilityVisual,
            CellVisual,
            Practical {
                at: light.position,
                shadows_allowed: practical.shadows_allowed,
            },
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

    // A fixture for any built cell the corpus left without one. The game does
    // this for the same reason and says so: "so a whole-room module with no
    // authored lights still has every part of its floor lit (Legibility
    // Contract)". Removing the directional light took away the guarantee that
    // darkness costs range and never legibility; this puts it back.
    for cell in &site.cells {
        if per_cell.contains_key(cell) {
            continue;
        }
        let register = facility
            .world
            .architecture
            .get(cell)
            .copied()
            .unwrap_or(ArchitectureRegister::Institutional);
        let practical =
            observed_style::hex_practical_light(register, composition(facility, *cell), 1);
        let at = Vec3::from_array(hex_origin(*cell)) + Vec3::Y * 3.2;
        commands.spawn((
            FacilityVisual,
            CellVisual,
            Practical {
                at,
                shadows_allowed: practical.shadows_allowed,
            },
            PointLight {
                color: practical.color,
                intensity: practical.intensity,
                range: practical.range,
                radius: practical.radius,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(at),
            Name::new("Fallback cell fill"),
        ));
    }

    // No sun. The facility has none: every photon comes from the district
    // ambient, the fog, the key and the authored practicals, and adding a
    // directional light is what made this floor look like an overcast
    // afternoon. `atmosphere` drives the rest each frame.
    commands.spawn((
        FacilityVisual,
        KeyLight,
        SpotLight::default(),
        Transform::default(),
        Name::new("district key light"),
    ));

    // Devices. These are lab equipment, not solver output, so they are drawn in
    // the kinetic legend's own colours rather than the district's.
    for (position, role, label) in [
        (site.generator, Role::Powered, "GENERATOR / E"),
        (site.station, Role::Powered, "RECHARGE STATION"),
        (site.panel, Role::Hazard, "DECOHERENCE / E"),
        (site.demolition, Role::Hazard, "RETRACT TILE / E"),
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
    guardians: Res<kinetic_lab::guardian::GuardianArt>,
    mut view: ResMut<ViewState>,
) {
    for (id, actor) in &runtime.world.actors {
        if view.actors.contains_key(id) {
            continue;
        }
        if actor.kind == Kind::Minor {
            // The shared Guardian rig, driven by this lab's simulation. It is
            // decorative and its feet stay inside the collision envelope, so
            // nothing about aim, motion or elimination changes.
            let entity = commands
                .spawn((
                    FacilityVisual,
                    ActorVisual(*id),
                    Transform::default(),
                    Visibility::default(),
                    Name::new(format!("Minor {}", id.0)),
                ))
                .id();
            kinetic_lab::guardian::spawn(&mut commands, &guardians, entity, id.0);
            view.actors.insert(*id, entity);
            continue;
        }
        let entity = commands
            .spawn((
                FacilityVisual,
                ActorVisual(*id),
                Mesh3d(art.cube.clone()),
                MeshMaterial3d(art.material(Role::Prop)),
                Transform::from_scale(Vec3::splat(0.9)),
                Name::new(format!("Crate {}", id.0)),
            ))
            .id();
        view.actors.insert(*id, entity);
    }
}

/// Pose the Guardian rigs from this lab's own actors.
fn animate_guardians(
    runtime: Res<Runtime>,
    mut rigs: kinetic_lab::guardian::Rigs,
    mut limbs: kinetic_lab::guardian::Limbs,
    mut lids: kinetic_lab::guardian::Lids,
) {
    let world = &runtime.world;
    kinetic_lab::guardian::animate_with(world.tick, &mut rigs, &mut limbs, &mut lids, |id| {
        let actor = world.actors.get(&ActorId(id))?;
        actor.alive.then(|| kinetic_lab::guardian::RigSample {
            velocity: world.velocity(actor.id),
            toward: world.player.position - world.pose(actor.id).position,
            grounded: actor.grounded,
            staggered: actor.stagger > 0,
        })
    });
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

fn sync(
    runtime: Res<Runtime>,
    art: Res<Art>,
    time: Res<Time<Fixed>>,
    mut bodies: ActorPoses,
    mut camera: EyePose,
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
        let frame = world.gravity.visual_frame();
        *transform = Transform::from_translation(frame.eye(&body, &world.player_config))
            .looking_to(frame.look(&body), frame.up());
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
    let Some((_, candidate)) = runtime.world.telegraph.as_ref() else {
        return;
    };
    // The tile that is about to stop existing, outlined in the hazard stripe on
    // a one-second blink. This is a Legibility Contract signal: it must read
    // from across the floor and through the fog.
    let role = if (runtime.world.tick / 10).is_multiple_of(2) {
        Role::Hazard
    } else {
        Role::Text
    };
    // Outline what is about to be re-collapsed. Looking at it is what saves it,
    // so the telegraph has to say exactly which cells are at stake.
    for cell in &candidate.region.cells {
        outline_cell(&mut gizmos, *cell, color(role), 0.2);
    }
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
    // Every plumbed body carries an arrow along its own down. This is a
    // Legibility Contract signal rather than a debug one: a body falling
    // sideways is otherwise indistinguishable from a body being thrown.
    for (id, actor) in &world.actors {
        let (Some(plumb), true) = (actor.lash, actor.alive) else {
            continue;
        };
        let at = world.pose(*id).position;
        let tip = at + plumb.direction * 1.8;
        gizmos.line(at, tip, color(Role::Pull));
        let side = plumb.direction.any_orthonormal_vector() * 0.3;
        gizmos.line(tip, tip - plumb.direction * 0.5 + side, color(Role::Pull));
        gizmos.line(tip, tip - plumb.direction * 0.5 - side, color(Role::Pull));
    }
    // The Observer's own artificial gravity, drawn along down when plumbed.
    // When expiring (<= 60 ticks), this pulses with Role::GravityWarning.
    if world.gravity.remaining > 0 {
        let down = -world.gravity.frame.up();
        let at = world.player.position;
        let role = if world.gravity.remaining <= 60 {
            if (world.tick / 6).is_multiple_of(2) {
                Role::GravityWarning
            } else {
                Role::Text
            }
        } else {
            Role::Pull
        };
        let tip = at + down * 1.5;
        gizmos.line(at, tip, color(role));
        let side = down.any_orthonormal_vector() * 0.25;
        gizmos.line(tip, tip - down * 0.4 + side, color(role));
        gizmos.line(tip, tip - down * 0.4 - side, color(role));

        // When expiring, also draw a reticle indicator in front of the eye
        // so the player cannot miss the warning when looking ahead.
        if world.gravity.remaining <= 60 {
            let visual = world.gravity.visual_frame();
            let look = visual.look(&world.player);
            let reticle = world.eye() + look * 1.1;
            let up = visual.up() * 0.08;
            let right = (visual.rotation * world.player.right()) * 0.08;
            gizmos.line(reticle - right, reticle + right, color(role));
            gizmos.line(reticle - up, reticle + up, color(role));
        }
    }
    // The armed direction, drawn just in front of the eye so the Observer can
    // see what they are about to commit without opening a menu.
    let muzzle = world.eye() + world.player.look_dir() * 1.4;
    gizmos.line(muzzle, muzzle + world.armed * 0.6, color(Role::Pull));

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

fn events(
    mut commands: Commands,
    mut runtime: ResMut<Runtime>,
    mut view: ResMut<ViewState>,
    bank: Res<kinetic_lab::sound::Bank>,
    mut sound: ResMut<kinetic_lab::sound::SoundState>,
) {
    let tick = runtime.world.tick;
    // The queue survives FixedUpdate catch-up, so no feedback is lost between
    // rendered frames.
    let queued = std::mem::take(&mut runtime.events);
    for event in queued {
        crate::sound::event(&mut commands, &bank, &mut sound, &runtime, &event);
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
            Event::Decohering(cell) => Some(format!(
                "({}, {}) decohering — look at it to hold it",
                cell.q, cell.r
            )),
            Event::Relaid(cell) => Some(format!("({}, {}) re-collapsed", cell.q, cell.r)),
            Event::Held => Some("the floor held — you were watching".to_string()),
            Event::Inert => Some("nothing here will move".to_string()),
            Event::Armed(direction) => Some(format!(
                "plumb armed  {:.2} {:.2} {:.2}",
                direction.x, direction.y, direction.z
            )),
            Event::Plumbed(..) => Some("plumb committed".to_string()),
            Event::SelfPlumbed => Some("self-plumb engaged — wall walk active".to_string()),
            Event::GravityReleased => Some("gravity released — returning upright".to_string()),
            Event::GravityWarning => Some("GRAVITY EXPIRING".to_string()),
            Event::Unplumbed(_) => None,
            Event::Retracted(cell) => {
                Some(format!("({}, {}) retracted toward void", cell.q, cell.r))
            }
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
        Refusal::Reorienting => "reorienting — wait for settle",
        Refusal::Clearance => "insufficient clearance to reorient",
    }
}

/// Cast shadows from the handful of fixtures nearest the Observer.
///
/// Every practical used to be shadowless, which left the key as the only
/// caster in the whole facility: every surface took the same flat
/// omnidirectional fill whatever way it faced, and the result was the milky
/// brown wash this lab had. Shadow maps are what give these authored hulls
/// value range, and they are also the expensive part — forty-nine fixtures is
/// too many to cast, and eight is plenty when only the ones around you can be
/// seen to cast at all. The game budgets them the same way.
fn practical_shadows(
    runtime: Res<Runtime>,
    mut last: Local<Option<(u32, u32, Option<observed_hex::HexCoord>)>>,
    mut practicals: Query<(&Practical, &mut PointLight)>,
) {
    const BUDGET: usize = 8;
    let cell = runtime.site.cell_containing(runtime.world.player.position);
    // Keyed on the same stamp the geometry rebuild uses. A retraction or a
    // relayout respawns every fixture shadowless without moving the Observer,
    // and a cell-only guard never notices.
    let stamp = (runtime.generation, runtime.world.geometry_generation, cell);
    if *last == Some(stamp) {
        return;
    }
    *last = Some(stamp);
    let eye = runtime.world.eye();
    let mut ranked: Vec<(f32, bool)> = practicals
        .iter()
        .map(|(practical, _)| {
            (
                practical.at.distance_squared(eye),
                practical.shadows_allowed,
            )
        })
        .collect();
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let cutoff = ranked
        .iter()
        .filter(|(_, allowed)| *allowed)
        .nth(BUDGET)
        .map_or(f32::MAX, |(distance, _)| *distance);
    for (practical, mut light) in &mut practicals {
        let wanted = practical.shadows_allowed && practical.at.distance_squared(eye) < cutoff;
        // Assign only on a change, so the steady state does not dirty every
        // light every time this runs.
        if light.shadow_maps_enabled != wanted {
            light.shadow_maps_enabled = wanted;
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// Ease ambient, fog and the key toward the district the Observer is standing
/// in, exactly as the game's shell does.
fn atmosphere(
    runtime: Res<Runtime>,
    view: Res<ViewState>,
    time: Res<Time>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut fog: Query<&mut DistanceFog, With<Eye>>,
    mut key: Query<(&mut SpotLight, &mut Transform), With<KeyLight>>,
    mut primed: Local<Option<(u32, u32)>>,
    mut inspecting: Local<Option<bool>>,
) {
    const BLEND_RATE: f32 = 2.0;
    /// How far inspection pushes the fog planes out.
    const INSPECT_FOG: f32 = 4.0;
    let world = &runtime.world;
    let Some(cell) = runtime.site.cell_containing(world.player.position) else {
        return;
    };
    let register = world
        .world
        .architecture
        .get(&cell)
        .copied()
        .unwrap_or(ArchitectureRegister::Institutional);
    let palette = observed_style::architecture_for_composition(register, composition(world, cell));
    // Initial state is not a transition. Easing in from a neutral grey makes
    // the first visible second of every floor the wrong district, so the first
    // frame of a run — and of each newly dealt floor — snaps to the target.
    // Including the geometry generation, because a relayout respawns the key as
    // a default white 1M-lumen spotlight: eased rather than snapped, it spends
    // a second climbing to the district's colour after every retraction.
    let stamp = (runtime.generation, runtime.world.geometry_generation);
    let fresh = *primed != Some(stamp) || *inspecting != Some(view.inspect);
    *primed = Some(stamp);
    *inspecting = Some(view.inspect);
    let blend = if fresh {
        1.0
    } else {
        (time.delta_secs() * BLEND_RATE).clamp(0.0, 1.0)
    };

    // Play lighting is tuned for a body standing inside a lit pool. Looking at
    // a whole floor at once is a different question and the codebase already
    // answers it: the game's overview swaps the district ambient for the
    // studio's fill and gives fog its own scale, "the same view of the same
    // building, so the same answer". This lab borrows both, on a key, rather
    // than adding a sun — a directional light is what flattened the districts
    // in the first place.
    ambient.color = lerp_color(ambient.color, palette.ambient_color, blend);
    ambient.brightness = if view.inspect {
        observed_style::iso::light::AMBIENT_BRIGHTNESS
    } else {
        lerp(ambient.brightness, palette.ambient_brightness, blend)
    };
    clear.0 = lerp_color(clear.0, palette.fog_color, blend);
    let (start, end) = if view.inspect {
        // Snapped, not eased: easing a fog plane across this much distance
        // leaves the view blank for the second it takes to arrive.
        (
            palette.fog_start * INSPECT_FOG,
            palette.fog_end * INSPECT_FOG,
        )
    } else {
        (palette.fog_start, palette.fog_end)
    };
    for mut fog in &mut fog {
        fog.color = lerp_color(fog.color, palette.fog_color, blend);
        fog.falloff = FogFalloff::Linear { start, end };
    }
    // The key hangs over the Observer's own cell, angled across it.
    let origin = Vec3::from_array(hex_origin(cell));
    let at = origin + Vec3::new(2.6, 6.4, 2.6);
    for (mut light, mut transform) in &mut key {
        light.color = lerp_color(light.color, palette.key_color, blend);
        light.intensity = lerp(
            light.intensity,
            palette.key_intensity * observed_style::HEX_KEY_INTENSITY_SCALE,
            blend,
        );
        light.range = palette.key_range;
        light.radius = palette.key_radius;
        light.inner_angle = palette.key_inner_angle;
        light.outer_angle = palette.key_outer_angle;
        light.shadow_maps_enabled = palette.key_shadows_enabled;
        *transform = Transform::from_translation(at)
            .looking_at(origin + Vec3::new(-1.0, 0.2, -1.0), Vec3::Y);
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    Color::srgb(
        lerp(a.red, b.red, t),
        lerp(a.green, b.green, t),
        lerp(a.blue, b.blue, t),
    )
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
                let gravity_summary = if world.gravity.remaining > 0 {
                    if world.gravity.remaining <= 60 {
                        format!(
                            "SELF-PLUMB  EXPIRING ({} ticks)  X releases",
                            world.gravity.remaining
                        )
                    } else {
                        format!(
                            "SELF-PLUMB  active ({} ticks)  X releases",
                            world.gravity.remaining
                        )
                    }
                } else if world.gravity.returning {
                    "SELF-PLUMB  returning upright".to_string()
                } else {
                    format!(
                        "SELF-PLUMB  {}   C self-plumbs",
                        match world.self_plumb_ready() {
                            Ok(_) => "ready".to_string(),
                            Err(reason) => refusal(reason).to_string(),
                        }
                    )
                };
                format!(
                    "SEED {}  ({} requested)\nPLAN {}\nCHARGE {}\nWAVE {} / 3    REMOVED {}\nGENERATOR {}\n\nFORCE {:.1} / {:.1}  [ ]\nMINOR SPEED {:.1}  - =\n\nPLUMB  down {:>5.2} {:>5.2} {:>5.2}   Q arms\n       {}\n{}",
                    site.seed,
                    site.requested_seed,
                    site.plan(),
                    charge,
                    world.wave,
                    world.kills,
                    if world.powered { "ON" } else { "OFF" },
                    world.config.push,
                    world.config.pull,
                    world.config.minor_speed,
                    world.armed.x,
                    world.armed.y,
                    world.armed.z,
                    match world.plumb_ready() {
                        Ok(_) => "ready".to_string(),
                        Err(reason) => refusal(reason).to_string(),
                    },
                    gravity_summary,
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
                if let Some(device) = world.interaction() {
                    lines.push(format!("E  {}", device.label()));
                }
                if world.gravity.remaining > 0 && world.gravity.remaining <= 60 {
                    lines.push(if (world.tick / 6).is_multiple_of(2) {
                        "GRAVITY EXPIRING".to_string()
                    } else {
                        format!("GRAVITY EXPIRING ({} TICKS)", world.gravity.remaining)
                    });
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
                        "tick {}  digest {:016x}\ngravity up {:>5.2} {:>5.2} {:>5.2}  rem {}  trans {}  ret {}\ncells {}  holes {}  thresholds onto void {}\nhulls {}  waypoints {}\npursuing {}  observing {}\nrelayouts {}  facility generation {}",
                        world.tick,
                        world.digest(),
                        world.gravity.frame.up().x,
                        world.gravity.frame.up().y,
                        world.gravity.frame.up().z,
                        world.gravity.remaining,
                        world.gravity.transition,
                        world.gravity.returning,
                        site.cells.len(),
                        world.holes().len(),
                        world.open_thresholds().len(),
                        world.snapshot.pieces.len(),
                        site.nav.len(),
                        pursuing,
                        world.observation().visible_cells.len(),
                        world.geometry_generation,
                        world.world.generation,
                    )
                }
            }
        };
    }
}

/// How a reader perceives a cell: a place, a way between places, or a joint
/// that changes floor. Style owns the light budget that follows from it.
fn composition(
    facility: &crate::model::WfcKineticWorld,
    cell: HexCoord,
) -> observed_style::HexComposition {
    let Some(placement) = facility.world.placements.get(&cell) else {
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

/// Which architectural surface a hull *is*, from its geometry.
///
/// This used to key off `HexStructureRole`, mapping Room, Hall and Shaft alike
/// to `Wall`, and that was the real cause of the floor looking washed out.
/// Shadow Screen's wall treatment carries an authored emissive of 1.30 — the
/// style crate notes it is roughly fourteen times the strongest structural glow
/// anywhere else, because in that district the wall *is* the lit surface — so
/// painting every floor, ceiling and column of a Shadow Screen cell with the
/// wall treatment self-illuminated the whole cell to one flat value. Ambient,
/// fog and shadows then changed almost nothing, which is exactly how it looked.
///
/// The game classifies by shape instead: low hulls are floor, high hulls are
/// ceiling, and a thin wide slab is a deck wherever it sits. A balcony is not
/// at floor level and is still something you walk on.
fn architecture_role(piece: &HexStructurePiece) -> ArchitectureSurfaceRole {
    let ColliderShape::ConvexHull { points } = &piece.shape else {
        return ArchitectureSurfaceRole::Wall;
    };
    let lowest = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let highest = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    if highest <= 0.75 {
        return ArchitectureSurfaceRole::Floor;
    }
    if lowest >= observed_hex::TILE_LEVEL_HEIGHT - 0.75
        || observed_traversal::render_mesh::is_overhead_slab(points)
    {
        return ArchitectureSurfaceRole::Ceiling;
    }
    if observed_traversal::render_mesh::is_horizontal_slab(points) {
        return ArchitectureSurfaceRole::Floor;
    }
    ArchitectureSurfaceRole::Wall
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
