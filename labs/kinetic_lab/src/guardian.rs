//! Original face-bearing geometric guardians, built entirely from primitives.
//! The rig is decorative: it never changes collision, aim, motion, or AI.
use crate::{model::ActorId, runtime::Runtime};
use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};
use observed_style::kinetic::{self, Role};
use std::f32::consts::{PI, TAU};

#[derive(Resource)]
pub struct GuardianArt {
    pyramid: Handle<Mesh>,
    sphere: Handle<Mesh>,
    rod: Handle<Mesh>,
    shell: Handle<StandardMaterial>,
    limb: Handle<StandardMaterial>,
    eye: Handle<StandardMaterial>,
    ink: Handle<StandardMaterial>,
    signal: Handle<StandardMaterial>,
}
/// Rigs are keyed by a plain id rather than by this lab's `ActorId`, so a lab
/// with its own simulation types can drive the same Guardian.
#[derive(Component)]
pub struct Rig(pub u32);
#[derive(Component)]
pub struct Limb {
    id: u32,
    side: usize,
    segment: usize,
}
#[derive(Component)]
pub struct Lid(pub u32);

/// What the rig needs to know about one Guardian this frame.
///
/// The rig is decorative and stateless: it never reads a simulation, it is
/// handed one of these per Guardian and poses itself. That is what lets it be
/// shared instead of copied into every lab that wants the silhouette.
#[derive(Clone, Copy, Debug)]
pub struct RigSample {
    pub velocity: Vec3,
    /// Where to face when not moving under its own power — usually the vector
    /// toward whoever it is hunting.
    pub toward: Vec3,
    pub grounded: bool,
    pub staggered: bool,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut material = |role| {
        let t = kinetic::treatment(role);
        materials.add(StandardMaterial {
            base_color: t.base_color,
            emissive: t.emissive,
            perceptual_roughness: 0.82,
            ..default()
        })
    };
    commands.insert_resource(GuardianArt {
        pyramid: meshes.add(pyramid()),
        sphere: meshes.add(Sphere::new(1.).mesh().uv(20, 12)),
        rod: meshes.add(Cylinder::new(1., 1.).mesh().resolution(8)),
        shell: material(Role::GuardianShell),
        limb: material(Role::GuardianLimb),
        eye: material(Role::GuardianEye),
        ink: material(Role::Panel),
        signal: material(Role::Minor),
    });
}
fn pyramid() -> Mesh {
    let base: [Vec3; 3] = std::array::from_fn(|i| {
        let angle = i as f32 * TAU / 3. + PI / 3.;
        Vec3::new(angle.sin() * 0.44, -0.12, angle.cos() * 0.44)
    });
    let apex = Vec3::new(0., 0.52, 0.);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    for face in [
        [base[0], base[1], apex],
        [base[1], base[2], apex],
        [base[2], base[0], apex],
        [base[2], base[1], base[0]],
    ] {
        let normal = (face[1] - face[0]).cross(face[2] - face[0]).normalize();
        positions.extend(face.map(|p| p.to_array()));
        normals.extend([normal.to_array(); 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0., 0.]; 12])
}
fn part(
    commands: &mut Commands,
    parent: Entity,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    transform: Transform,
) -> Entity {
    let entity = commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            transform,
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}
fn ellipsoid(
    commands: &mut Commands,
    parent: Entity,
    art: &GuardianArt,
    material: &Handle<StandardMaterial>,
    p: Vec3,
    size: Vec3,
) -> Entity {
    part(
        commands,
        parent,
        &art.sphere,
        material,
        Transform::from_translation(p).with_scale(size),
    )
}
fn rod(a: Vec3, b: Vec3, radius: f32) -> Transform {
    Transform::from_translation((a + b) * 0.5)
        .with_rotation(Quat::from_rotation_arc(
            Vec3::Y,
            (b - a).normalize_or_zero(),
        ))
        .with_scale(Vec3::new(radius, a.distance(b), radius))
}
pub fn spawn(commands: &mut Commands, art: &GuardianArt, root: Entity, id: u32) {
    let rig = commands
        .spawn((Rig(id), Transform::default(), Visibility::default()))
        .id();
    commands.entity(root).add_child(rig);
    part(
        commands,
        rig,
        &art.pyramid,
        &art.shell,
        Transform::default(),
    );
    for side in 0..3 {
        let angle = side as f32 * TAU / 3.;
        let face = commands
            .spawn((
                Transform::from_rotation(Quat::from_rotation_y(angle)),
                Visibility::default(),
            ))
            .id();
        commands.entity(rig).add_child(face);
        // Faces are part of the solid, without lenses, armour plates, rivets or gears.
        ellipsoid(
            commands,
            face,
            art,
            &art.limb,
            Vec3::new(0., 0.19, 0.135),
            Vec3::new(0.138, 0.088, 0.047),
        );
        ellipsoid(
            commands,
            face,
            art,
            &art.eye,
            Vec3::new(0., 0.19, 0.167),
            Vec3::new(0.107, 0.060, 0.042),
        );
        ellipsoid(
            commands,
            face,
            art,
            &art.ink,
            Vec3::new(0., 0.19, 0.202),
            Vec3::new(0.036, 0.040, 0.013),
        );
        ellipsoid(
            commands,
            face,
            art,
            &art.signal,
            Vec3::new(-0.012, 0.206, 0.215),
            Vec3::splat(0.010),
        );
        let lid = ellipsoid(
            commands,
            face,
            art,
            &art.shell,
            Vec3::new(0., 0.252, 0.17),
            Vec3::new(0.12, 0.013, 0.045),
        );
        commands.entity(lid).insert(Lid(id));
        // An unsmiling, shallow mouth under each eye.
        for (y, width, role) in [
            (0.015, 0.081, &art.limb),
            (0.004, 0.072, &art.ink),
            (-0.011, 0.073, &art.shell),
        ] {
            ellipsoid(
                commands,
                face,
                art,
                role,
                Vec3::new(0., y, 0.206),
                Vec3::new(width, 0.009, 0.018),
            );
        }
        // Small luminous brow marks retain the shared threat signal in darkness.
        part(
            commands,
            face,
            &art.rod,
            &art.signal,
            rod(
                Vec3::new(-0.095, 0.29, 0.102),
                Vec3::new(0.095, 0.29, 0.102),
                0.009,
            ),
        );
        for segment in 0..3 {
            let (a, b) = limb_points(side, segment, 0., false);
            let entity = part(
                commands,
                rig,
                &art.rod,
                &art.limb,
                rod(a, b, if segment == 2 { 0.025 } else { 0.022 }),
            );
            commands.entity(entity).insert(Limb { id, side, segment });
        }
        let (_, knee) = limb_points(side, 0, 0., false);
        ellipsoid(commands, rig, art, &art.shell, knee, Vec3::splat(0.04));
    }
}
fn limb_points(side: usize, segment: usize, stride: f32, airborne: bool) -> (Vec3, Vec3) {
    let angle = side as f32 * TAU / 3.;
    let turn = Quat::from_rotation_y(angle);
    let lift = if airborne {
        0.16
    } else {
        stride.max(0.) * 0.07
    };
    let points = [
        Vec3::new(0., -0.055, 0.22),
        Vec3::new(0., 0.04, 0.46),
        Vec3::new(stride * 0.065, -0.48 + lift, 0.39),
        Vec3::new(stride * 0.065, -0.515 + lift, 0.48),
    ];
    (turn * points[segment], turn * points[segment + 1])
}
/// The three transform families of a Guardian rig, each filtered off the other
/// two so Bevy can hand out disjoint mutable access to `Transform`.
pub type Rigs<'w, 's> =
    Query<'w, 's, (&'static Rig, &'static mut Transform), (Without<Limb>, Without<Lid>)>;
pub type Limbs<'w, 's> =
    Query<'w, 's, (&'static Limb, &'static mut Transform), (Without<Rig>, Without<Lid>)>;
pub type Lids<'w, 's> =
    Query<'w, 's, (&'static Lid, &'static mut Transform), (Without<Rig>, Without<Limb>)>;

pub fn animate(runtime: Res<Runtime>, mut rigs: Rigs, mut limbs: Limbs, mut lids: Lids) {
    let w = &runtime.world;
    animate_with(w.tick, &mut rigs, &mut limbs, &mut lids, |id| {
        let actor = w.actors.get(&ActorId(id))?;
        Some(RigSample {
            velocity: w.velocity(actor.id),
            toward: w.player.position - w.pose(actor.id).position,
            grounded: actor.grounded,
            staggered: actor.stagger > 0,
        })
    });
}

/// Pose every rig from a caller-supplied sample.
pub fn animate_with(
    tick: u64,
    rigs: &mut Rigs,
    limbs: &mut Limbs,
    lids: &mut Lids,
    sample: impl Fn(u32) -> Option<RigSample>,
) {
    for (rig, mut transform) in rigs.iter_mut() {
        let Some(state) = sample(rig.0) else {
            continue;
        };
        let direction =
            if state.grounded && !state.staggered && state.velocity.with_y(0.).length() > 0.3 {
                state.velocity
            } else {
                state.toward
            };
        let tilt = if state.staggered {
            (tick as f32 * 0.8).sin() * 0.10
        } else {
            0.
        };
        transform.rotation =
            Quat::from_rotation_y(direction.x.atan2(direction.z)) * Quat::from_rotation_z(tilt);
    }
    for (limb, mut transform) in limbs.iter_mut() {
        let Some(state) = sample(limb.id) else {
            continue;
        };
        let moving = state.grounded && !state.staggered && state.velocity.with_y(0.).length() > 0.3;
        let stride = if moving {
            (tick as f32 * 0.24 + limb.side as f32 * TAU / 3.).sin()
        } else {
            0.
        };
        let (a, b) = limb_points(limb.side, limb.segment, stride, !state.grounded);
        *transform = rod(a, b, if limb.segment == 2 { 0.025 } else { 0.022 });
    }
    for (lid, mut transform) in lids.iter_mut() {
        let blink = (tick + u64::from(lid.0) * 37) % 241 < 9;
        transform.translation.y = if blink { 0.19 } else { 0.252 };
        transform.scale.y = if blink { 0.065 } else { 0.013 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decorative_feet_stay_inside_existing_collision_envelope() {
        for side in 0..3 {
            for segment in 0..3 {
                for stride in [-1., 0., 1.] {
                    for airborne in [false, true] {
                        let (a, b) = limb_points(side, segment, stride, airborne);
                        for p in [a, b] {
                            assert!(p.abs().max_element() + 0.025 <= 0.55);
                        }
                    }
                }
            }
        }
    }
}
