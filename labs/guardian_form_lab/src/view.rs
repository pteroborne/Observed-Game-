//! Draws the candidates on a moonlit stage beside the existing minor Guardian, a
//! person and the facility's doorway, and runs the capture plan.
//!
//! Keys: `1`-`3` the tumbler with 3, 4 or 5 tiers, `4` the plumb, `5` the roller,
//! `L` all three side by side, `K` the tumbler's ranks; `H` hunting, `S` frozen by
//! sight, `A` frozen by an anchor, `C` a catch; `Tab` the camera; `P` pause; `R` reset.
use std::path::PathBuf;

use bevy::app::AppExit;
use bevy::camera::Hdr;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};
use observed_style::guardian::{self as style, Part as Finish};
use observed_style::kinetic::{Role, treatment};
use observed_style::open_air::{SkyRole, moon, sky, toward_moon};
use observed_style::{MarkerRole, marker};

use crate::capture::{self, Frame};
use crate::form::{self, Form, Look, Stage, State};
use crate::mesh::mesh;
use crate::sound;

/// What is on the stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Layout {
    /// One candidate, with the minor and the person beside it.
    Single(Form),
    /// All three candidates side by side, the minor and the person at the ends.
    Lineup,
    /// The tumbler at three, four and five tiers.
    Ranks,
}

/// Where the camera is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Framing {
    ThreeQuarter,
    /// Standing in front of it, looking up at the eye.
    Encounter,
    Lineup,
    Ranks,
}

impl Framing {
    fn eye(self) -> (Vec3, Vec3) {
        match self {
            Self::ThreeQuarter => (Vec3::new(5.4, 2.1, 8.4), Vec3::new(-0.3, 1.45, 0.0)),
            Self::Encounter => (Vec3::new(0.9, 1.6, 4.1), Vec3::new(0.0, 2.0, 0.0)),
            Self::Lineup => (Vec3::new(0.0, 2.8, 18.5), Vec3::new(0.0, 1.5, 0.0)),
            Self::Ranks => (Vec3::new(1.0, 2.4, 14.0), Vec3::new(0.8, 1.7, 0.0)),
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::ThreeQuarter => Self::Encounter,
            Self::Encounter => Self::Lineup,
            Self::Lineup => Self::Ranks,
            Self::Ranks => Self::ThreeQuarter,
        }
    }
}

/// Where the candidates stand for a layout.
fn slots(layout: Layout) -> Vec<(Form, Vec3)> {
    match layout {
        Layout::Single(form) => vec![(form, Vec3::ZERO)],
        Layout::Lineup => vec![
            (Form::Tumbler { tiers: 4 }, Vec3::new(-5.6, 0.0, 0.0)),
            (Form::Plumb, Vec3::ZERO),
            (Form::Roller, Vec3::new(5.6, 0.0, 0.0)),
        ],
        Layout::Ranks => vec![
            (Form::Tumbler { tiers: 3 }, Vec3::new(-4.4, 0.0, 0.0)),
            (Form::Tumbler { tiers: 4 }, Vec3::new(0.6, 0.0, 0.0)),
            (Form::Tumbler { tiers: 5 }, Vec3::new(6.2, 0.0, 0.0)),
        ],
    }
}

/// Where the minor Guardian and the person stand, for scale.
fn company(layout: Layout) -> (Option<Vec3>, Vec3) {
    match layout {
        Layout::Single(_) => (Some(Vec3::new(-3.3, 0.0, 1.8)), Vec3::new(3.3, 0.0, -1.2)),
        Layout::Lineup => (Some(Vec3::new(-9.8, 0.0, 1.0)), Vec3::new(9.6, 0.0, 1.0)),
        Layout::Ranks => (None, Vec3::new(10.0, 0.0, 1.0)),
    }
}

#[derive(Resource)]
struct Lab {
    layout: Layout,
    framing: Framing,
    state: State,
    t: f32,
    clock: f32,
    paused: bool,
    /// The layout on stage, to know when to rebuild it.
    built: Option<Layout>,
}

impl Default for Lab {
    fn default() -> Self {
        Self {
            layout: Layout::Single(Form::Tumbler { tiers: 4 }),
            framing: Framing::ThreeQuarter,
            state: State::Hunting,
            t: 0.0,
            clock: 0.0,
            paused: false,
            built: None,
        }
    }
}

#[derive(Resource)]
struct Capture {
    dir: PathBuf,
    plan: Vec<Frame>,
    next: usize,
}

#[derive(Resource)]
struct Looks {
    shell: Handle<StandardMaterial>,
    trim: Handle<StandardMaterial>,
    pupil: Handle<StandardMaterial>,
    eye: Handle<StandardMaterial>,
    eye_flare: Handle<StandardMaterial>,
    seam: Handle<StandardMaterial>,
    seam_dark: Handle<StandardMaterial>,
    seam_flare: Handle<StandardMaterial>,
    beam: Handle<StandardMaterial>,
    clamp: Handle<StandardMaterial>,
    stamp: Handle<StandardMaterial>,
}

/// Everything a layout spawns, cleared when it changes or the lab resets.
#[derive(Component)]
struct Actor;

#[derive(Component)]
struct GuardianRoot {
    form: Form,
    at: Vec3,
}

#[derive(Component)]
struct PartOf {
    index: usize,
    look: Look,
}

#[derive(Component)]
struct MinorRoot;

#[derive(Component)]
struct LabCamera;

#[derive(Component)]
struct Caption;

/// What the lab last sounded, to know what changed.
#[derive(Resource, Default)]
struct Heard {
    form: Option<Form>,
    state: Option<State>,
    clock: f32,
    t: f32,
}

#[derive(Component)]
struct Hum;

pub fn run() {
    let capture = std::env::var("OBSERVED2_CAPTURE").ok().map(|dir| Capture {
        dir: PathBuf::from(dir),
        plan: capture::plan(),
        next: 0,
    });
    let mut app = App::new();
    app.insert_resource(ClearColor(sky(SkyRole::Horizon)))
        .init_resource::<Lab>()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Observed 2 — Guardian Form Lab".to_string(),
                        resolution: WindowResolution::new(1440, 900),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                // The workspace's assets, where the guardian sounds are.
                .set(AssetPlugin {
                    file_path: format!("{}/../../assets", env!("CARGO_MANIFEST_DIR")),
                    ..default()
                }),
        )
        .init_resource::<Heard>()
        .add_systems(Startup, (setup, kinetic_lab::guardian::setup).chain())
        .add_systems(
            Update,
            (
                read_input,
                advance,
                drive_capture,
                rebuild,
                pose_guardians,
                pose_minor,
                place_camera,
                caption,
                play_sounds,
            )
                .chain(),
        );
    if let Some(capture) = capture {
        std::fs::create_dir_all(&capture.dir).expect("capture directory");
        app.insert_resource(capture);
    }
    app.run();
}

fn material(finish: Finish) -> StandardMaterial {
    let f = style::finish(finish);
    StandardMaterial {
        base_color: f.base_color,
        emissive: f.emissive,
        metallic: f.metallic,
        perceptual_roughness: f.roughness,
        ..default()
    }
}

fn glow(emissive: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.02, 0.01, 0.01),
        emissive,
        ..default()
    }
}

fn haze(color: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::LinearRgba(color),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        ..default()
    }
}

fn plain(role: Role) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment(role).base_color,
        perceptual_roughness: 0.85,
        ..default()
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut eye_flare = material(Finish::Eye);
    eye_flare.emissive = style::catch_flare();
    let control = marker(MarkerRole::Control);
    commands.insert_resource(Looks {
        shell: materials.add(material(Finish::Shell)),
        trim: materials.add(material(Finish::Trim)),
        pupil: materials.add(material(Finish::Pupil)),
        eye: materials.add(material(Finish::Eye)),
        eye_flare: materials.add(eye_flare),
        seam: materials.add(glow(style::seam(false))),
        seam_dark: materials.add(glow(style::seam(true))),
        seam_flare: materials.add(glow(style::catch_flare())),
        beam: materials.add(haze(style::beam())),
        clamp: materials.add(StandardMaterial {
            base_color: control.base_color,
            emissive: style::anchor_clamp(),
            ..default()
        }),
        stamp: materials.add(haze(style::catch_flare() * 0.12)),
    });

    let (eye, target) = Framing::ThreeQuarter.eye();
    commands.spawn((
        LabCamera,
        Camera3d::default(),
        Hdr,
        Bloom {
            intensity: 0.12,
            ..Bloom::NATURAL
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 50f32.to_radians(),
            ..default()
        }),
        DistanceFog {
            color: sky(SkyRole::Horizon),
            falloff: FogFalloff::Linear {
                start: 24.0,
                end: 90.0,
            },
            ..default()
        },
        Transform::from_translation(eye).looking_at(target, Vec3::Y),
    ));
    // The facility's light: the moon from the west-south-west, and a warm district key.
    commands.spawn((
        DirectionalLight {
            color: moon(),
            illuminance: 8_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 2,
            maximum_distance: 50.0,
            first_cascade_far_bound: 16.0,
            ..default()
        }
        .build(),
        Transform::from_translation(Vec3::from_array(toward_moon()))
            .looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.78, 0.52),
            intensity: 3_000_000.0,
            range: 40.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-4.0, 7.0, 6.0),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: moon(),
        brightness: 70.0,
        ..default()
    });

    // The deck: facility cells, edge to edge, over a darker slab that shows as seams.
    let tile = meshes.add(
        Extrusion::new(RegularPolygon::new(7.93, 6), 0.3)
            .mesh()
            .build()
            .rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    );
    let floor = materials.add(plain(Role::Catwalk));
    for (q, r) in [
        (0, 0),
        (1, 0),
        (-1, 0),
        (0, 1),
        (-1, 1),
        (0, -1),
        (1, -1),
        (2, -1),
        (-2, 1),
    ] {
        #[allow(clippy::cast_precision_loss)]
        let at = Vec3::new(q as f32 * 14.0 + r as f32 * 7.0, -0.15, r as f32 * 12.0);
        commands.spawn((
            Mesh3d(tile.clone()),
            MeshMaterial3d(floor.clone()),
            Transform::from_translation(at),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(120.0, 120.0))),
        MeshMaterial3d(materials.add(plain(Role::Panel))),
        Transform::from_xyz(0.0, -0.2, 0.0),
    ));
    // The back wall and its doorway: 4.5 m wide, 4 m clear, as the tiles author it.
    let wall = materials.add(plain(Role::Wall));
    let block = |w: f32, h: f32| Cuboid::new(w, h, 0.5);
    for (size, at) in [
        ((10.0, 8.0), Vec3::new(-7.25, 4.0, -4.5)),
        ((10.0, 8.0), Vec3::new(7.25, 4.0, -4.5)),
        ((4.5, 4.0), Vec3::new(0.0, 6.0, -4.5)),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(block(size.0, size.1))),
            MeshMaterial3d(wall.clone()),
            Transform::from_translation(at),
        ));
    }
    commands.spawn((
        Caption,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.88, 0.92)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(20.0),
            bottom: Val::Px(16.0),
            ..default()
        },
    ));
}

fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut lab: ResMut<Lab>,
    capture: Option<Res<Capture>>,
) {
    if capture.is_some() {
        return;
    }
    let layout = [
        (KeyCode::Digit1, Layout::Single(Form::Tumbler { tiers: 3 })),
        (KeyCode::Digit2, Layout::Single(Form::Tumbler { tiers: 4 })),
        (KeyCode::Digit3, Layout::Single(Form::Tumbler { tiers: 5 })),
        (KeyCode::Digit4, Layout::Single(Form::Plumb)),
        (KeyCode::Digit5, Layout::Single(Form::Roller)),
        (KeyCode::KeyL, Layout::Lineup),
        (KeyCode::KeyK, Layout::Ranks),
    ];
    for (key, to) in layout {
        if keys.just_pressed(key) {
            lab.layout = to;
            lab.framing = match to {
                Layout::Single(_) => Framing::ThreeQuarter,
                Layout::Lineup => Framing::Lineup,
                Layout::Ranks => Framing::Ranks,
            };
        }
    }
    for (key, state) in [
        (KeyCode::KeyH, State::Hunting),
        (KeyCode::KeyS, State::FrozenBySight),
        (KeyCode::KeyA, State::FrozenByAnchor),
        (KeyCode::KeyC, State::Catch),
    ] {
        if keys.just_pressed(key) && lab.state != state {
            lab.state = state;
            lab.t = 0.0;
        }
    }
    if keys.just_pressed(KeyCode::Tab) {
        lab.framing = lab.framing.next();
    }
    if keys.just_pressed(KeyCode::KeyP) {
        lab.paused = !lab.paused;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let layout = lab.layout;
        *lab = Lab {
            layout,
            ..Lab::default()
        };
    }
}

fn advance(time: Res<Time>, mut lab: ResMut<Lab>, capture: Option<Res<Capture>>) {
    if capture.is_some() || lab.paused {
        return;
    }
    let dt = time.delta_secs();
    lab.clock += dt;
    lab.t += dt;
    // A catch plays and plays again, as the capture shows it.
    if lab.state == State::Catch && lab.t > form::CATCH_SECONDS {
        lab.t = 0.0;
    }
}

/// Set this frame from the plan, and write it if the plan says to.
fn drive_capture(
    mut commands: Commands,
    capture: Option<ResMut<Capture>>,
    mut lab: ResMut<Lab>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut capture) = capture else {
        return;
    };
    let Some(frame) = capture.plan.get(capture.next).cloned() else {
        exit.write(AppExit::Success);
        return;
    };
    capture.next += 1;
    lab.layout = frame.layout;
    lab.framing = frame.framing;
    lab.state = frame.state;
    lab.t = frame.t;
    lab.clock = frame.clock;
    if let Some(output) = frame.output {
        let path = capture.dir.join(output);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("capture subdirectory");
        }
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

fn rebuild(
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    looks: Res<Looks>,
    art: Option<Res<kinetic_lab::guardian::GuardianArt>>,
    actors: Query<Entity, With<Actor>>,
) {
    if lab.built == Some(lab.layout) {
        return;
    }
    let Some(art) = art else { return };
    for entity in &actors {
        commands.entity(entity).despawn();
    }
    lab.built = Some(lab.layout);
    for (form, at) in slots(lab.layout) {
        let root = commands
            .spawn((
                Actor,
                GuardianRoot { form, at },
                Transform::IDENTITY,
                Visibility::default(),
            ))
            .id();
        for (index, part) in form::parts(form).into_iter().enumerate() {
            let material = match part.look {
                Look::Shell => looks.shell.clone(),
                Look::Trim => looks.trim.clone(),
                Look::Seam => looks.seam.clone(),
                Look::Eye => looks.eye.clone(),
                Look::Pupil => looks.pupil.clone(),
                Look::Beam => looks.beam.clone(),
                Look::Clamp => looks.clamp.clone(),
                Look::Stamp => looks.stamp.clone(),
            };
            let mut entity = commands.spawn((
                PartOf {
                    index,
                    look: part.look,
                },
                Mesh3d(meshes.add(mesh(part.shape))),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                Visibility::Hidden,
                ChildOf(root),
            ));
            if matches!(part.look, Look::Beam | Look::Stamp) {
                entity.insert((NotShadowCaster, NotShadowReceiver));
            }
        }
    }
    let (minor, person) = company(lab.layout);
    if let Some(at) = minor {
        // The rig stands on legs reaching about half a metre below its root.
        let root = commands
            .spawn((
                Actor,
                MinorRoot,
                Transform::from_translation(at + Vec3::Y * 0.52),
                Visibility::default(),
            ))
            .id();
        kinetic_lab::guardian::spawn(&mut commands, &art, root, 0);
    }
    // A person, for scale: 1.8 m, dark, with the "you" colour at eye height.
    let body = materials.add(plain(Role::Panel));
    let you = marker(MarkerRole::You);
    let visor = materials.add(StandardMaterial {
        base_color: you.base_color,
        emissive: you.emissive * 0.4,
        ..default()
    });
    commands.spawn((
        Actor,
        Mesh3d(meshes.add(Capsule3d::new(0.26, 1.28))),
        MeshMaterial3d(body),
        Transform::from_translation(person + Vec3::Y * 0.9),
    ));
    commands.spawn((
        Actor,
        Mesh3d(meshes.add(Cuboid::new(0.34, 0.07, 0.2))),
        MeshMaterial3d(visor),
        Transform::from_translation(person + Vec3::new(0.0, 1.62, 0.12)),
    ));
}

fn pose_guardians(
    lab: Res<Lab>,
    looks: Res<Looks>,
    roots: Query<(&GuardianRoot, &Children)>,
    mut parts: Query<(
        &PartOf,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    // They look at the camera: whoever is watching this stage.
    let toward = lab.framing.eye().0;
    for (root, children) in &roots {
        let pose = form::pose(
            root.form,
            lab.state,
            lab.t,
            lab.clock,
            Stage {
                at: root.at,
                toward,
            },
        );
        for child in children.iter() {
            let Ok((part, mut transform, mut visibility, mut material)) = parts.get_mut(child)
            else {
                continue;
            };
            match pose.parts.get(part.index).copied().flatten() {
                Some(at) => {
                    *transform = at;
                    *visibility = Visibility::Inherited;
                }
                None => *visibility = Visibility::Hidden,
            }
            let wanted = match part.look {
                Look::Seam if pose.flare > 0.05 => Some(&looks.seam_flare),
                Look::Seam if pose.seams_dark => Some(&looks.seam_dark),
                Look::Seam => Some(&looks.seam),
                Look::Eye if pose.flare > 0.05 => Some(&looks.eye_flare),
                Look::Eye => Some(&looks.eye),
                _ => None,
            };
            if let Some(wanted) = wanted
                && material.0 != *wanted
            {
                material.0 = wanted.clone();
            }
        }
    }
}

fn pose_minor(
    lab: Res<Lab>,
    mut rigs: kinetic_lab::guardian::Rigs,
    mut limbs: kinetic_lab::guardian::Limbs,
    mut lids: kinetic_lab::guardian::Lids,
) {
    // Where things are comes from the layout and the framing, not from other entities'
    // transforms, which the rig's own queries write.
    let Some(at) = company(lab.layout).0 else {
        return;
    };
    let toward = lab.framing.eye().0 - at;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let tick = (lab.clock * 60.0) as u64;
    kinetic_lab::guardian::animate_with(tick, &mut rigs, &mut limbs, &mut lids, |_| {
        Some(kinetic_lab::guardian::RigSample {
            velocity: Vec3::ZERO,
            toward,
            grounded: true,
            staggered: false,
        })
    });
}

fn place_camera(lab: Res<Lab>, mut camera: Query<&mut Transform, With<LabCamera>>) {
    if let Ok(mut transform) = camera.single_mut() {
        let (eye, target) = lab.framing.eye();
        *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}

fn caption(lab: Res<Lab>, mut text: Query<&mut Text, With<Caption>>) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let what = match lab.layout {
        Layout::Single(form) => form.name().to_uppercase().replace('_', " "),
        Layout::Lineup => "TUMBLER 4 / PLUMB / ROLLER".into(),
        Layout::Ranks => "TUMBLER RANKS 3 / 4 / 5".into(),
    };
    let company = match lab.layout {
        Layout::Ranks => "person 1.8 m / doorway 4.5 x 4 m",
        _ => "existing minor Guardian / person 1.8 m / doorway 4.5 x 4 m",
    };
    let state = lab.state.name().replace('_', " ");
    text.0 = format!("{what} - {state}\n{company}");
}

/// Sound the candidate on stage: its hum or its steps while hunting, and a one-shot on
/// entering a state. Silent in capture, whose films are mixed afterwards.
fn play_sounds(
    mut commands: Commands,
    lab: Res<Lab>,
    capture: Option<Res<Capture>>,
    assets: Res<AssetServer>,
    mut heard: ResMut<Heard>,
    hums: Query<Entity, With<Hum>>,
) {
    if capture.is_some() {
        return;
    }
    let one_shot = |commands: &mut Commands, name: &str| {
        commands.spawn((
            Actor,
            AudioPlayer::<AudioSource>(assets.load(sound::path(name))),
            PlaybackSettings::DESPAWN,
        ));
    };
    let Layout::Single(form) = lab.layout else {
        for entity in &hums {
            commands.entity(entity).despawn();
        }
        *heard = Heard::default();
        return;
    };
    let voice = sound::voice(form);
    let new_form = heard.form != Some(form);
    if new_form || heard.state != Some(lab.state) {
        if let (Some(from), false) = (heard.state, new_form)
            && let Some(name) = sound::on_entering(form, from, lab.state)
        {
            one_shot(&mut commands, name);
        }
        for entity in &hums {
            commands.entity(entity).despawn();
        }
        if lab.state == State::Hunting
            && let Some(hum) = voice.hum
        {
            commands.spawn((
                Actor,
                Hum,
                AudioPlayer::<AudioSource>(assets.load(sound::path(hum))),
                PlaybackSettings::LOOP,
            ));
        }
    } else if lab.state == State::Hunting
        && let Some(step) = voice.step
        && sound::landings(heard.clock, lab.clock) > 0
    {
        one_shot(&mut commands, step);
    } else if lab.state == State::Catch && lab.t < heard.t {
        // The catch plays again.
        one_shot(&mut commands, voice.catch);
    }
    heard.form = Some(form);
    heard.state = Some(lab.state);
    heard.clock = lab.clock;
    heard.t = lab.t;
}
