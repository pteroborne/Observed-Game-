//! Every Observer but the one you are, drawn as a floating eye (`observed_observer`).
//!
//! The eye hangs where the body's own eye is and looks where the body looks: the
//! simulation's yaw and pitch, eased, because a bot's heading can snap between one
//! tick and the next and an eye that snaps reads as a glitch rather than a glance.
//! Its iris is whose it is - yours, a teammate's, a rival's - from
//! `observed_style::observer`, which owns the colours and their legibility rules.
//!
//! An eye at arm's length goes translucent. Bodies set out together and bots walk in
//! file, so the eye ahead is often right at your own eye height, and a dark globe
//! half a metre across there hides the doorway you are both heading for. The old
//! capsule was translucent always for the same reason; the eye is solid where it is
//! read, across a room, and gives way only where it is in the way.
//!
//! Presentation only: it reads the authoritative snapshot and writes nothing back.
//! `entities` owns each body's root - where it stands and whether it is drawn at all;
//! this module dresses the root and poses its parts.
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use observed_observer::form::{self, Gaze, Look};
use observed_style::MarkerRole;
use observed_style::guardian::Finish;
use observed_style::observer::{self as style, Part};

use super::entities::ActorVisual;
use super::sim::HexWfcRuntime;

/// How fast a drawn eye turns after where its body is looking, per second.
const GAZE_RESPONSE: f32 = 10.0;
/// Closer than this to the camera, an eye goes translucent; it comes back past the
/// second, so one standing at the threshold does not flicker.
const NEAR: f32 = 2.4;
const NEAR_CLEAR: f32 = 2.8;
/// How much of a near eye still shows.
const NEAR_ALPHA: f32 = 0.22;

/// The meshes and materials every eye shares.
pub(super) struct ObserverArt {
    parts: Vec<(Handle<Mesh>, Look)>,
    globe: Handle<StandardMaterial>,
    trim: Handle<StandardMaterial>,
    pupil: Handle<StandardMaterial>,
    /// Iris and haze for each role, in `style::ROLES` order.
    roles: Vec<(
        MarkerRole,
        Handle<StandardMaterial>,
        Handle<StandardMaterial>,
    )>,
    /// The same, translucent, for an eye at arm's length: globe, trim, pupil, and
    /// each role's iris.
    near: [Handle<StandardMaterial>; 3],
    near_irises: Vec<Handle<StandardMaterial>>,
}

/// One eye's materials, solid and near, part by part, and which it is wearing.
#[derive(Component)]
pub(super) struct EyeMaterials {
    solid: Vec<Handle<StandardMaterial>>,
    near: Vec<Handle<StandardMaterial>>,
    wearing_near: bool,
}

/// The gaze an eye is drawn with, easing after its body's.
#[derive(Component)]
pub(super) struct DrawnGaze(Gaze);

/// One part of an eye, by its index in `form::parts`.
#[derive(Component)]
pub(super) struct EyePart(usize);

/// Which eye this is, so a room of them blinks and bobs out of step.
#[derive(Component)]
pub(super) struct EyeSeed(u32);

fn finish(finish: Finish) -> StandardMaterial {
    StandardMaterial {
        base_color: finish.base_color,
        emissive: finish.emissive,
        metallic: finish.metallic,
        perceptual_roughness: finish.roughness,
        ..default()
    }
}

/// A finish seen through: most of it gone, its light dimmed with it.
fn near(finish: Finish) -> StandardMaterial {
    StandardMaterial {
        base_color: finish.base_color.with_alpha(NEAR_ALPHA),
        emissive: finish.emissive * NEAR_ALPHA,
        alpha_mode: AlphaMode::Blend,
        ..self::finish(finish)
    }
}

impl ObserverArt {
    pub(super) fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        Self {
            parts: form::parts()
                .into_iter()
                .map(|part| {
                    (
                        meshes.add(observed_observer::mesh::mesh(part.shape)),
                        part.look,
                    )
                })
                .collect(),
            globe: materials.add(finish(style::finish(Part::Globe))),
            trim: materials.add(finish(style::finish(Part::Trim))),
            pupil: materials.add(finish(style::finish(Part::Pupil))),
            roles: style::ROLES
                .into_iter()
                .map(|role| {
                    let iris = materials.add(finish(style::finish(Part::Iris(role))));
                    let haze = materials.add(StandardMaterial {
                        base_color: Color::LinearRgba(style::haze(role)),
                        alpha_mode: AlphaMode::Add,
                        unlit: true,
                        cull_mode: None,
                        ..default()
                    });
                    (role, iris, haze)
                })
                .collect(),
            near: [Part::Globe, Part::Trim, Part::Pupil]
                .map(|part| materials.add(near(style::finish(part)))),
            near_irises: style::ROLES
                .into_iter()
                .map(|role| materials.add(near(style::finish(Part::Iris(role)))))
                .collect(),
        }
    }

    /// Dress `root` as an eye in `role`'s colour.
    pub(super) fn dress(&self, commands: &mut Commands, root: Entity, role: MarkerRole, seed: u32) {
        let index = self
            .roles
            .iter()
            .position(|(r, ..)| *r == role)
            .expect("an Observer's role is one of style::ROLES");
        let (_, iris, haze) = &self.roles[index];
        let [near_globe, near_trim, near_pupil] = &self.near;
        let (solid, near): (Vec<_>, Vec<_>) = self
            .parts
            .iter()
            .map(|(_, look)| match look {
                Look::Globe => (self.globe.clone(), near_globe.clone()),
                Look::Trim => (self.trim.clone(), near_trim.clone()),
                Look::Iris => (iris.clone(), self.near_irises[index].clone()),
                Look::Pupil => (self.pupil.clone(), near_pupil.clone()),
                // The haze is already only light.
                Look::Haze => (haze.clone(), haze.clone()),
            })
            .unzip();
        commands
            .entity(root)
            .insert((
                DrawnGaze(Gaze::default()),
                EyeSeed(seed),
                EyeMaterials {
                    solid: solid.clone(),
                    near,
                    wearing_near: false,
                },
            ))
            .with_children(|eye| {
                for (index, (mesh, look)) in self.parts.iter().enumerate() {
                    let material = solid[index].clone();
                    let mut part = eye.spawn((
                        EyePart(index),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::default(),
                    ));
                    if *look == Look::Haze {
                        part.insert(NotShadowCaster);
                    }
                }
            });
    }
}

/// The shortest way round from one heading to another.
fn toward_angle(from: f32, to: f32, t: f32) -> f32 {
    let delta =
        (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    from + delta * t
}

/// Whether an eye `distance` from the camera should be seen through, given whether
/// it already is.
fn wears_near(distance: f32, wearing_near: bool) -> bool {
    distance < if wearing_near { NEAR_CLEAR } else { NEAR }
}

/// Turn each eye after its body's gaze, place its parts, and see through it at arm's
/// length.
pub(super) fn sync(
    time: Res<Time>,
    runtime: Res<HexWfcRuntime>,
    camera: Query<&GlobalTransform, With<crate::view::components::GameCam>>,
    mut eyes: Query<(
        &ActorVisual,
        &EyeSeed,
        &mut DrawnGaze,
        &mut EyeMaterials,
        &Transform,
        &Children,
    )>,
    mut parts: Query<
        (
            &EyePart,
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<ActorVisual>,
    >,
) {
    let t = 1.0 - (-GAZE_RESPONSE * time.delta_secs()).exp();
    let clock = time.elapsed_secs();
    let camera = camera.single().ok().map(GlobalTransform::translation);
    for (actor, seed, mut drawn, mut materials, root, children) in &mut eyes {
        let Some(player) = runtime.match_state.players.get(&actor.player()) else {
            continue;
        };
        let gaze = Gaze {
            yaw: toward_angle(drawn.0.yaw, player.yaw, t),
            pitch: drawn.0.pitch + (player.pitch - drawn.0.pitch) * t,
        };
        drawn.0 = gaze;
        let placed = form::pose(gaze, clock, seed.0);
        let centre = root.translation + Vec3::Y * form::EYE_RISE;
        let near = camera
            .is_some_and(|camera| wears_near(camera.distance(centre), materials.wearing_near));
        let swap = near != materials.wearing_near;
        materials.wearing_near = near;
        for child in children.iter() {
            if let Ok((part, mut transform, mut material)) = parts.get_mut(child) {
                *transform = placed[part.0];
                if swap {
                    let set = if near {
                        &materials.near
                    } else {
                        &materials.solid
                    };
                    material.0 = set[part.0].clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The drawn eye must sit where the camera puts the body's own eye, or looking
    /// through a body and looking at it disagree about where it sees from.
    #[test]
    fn the_eye_is_where_the_camera_sees_from() {
        assert!((form::EYE_RISE - super::super::sim::EYE_OFFSET).abs() < 1e-6);
    }

    #[test]
    fn an_eye_at_arms_length_is_seen_through_without_flickering() {
        assert!(wears_near(1.5, false));
        assert!(!wears_near(3.5, false));
        // Between the two thresholds it keeps whatever it was wearing.
        let between = (NEAR + NEAR_CLEAR) * 0.5;
        assert!(wears_near(between, true));
        assert!(!wears_near(between, false));
    }

    #[test]
    fn a_heading_eases_the_short_way_round() {
        let near_pi = std::f32::consts::PI - 0.1;
        let eased = toward_angle(near_pi, -near_pi, 0.5);
        assert!((eased - std::f32::consts::PI).abs() < 1e-5, "{eased}");
        assert!((toward_angle(0.2, 0.6, 1.0) - 0.6).abs() < 1e-6);
    }
}
