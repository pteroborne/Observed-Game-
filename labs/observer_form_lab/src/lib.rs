//! The Observer's floating eye on a moonlit stage: one in each role it can wear,
//! looking about, beside the major Guardian it shares the facility with.
//!
//! Keys: `Tab` the camera; `C` every eye looks at the camera; `P` pause; `R` reset.
//!
//! `OBSERVED2_CAPTURE=<dir>` writes the stills and a short film (see [`plan`]). Poses
//! are pure functions of the clock, so every frame is exactly what the plan says,
//! however fast the machine renders.
use std::path::PathBuf;

use bevy::app::AppExit;
use bevy::camera::Hdr;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};
use observed_guardian::form::{self as guardian, Stage, State};
use observed_observer::form::{self, Gaze, Look};
use observed_style::kinetic::{Role, treatment};
use observed_style::observer::{self as style, Part};
use observed_style::open_air::{SkyRole, moon, sky, toward_moon};
use observed_style::{MarkerRole, guardian::Finish};

/// A body's centre above its feet: the eye floats at 1.6 m, where its player sees.
const BODY_CENTRE: f32 = 0.9;
/// The eyes on the stage: where each body stands, and whose eye it is.
const EYES: [(f32, MarkerRole); 3] = [
    (-2.0, MarkerRole::Rival),
    (0.0, MarkerRole::You),
    (2.0, MarkerRole::Teammate),
];
/// The major Guardian, off to one side.
const GUARDIAN_AT: Vec3 = Vec3::new(5.6, 0.0, -1.2);

/// Where the camera is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Framing {
    /// All three eyes and the Guardian.
    Lineup,
    /// Close on the middle eye.
    Close,
    /// Low, from beside, so the gimbal and the hover ring read.
    Profile,
}

impl Framing {
    fn eye(self) -> (Vec3, Vec3) {
        match self {
            Self::Lineup => (Vec3::new(1.6, 2.2, 8.6), Vec3::new(1.4, 1.3, 0.0)),
            Self::Close => (Vec3::new(0.35, 1.7, 1.7), Vec3::new(0.0, 1.58, 0.0)),
            Self::Profile => (Vec3::new(2.6, 1.15, 1.6), Vec3::new(0.0, 1.3, 0.0)),
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Lineup => Self::Close,
            Self::Close => Self::Profile,
            Self::Profile => Self::Lineup,
        }
    }
}

/// What the eyes are doing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gazing {
    /// Each looks about on its own.
    About,
    /// Every eye on the camera.
    AtCamera,
}

/// One frame of the capture plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub framing: Framing,
    pub gazing: Gazing,
    pub clock: f32,
    /// Where to write it, under the capture directory.
    pub output: Option<String>,
}

/// Frames before anything is written, while the renderer compiles its pipelines:
/// at 90 the first two stills had the Guardian and none of the eyes, floor or wall,
/// whose pipelines were still compiling.
const WARM_UP: usize = 300;
/// Frames after the last, so its screenshot is saved before the app exits.
const TAIL: usize = 10;
/// Film frame rate.
const FPS: f32 = 30.0;

/// When the middle eye is shut mid-blink, for the blink still.
fn blink_time() -> f32 {
    let seed = 1;
    (0..60 * 20)
        .map(|frame| {
            #[allow(clippy::cast_precision_loss)]
            let clock = 4.0 + frame as f32 / 60.0;
            clock
        })
        .min_by(|a, b| form::openness(*a, seed).total_cmp(&form::openness(*b, seed)))
        .expect("a blink in twenty seconds")
}

/// Stills of each framing, a blink, and eight seconds of the line-up looking about.
#[must_use]
pub fn plan() -> Vec<Frame> {
    let frame = |framing, gazing, clock, output: Option<&str>| Frame {
        framing,
        gazing,
        clock,
        output: output.map(str::to_owned),
    };
    let mut plan: Vec<Frame> = (0..WARM_UP)
        .map(|_| frame(Framing::Lineup, Gazing::AtCamera, 2.0, None))
        .collect();
    let stills = [
        (Framing::Lineup, Gazing::AtCamera, 2.0, "lineup.png"),
        (Framing::Lineup, Gazing::About, 3.1, "lineup_about.png"),
        (Framing::Close, Gazing::AtCamera, 2.0, "close.png"),
        (Framing::Profile, Gazing::About, 2.6, "profile.png"),
        (Framing::Close, Gazing::AtCamera, blink_time(), "blink.png"),
    ];
    for (framing, gazing, clock, name) in stills {
        // Settle each framing for a few frames, then take it.
        plan.extend((0..6).map(|_| frame(framing, gazing, clock, None)));
        plan.push(frame(framing, gazing, clock, Some(name)));
    }
    plan.extend((0..6).map(|_| frame(Framing::Lineup, Gazing::About, 0.0, None)));
    for index in 0..240 {
        #[allow(clippy::cast_precision_loss)]
        let clock = index as f32 / FPS;
        let name = format!("frames/film_{index:04}.png");
        plan.push(frame(Framing::Lineup, Gazing::About, clock, Some(&name)));
    }
    plan.extend((0..TAIL).map(|_| frame(Framing::Lineup, Gazing::About, 8.0, None)));
    plan
}

/// How an eye looks about on its own: a slow, wandering heading and a little pitch,
/// different for each.
fn wander(clock: f32, index: usize) -> Gaze {
    #[allow(clippy::cast_precision_loss)]
    let i = index as f32;
    Gaze {
        yaw: 0.9 * (clock * 0.45 + i * 2.1).sin() + 0.35 * (clock * 1.3 + i).sin(),
        pitch: 0.28 * (clock * 0.37 + i * 1.7).sin(),
    }
}

/// The gaze that looks from `from` at `to`.
fn toward(from: Vec3, to: Vec3) -> Gaze {
    let d = (to - from).normalize_or_zero();
    Gaze {
        yaw: d.x.atan2(-d.z),
        pitch: d.y.clamp(-1.0, 1.0).asin(),
    }
}

#[derive(Resource)]
struct Lab {
    framing: Framing,
    gazing: Gazing,
    clock: f32,
    paused: bool,
}

impl Default for Lab {
    fn default() -> Self {
        Self {
            framing: Framing::Lineup,
            gazing: Gazing::About,
            clock: 0.0,
            paused: false,
        }
    }
}

#[derive(Resource)]
struct Capture {
    dir: PathBuf,
    plan: Vec<Frame>,
    next: usize,
}

/// One eye's root, at its body's centre.
#[derive(Component)]
struct EyeRoot {
    index: usize,
    seed: u32,
}

#[derive(Component)]
struct EyePart(usize);

#[derive(Component)]
struct GuardianPart(usize);

#[derive(Component)]
struct LabCamera;

#[derive(Component)]
struct Caption;

pub fn run() {
    let capture = std::env::var("OBSERVED2_CAPTURE").ok().map(|dir| Capture {
        dir: PathBuf::from(dir),
        plan: plan(),
        next: 0,
    });
    let mut app = App::new();
    app.insert_resource(ClearColor(sky(SkyRole::Horizon)))
        .init_resource::<Lab>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Observer Form Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                read_input,
                advance,
                drive_capture,
                pose_eyes,
                pose_guardian,
                place_camera,
                caption,
            )
                .chain(),
        );
    if let Some(capture) = capture {
        std::fs::create_dir_all(capture.dir.join("frames")).expect("capture directory");
        app.insert_resource(capture);
    }
    app.run();
}

fn material(finish: Finish) -> StandardMaterial {
    StandardMaterial {
        base_color: finish.base_color,
        emissive: finish.emissive,
        metallic: finish.metallic,
        perceptual_roughness: finish.roughness,
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
    let (eye, target) = Framing::Lineup.eye();
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
    // The facility's light, as the Guardian lab stages it: the moon and a warm key.
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

    // The deck and the back wall with the facility's doorway: 4.5 m wide, 4 m clear.
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(120.0, 120.0))),
        MeshMaterial3d(materials.add(plain(Role::Catwalk))),
    ));
    let wall = materials.add(plain(Role::Wall));
    for (size, at) in [
        ((10.0, 8.0), Vec3::new(-7.25, 4.0, -4.5)),
        ((10.0, 8.0), Vec3::new(7.25, 4.0, -4.5)),
        ((4.5, 4.0), Vec3::new(0.0, 6.0, -4.5)),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.0, size.1, 0.5))),
            MeshMaterial3d(wall.clone()),
            Transform::from_translation(at),
        ));
    }

    // The eyes.
    let globe = materials.add(material(style::finish(Part::Globe)));
    let trim = materials.add(material(style::finish(Part::Trim)));
    let pupil = materials.add(material(style::finish(Part::Pupil)));
    let parts = form::parts();
    let part_meshes: Vec<_> = parts
        .iter()
        .map(|part| meshes.add(observed_observer::mesh::mesh(part.shape)))
        .collect();
    for (index, &(x, role)) in EYES.iter().enumerate() {
        let iris = materials.add(material(style::finish(Part::Iris(role))));
        let ring = materials.add(haze(style::haze(role)));
        commands
            .spawn((
                EyeRoot {
                    index,
                    seed: u32::try_from(index).expect("three eyes"),
                },
                Transform::from_xyz(x, BODY_CENTRE, 0.0),
                Visibility::default(),
            ))
            .with_children(|root| {
                for (k, part) in parts.iter().enumerate() {
                    let look = match part.look {
                        Look::Globe => globe.clone(),
                        Look::Trim => trim.clone(),
                        Look::Iris => iris.clone(),
                        Look::Pupil => pupil.clone(),
                        Look::Haze => ring.clone(),
                    };
                    let mut part_entity = root.spawn((
                        EyePart(k),
                        Mesh3d(part_meshes[k].clone()),
                        MeshMaterial3d(look),
                        Transform::default(),
                    ));
                    if part.look == Look::Haze {
                        part_entity.insert(NotShadowCaster);
                    }
                }
            });
    }

    // The major Guardian, hunting, for scale and for the silhouette it must not share.
    let guardian_material = |look: guardian::Look| -> StandardMaterial {
        use observed_style::guardian::{self as g, Part as G};
        match look {
            guardian::Look::Shell => material(g::finish(G::Shell)),
            guardian::Look::Trim => material(g::finish(G::Trim)),
            guardian::Look::Eye => material(g::finish(G::Eye)),
            guardian::Look::Pupil => material(g::finish(G::Pupil)),
            guardian::Look::Seam => StandardMaterial {
                base_color: Color::srgb(0.02, 0.01, 0.01),
                emissive: g::seam(false),
                ..default()
            },
            guardian::Look::Beam | guardian::Look::Clamp | guardian::Look::Stamp => haze(g::beam()),
        }
    };
    for (k, part) in guardian::parts(guardian::Form::Tumbler { tiers: 4 })
        .iter()
        .enumerate()
    {
        commands.spawn((
            GuardianPart(k),
            Mesh3d(meshes.add(observed_guardian::mesh::mesh(part.shape))),
            MeshMaterial3d(materials.add(guardian_material(part.look))),
            Transform::default(),
            Visibility::Hidden,
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
    if keys.just_pressed(KeyCode::Tab) {
        lab.framing = lab.framing.next();
    }
    if keys.just_pressed(KeyCode::KeyC) {
        lab.gazing = match lab.gazing {
            Gazing::About => Gazing::AtCamera,
            Gazing::AtCamera => Gazing::About,
        };
    }
    if keys.just_pressed(KeyCode::KeyP) {
        lab.paused = !lab.paused;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        *lab = Lab::default();
    }
}

fn advance(time: Res<Time>, mut lab: ResMut<Lab>, capture: Option<Res<Capture>>) {
    if capture.is_none() && !lab.paused {
        lab.clock += time.delta_secs();
    }
}

fn drive_capture(
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    capture: Option<ResMut<Capture>>,
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
    lab.framing = frame.framing;
    lab.gazing = frame.gazing;
    lab.clock = frame.clock;
    if let Some(output) = frame.output {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(capture.dir.join(output)));
    }
}

fn pose_eyes(
    lab: Res<Lab>,
    roots: Query<(&EyeRoot, &Transform, &Children), Without<EyePart>>,
    mut parts: Query<(&EyePart, &mut Transform), Without<EyeRoot>>,
) {
    // The framing's own eye, not the camera's transform: stills are taken the frame
    // a framing is set, before the camera has been moved there.
    let camera = lab.framing.eye().0;
    for (root, root_transform, children) in &roots {
        let centre = root_transform.translation + Vec3::Y * form::EYE_RISE;
        let gaze = match lab.gazing {
            Gazing::About => wander(lab.clock, root.index),
            Gazing::AtCamera => toward(centre, camera),
        };
        let placed = form::pose(gaze, lab.clock, root.seed);
        for child in children.iter() {
            if let Ok((part, mut transform)) = parts.get_mut(child) {
                *transform = placed[part.0];
            }
        }
    }
}

fn pose_guardian(
    lab: Res<Lab>,
    mut parts: Query<(&GuardianPart, &mut Transform, &mut Visibility)>,
) {
    let stage = Stage {
        at: GUARDIAN_AT,
        toward: Vec3::new(0.0, 1.6, 0.0),
    };
    let pose = guardian::pose(
        guardian::Form::Tumbler { tiers: 4 },
        State::Hunting,
        lab.clock,
        lab.clock,
        stage,
    );
    for (part, mut transform, mut visibility) in &mut parts {
        match pose.parts.get(part.0).copied().flatten() {
            Some(placed) => {
                *transform = placed;
                *visibility = Visibility::Inherited;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

fn place_camera(lab: Res<Lab>, mut camera: Query<&mut Transform, With<LabCamera>>) {
    let (eye, target) = lab.framing.eye();
    for mut transform in &mut camera {
        *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}

fn caption(lab: Res<Lab>, mut text: Query<&mut Text, With<Caption>>) {
    let gazing = match lab.gazing {
        Gazing::About => "looking about",
        Gazing::AtCamera => "looking at you",
    };
    for mut text in &mut text {
        **text = format!(
            "Observer: floating eye   rival  /  you  /  teammate, beside the major Guardian   \
             {gazing}   [{:?}]",
            lab.framing
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_takes_every_still_once() {
        let plan = plan();
        let outputs: Vec<_> = plan
            .iter()
            .filter_map(|frame| frame.output.clone())
            .collect();
        for still in [
            "lineup.png",
            "lineup_about.png",
            "close.png",
            "profile.png",
            "blink.png",
        ] {
            assert_eq!(outputs.iter().filter(|o| *o == still).count(), 1, "{still}");
        }
        assert_eq!(
            outputs.iter().filter(|o| o.starts_with("frames/")).count(),
            240
        );
    }

    #[test]
    fn the_blink_still_is_taken_mid_blink() {
        assert!(form::openness(blink_time(), 1) < 0.1);
    }

    #[test]
    fn looking_at_the_camera_points_the_pupil_at_it() {
        let from = Vec3::new(2.0, BODY_CENTRE + form::EYE_RISE, 0.0);
        let camera = Framing::Close.eye().0;
        let gaze = toward(from, camera);
        assert!(gaze.forward().dot((camera - from).normalize()) > 0.999);
    }
}
