//! Kinetic Tool Lab: what the Observer's kinetic tool could look like.
//!
//! Three candidate designs (`observed_tool`) on turntables side by side, and the one
//! selected held in a first-person hand over a facility floor. Each cycles through what
//! the tool does, idle, push, pull and lash, while its armed direction sweeps round,
//! because the armed lash turns with the Observer and the tool is where it shows.
//!
//! Keys: `1` the lineup, `2` held; `Tab` the next design; `Space` push, `E` pull,
//! `F` lash; `Up`/`Down` the armed pitch, `Left`/`Right` its yaw; `A` automatic cycle on
//! or off; `P` pause.
//! `OBSERVED2_CAPTURE=<dir>` writes the stills and exits.
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::path::PathBuf;

use bevy::app::AppExit;
use bevy::camera::Hdr;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};
use observed_guardian::mesh::mesh;
use observed_style::equipment::{Hardware, finish};
use observed_style::kinetic::{Role, treatment};
use observed_style::open_air::{SkyRole, moon, sky, toward_moon};
use observed_tool::{Beat, Design, FIRE_SECONDS, Finish, ToolState, parts, pose};

/// The tools are shown at this multiple of life size on the turntables, so a 30 cm
/// instrument can be read from across a lineup.
const TURNTABLE_SCALE: f32 = 4.0;
/// Where a right hand holds a tool in the first-person view, from the eye: the game's
/// hand for the observation torch (`game/src/hex_wfc/lantern/torch.rs`), so a candidate
/// is judged at the size it would be held at in play.
const HELD_AT: Vec3 = Vec3::new(0.13, -0.095, -0.20);
const HELD_ROLL: f32 = 0.16;
const HELD_SCALE: f32 = 0.40;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum View {
    Lineup,
    Held,
}

#[derive(Resource)]
struct Lab {
    view: View,
    design: Design,
    beat: Beat,
    since: f32,
    armed_pitch: f32,
    armed_yaw: f32,
    clock: f32,
    auto: bool,
    paused: bool,
    built: Option<(View, Design)>,
}

impl Default for Lab {
    fn default() -> Self {
        Self {
            view: View::Lineup,
            design: Design::Coil,
            beat: Beat::Idle,
            since: 10.0,
            armed_pitch: 0.4,
            armed_yaw: 0.0,
            clock: 0.0,
            auto: true,
            paused: false,
            built: None,
        }
    }
}

impl Lab {
    fn state(&self) -> ToolState {
        ToolState {
            beat: self.beat,
            since: self.since,
            armed_pitch: self.armed_pitch,
            armed_yaw: self.armed_yaw,
            charge: 0.8,
        }
    }

    fn fire(&mut self, beat: Beat) {
        self.beat = beat;
        self.since = 0.0;
    }
}

/// One still: the view, the design, what it is doing, and where the clock is.
#[derive(Clone, Copy)]
struct Frame {
    view: View,
    design: Design,
    beat: Beat,
    since: f32,
    armed_pitch: f32,
    armed_yaw: f32,
    clock: f32,
    output: Option<&'static str>,
}

fn plan() -> Vec<Frame> {
    let settle = |view, design| Frame {
        view,
        design,
        beat: Beat::Idle,
        since: 10.0,
        armed_pitch: 0.4,
        armed_yaw: 0.0,
        clock: 1.0,
        output: None,
    };
    let shot = |view, design, beat, armed_pitch, output| Frame {
        view,
        design,
        beat,
        since: FIRE_SECONDS * 0.15,
        armed_pitch,
        armed_yaw: 0.0,
        clock: 1.3,
        output: Some(output),
    };
    // The first frames compile every pipeline, which a software renderer takes a while
    // over; a still taken before then is empty.
    let mut frames = vec![settle(View::Lineup, Design::Coil); 90];
    // A few frames to settle each layout before the shot: meshes, shadows, bloom.
    let mut take = |frame: Frame| {
        frames.extend(std::iter::repeat_n(settle(frame.view, frame.design), 6));
        frames.push(frame);
    };
    take(shot(
        View::Lineup,
        Design::Coil,
        Beat::Idle,
        0.4,
        "lineup.png",
    ));
    take(shot(
        View::Lineup,
        Design::Coil,
        Beat::Push,
        0.4,
        "lineup-push.png",
    ));
    for (design, name) in [
        (Design::Coil, "held-coil.png"),
        (Design::Plumb, "held-plumb.png"),
        (Design::Lance, "held-lance.png"),
    ] {
        take(shot(View::Held, design, Beat::Idle, 0.6, name));
    }
    take(shot(
        View::Lineup,
        Design::Coil,
        Beat::Idle,
        -0.9,
        "lineup-armed-down.png",
    ));
    // Armed up and to the right, and armed back over the shoulder: the yaw reads too.
    take(Frame {
        armed_yaw: 1.0,
        ..shot(
            View::Lineup,
            Design::Coil,
            Beat::Idle,
            0.35,
            "lineup-armed-right.png",
        )
    });
    take(Frame {
        armed_yaw: 1.2,
        ..shot(
            View::Held,
            Design::Lance,
            Beat::Idle,
            0.3,
            "held-lance-right.png",
        )
    });
    take(Frame {
        armed_yaw: -2.6,
        ..shot(
            View::Held,
            Design::Lance,
            Beat::Idle,
            0.2,
            "held-lance-back.png",
        )
    });
    // And a few more, so the last still is written before the lab exits.
    frames.extend(std::iter::repeat_n(settle(View::Lineup, Design::Coil), 10));
    frames
}

#[derive(Resource)]
struct Capture {
    dir: PathBuf,
    plan: Vec<Frame>,
    next: usize,
}

#[derive(Resource)]
struct Looks {
    body: Handle<StandardMaterial>,
    trim: Handle<StandardMaterial>,
    grip: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    signal: Handle<StandardMaterial>,
    eye: Handle<StandardMaterial>,
    pupil: Handle<StandardMaterial>,
}

impl Looks {
    fn of(&self, finish: Finish) -> Handle<StandardMaterial> {
        match finish {
            Finish::Body => self.body.clone(),
            Finish::Trim => self.trim.clone(),
            Finish::Grip => self.grip.clone(),
            Finish::Glass => self.glass.clone(),
            Finish::Signal => self.signal.clone(),
            Finish::Eye => self.eye.clone(),
            Finish::Pupil => self.pupil.clone(),
        }
    }
}

/// Everything a view spawns, cleared when it changes.
#[derive(Component)]
struct Actor;

/// A tool's root: which design, and how it is placed.
#[derive(Component)]
struct ToolRoot {
    design: Design,
    base: Transform,
    turntable: bool,
}

#[derive(Component)]
struct ToolPart(usize);

#[derive(Component)]
struct LabCamera;

/// A turntable's plinth: shown in the lineup, cleared from the held view.
#[derive(Component)]
struct Plinth;

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
                title: "Observed 2 — Kinetic Tool Lab".to_string(),
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
                rebuild,
                pose_tools,
                place_camera,
                caption,
            )
                .chain(),
        );
    if let Some(capture) = capture {
        std::fs::create_dir_all(&capture.dir).expect("capture directory");
        app.insert_resource(capture);
    }
    app.run();
}

fn hardware(part: Hardware) -> StandardMaterial {
    let f = finish(part);
    StandardMaterial {
        base_color: f.base_color,
        metallic: f.metallic,
        perceptual_roughness: f.roughness,
        alpha_mode: if part == Hardware::Glass {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(Looks {
        body: materials.add(hardware(Hardware::Body)),
        trim: materials.add(hardware(Hardware::Trim)),
        grip: materials.add(hardware(Hardware::Grip)),
        glass: materials.add(hardware(Hardware::Glass)),
        signal: materials.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.02, 0.03),
            emissive: LinearRgba::BLACK,
            ..default()
        }),
        eye: materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.9, 0.85),
            emissive: LinearRgba::rgb(2.5, 2.4, 2.1),
            ..default()
        }),
        pupil: materials.add(StandardMaterial {
            base_color: Color::srgb(0.01, 0.01, 0.01),
            perceptual_roughness: 0.3,
            ..default()
        }),
    });
    commands.spawn((
        LabCamera,
        Camera3d::default(),
        Hdr,
        Bloom {
            intensity: 0.12,
            ..Bloom::NATURAL
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 55f32.to_radians(),
            near: 0.02,
            ..default()
        }),
        Transform::default(),
    ));
    // The facility's light: the moon, and a warm district key.
    commands.spawn((
        DirectionalLight {
            color: moon(),
            illuminance: 8_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 2,
            maximum_distance: 30.0,
            first_cascade_far_bound: 8.0,
            ..default()
        }
        .build(),
        Transform::from_translation(Vec3::from_array(toward_moon()))
            .looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.78, 0.52),
            intensity: 1_500_000.0,
            range: 30.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-3.0, 5.0, 4.0),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: moon(),
        brightness: 90.0,
        ..default()
    });
    let plain = |role: Role| StandardMaterial {
        base_color: treatment(role).base_color,
        perceptual_roughness: 0.85,
        ..default()
    };
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(60.0, 60.0))),
        MeshMaterial3d(materials.add(plain(Role::Catwalk))),
        Transform::default(),
    ));
    // The plinths the turntables stand on.
    let plinth = meshes.add(Cylinder::new(0.9, 0.6));
    let stone = materials.add(plain(Role::Wall));
    for x in [-2.6, 0.0, 2.6] {
        commands.spawn((
            Plinth,
            Mesh3d(plinth.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(x, 0.3, 0.0),
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
    if keys.just_pressed(KeyCode::Digit1) {
        lab.view = View::Lineup;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        lab.view = View::Held;
    }
    if keys.just_pressed(KeyCode::Tab) {
        lab.design = match lab.design {
            Design::Coil => Design::Plumb,
            Design::Plumb => Design::Lance,
            Design::Lance => Design::Coil,
        };
    }
    for (key, beat) in [
        (KeyCode::Space, Beat::Push),
        (KeyCode::KeyE, Beat::Pull),
        (KeyCode::KeyF, Beat::Lash),
    ] {
        if keys.just_pressed(key) {
            lab.fire(beat);
        }
    }
    if keys.pressed(KeyCode::ArrowUp) {
        lab.armed_pitch = (lab.armed_pitch + 0.02).min(FRAC_PI_2);
    }
    if keys.pressed(KeyCode::ArrowDown) {
        lab.armed_pitch = (lab.armed_pitch - 0.02).max(-FRAC_PI_2);
    }
    let turn =
        f32::from(keys.pressed(KeyCode::ArrowRight)) - f32::from(keys.pressed(KeyCode::ArrowLeft));
    lab.armed_yaw = (lab.armed_yaw + turn * 0.03 + PI).rem_euclid(TAU) - PI;
    if keys.just_pressed(KeyCode::KeyA) {
        lab.auto = !lab.auto;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        lab.paused = !lab.paused;
    }
}

/// The automatic cycle: push, pull, lash, a second apart, while the pitch sweeps.
fn advance(time: Res<Time>, mut lab: ResMut<Lab>, capture: Option<Res<Capture>>) {
    if lab.paused || capture.is_some() {
        return;
    }
    let dt = time.delta_secs();
    let before = lab.clock;
    lab.clock += dt;
    lab.since += dt;
    if lab.auto {
        lab.armed_pitch = 1.1 * (lab.clock * 0.35).sin();
        lab.armed_yaw = 1.4 * (lab.clock * 0.23).sin();
        let beat = |t: f32| (t / 1.2).floor() as i64;
        if beat(lab.clock) != beat(before) {
            let next = match beat(lab.clock).rem_euclid(4) {
                0 => Beat::Push,
                1 => Beat::Pull,
                2 => Beat::Lash,
                _ => Beat::Idle,
            };
            lab.fire(next);
        }
    }
}

fn drive_capture(
    mut commands: Commands,
    capture: Option<ResMut<Capture>>,
    mut lab: ResMut<Lab>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut capture) = capture else {
        return;
    };
    let Some(frame) = capture.plan.get(capture.next).copied() else {
        exit.write(AppExit::Success);
        return;
    };
    capture.next += 1;
    lab.view = frame.view;
    lab.design = frame.design;
    lab.beat = frame.beat;
    lab.since = frame.since;
    lab.armed_pitch = frame.armed_pitch;
    lab.armed_yaw = frame.armed_yaw;
    lab.clock = frame.clock;
    if let Some(output) = frame.output {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(capture.dir.join(output)));
    }
}

fn rebuild(
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    mut meshes: ResMut<Assets<Mesh>>,
    looks: Res<Looks>,
    actors: Query<Entity, With<Actor>>,
) {
    let wanted = (lab.view, lab.design);
    if lab.built == Some(wanted) {
        return;
    }
    lab.built = Some(wanted);
    for entity in &actors {
        commands.entity(entity).despawn();
    }
    let placed: Vec<(Design, Transform, bool)> = match lab.view {
        View::Lineup => Design::ALL
            .into_iter()
            .zip([-2.6, 0.0, 2.6])
            .map(|(design, x)| {
                (
                    design,
                    Transform::from_xyz(x, 1.25, 0.0).with_scale(Vec3::splat(TURNTABLE_SCALE)),
                    true,
                )
            })
            .collect(),
        // Held: placed under the camera each frame.
        View::Held => vec![(lab.design, Transform::IDENTITY, false)],
    };
    for (design, base, turntable) in placed {
        commands
            .spawn((
                Actor,
                ToolRoot {
                    design,
                    base,
                    turntable,
                },
                base,
                Visibility::Visible,
            ))
            .with_children(|root| {
                for (index, part) in parts(design).into_iter().enumerate() {
                    root.spawn((
                        ToolPart(index),
                        Mesh3d(meshes.add(mesh(part.shape))),
                        MeshMaterial3d(looks.of(part.finish)),
                        Transform::IDENTITY,
                    ));
                }
            });
    }
}

#[allow(clippy::type_complexity)]
fn pose_tools(
    lab: Res<Lab>,
    looks: Res<Looks>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Query<&Transform, (With<LabCamera>, Without<ToolRoot>, Without<ToolPart>)>,
    mut roots: Query<(&ToolRoot, &mut Transform, &Children), Without<ToolPart>>,
    mut tool_parts: Query<(&ToolPart, &mut Transform), Without<ToolRoot>>,
) {
    let state = lab.state();
    let mut signal = 0.0;
    for (root, mut transform, children) in &mut roots {
        let pose = pose(root.design, state, lab.clock);
        signal = pose.signal;
        *transform = if root.turntable {
            // Three-quarter on to the camera, turning slowly.
            root.base
                * Transform::from_rotation(Quat::from_rotation_y(
                    -2.2 + 0.6 * (lab.clock * 0.25).sin(),
                ))
        } else if let Ok(eye) = camera.single() {
            *eye * Transform::from_translation(HELD_AT)
                * Transform::from_rotation(Quat::from_rotation_z(HELD_ROLL))
                    .with_scale(Vec3::splat(HELD_SCALE))
        } else {
            root.base
        };
        for child in children.iter() {
            if let Ok((part, mut at)) = tool_parts.get_mut(child)
                && let Some(placed) = pose.parts.get(part.0)
            {
                *at = *placed;
            }
        }
    }
    // Push and lash burn in the push colour, a pull in the pull colour.
    let role = if lab.beat == Beat::Pull {
        Role::Pull
    } else {
        Role::Push
    };
    if let Some(mut material) = materials.get_mut(&looks.signal) {
        material.emissive = LinearRgba::from(treatment(role).base_color) * (signal * 6.0);
    }
}

fn place_camera(
    lab: Res<Lab>,
    mut camera: Query<&mut Transform, With<LabCamera>>,
    mut plinths: Query<&mut Visibility, With<Plinth>>,
) {
    for mut visibility in &mut plinths {
        *visibility = if lab.view == View::Lineup {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    *transform = match lab.view {
        View::Lineup => {
            Transform::from_xyz(0.0, 2.3, 5.2).looking_at(Vec3::new(0.0, 1.2, 0.0), Vec3::Y)
        }
        // Standing on the floor, looking a little down the facility.
        View::Held => {
            Transform::from_xyz(0.0, 1.62, 3.0).looking_at(Vec3::new(0.0, 1.3, -4.0), Vec3::Y)
        }
    };
}

fn caption(lab: Res<Lab>, mut text: Query<&mut Text, With<Caption>>) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let showing = match lab.view {
        View::Lineup => "COIL / PLUMB / LANCE".to_string(),
        View::Held => lab.design.title().to_string(),
    };
    text.0 = format!(
        "{showing}   {:?}   lash armed pitch {:+.0} deg  yaw {:+.0} deg\n1 lineup  2 held  Tab design  Space push  E pull  F lash  Up/Down pitch  Left/Right yaw  A auto ({})",
        lab.beat,
        lab.armed_pitch.to_degrees(),
        lab.armed_yaw.to_degrees(),
        if lab.auto { "on" } else { "off" },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capture plan photographs every design, and every still is preceded by
    /// frames that let its layout settle.
    #[test]
    fn the_capture_plan_shows_every_design() {
        let frames = plan();
        let shots: Vec<_> = frames
            .iter()
            .filter(|frame| frame.output.is_some())
            .collect();
        for design in Design::ALL {
            assert!(
                shots
                    .iter()
                    .any(|frame| frame.view == View::Held && frame.design == design),
                "{design:?} is never photographed in a hand"
            );
        }
        for (index, frame) in frames.iter().enumerate() {
            if frame.output.is_some() {
                assert!(index >= 6 && frames[index - 1].output.is_none());
            }
        }
    }
}
