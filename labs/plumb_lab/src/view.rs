//! Presentation. A fixed corner camera watching one Modron be lied to about
//! which way the ground is.

use bevy::camera::Hdr;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::ui::widget::Text;
use observed_style::kinetic::{self, Role};

use crate::model::{Footing, Plumb, PlumbWorld};
use crate::room::{self, Surface};
use crate::script::{self, Phase};

#[derive(Component)]
struct Subject;
#[derive(Component)]
enum HudField {
    Caption,
    State,
}

#[derive(Resource)]
pub struct Run {
    pub world: PlumbWorld,
    pub phases: Vec<Phase>,
    pub phase: usize,
    pub tick: u32,
    pub started: bool,
    pub finished: bool,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            world: PlumbWorld::new(),
            phases: script::phases(),
            phase: 0,
            tick: 0,
            started: false,
            finished: false,
        }
    }
}

impl Run {
    #[must_use]
    pub fn caption(&self) -> &'static str {
        self.phases
            .get(self.phase)
            .map_or("COMPLETE", |phase| phase.caption)
    }

    /// Advance the script by one fixed tick.
    pub fn advance(&mut self) {
        let Some(phase) = self.phases.get(self.phase).copied() else {
            self.finished = true;
            return;
        };
        if !self.started {
            match phase.down {
                Some(down) => self.world.apply(Plumb::new(
                    down,
                    crate::model::GRAVITY,
                    phase.ticks + script::EXPIRY_MARGIN,
                )),
                None => self.world.release(),
            }
            self.started = true;
        }
        let walking = self.tick > phase.ticks / 3;
        if self.tick == phase.ticks * 3 / 4 {
            self.world.reverse();
        }
        self.world.step(walking);
        self.tick += 1;
        if self.tick >= phase.ticks {
            self.phase += 1;
            self.tick = 0;
            self.started = false;
        }
    }
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

/// The room is lit for review rather than for mood.
///
/// The kinetic registers are a neon-noir palette and the darkest of them are
/// near-black, which is right in a facility and useless here: this lab exists
/// to be watched, and a surface nobody can see is a surface nobody can tell the
/// subject is standing on.
fn role_for(surface: Surface) -> Role {
    match surface {
        Surface::Floor => Role::Catwalk,
        // Not `Landing`: that register is a lit understory and, spread across a
        // whole ceiling, it blows the frame out and takes the subject with it.
        Surface::Ceiling => Role::Floor,
        Surface::Wall => Role::Wall,
        Surface::Obstacle => Role::Prop,
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Run>()
        .add_systems(Startup, (setup, kinetic_lab::guardian::setup).chain())
        .add_systems(
            Update,
            (crate::evidence::advance, sync, animate, draw, hud).chain(),
        );
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube = meshes.add(Cuboid::from_length(1.));
    for solid in room::solids() {
        if room::CUTAWAY.contains(&solid.id) {
            continue;
        }
        let treatment = kinetic::treatment(role_for(solid.surface));
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: treatment.base_color,
                emissive: treatment.emissive,
                perceptual_roughness: 0.8,
                ..default()
            })),
            Transform::from_translation(solid.center).with_scale(solid.half * 2.),
            Name::new(format!("{:?} {}", solid.surface, solid.id)),
        ));
    }
    let (eye, at) = room::camera();
    commands.spawn((
        Camera3d::default(),
        // Wide enough to hold floor and ceiling at once from inside a ten-metre
        // room; the default forty-five degrees crops one or the other.
        Projection::Perspective(PerspectiveProjection {
            fov: 1.15,
            ..default()
        }),
        Hdr,
        Bloom::NATURAL,
        Transform::from_translation(eye).looking_at(at, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 6500.,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(14., room::HEIGHT + 6., 16.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // Practicals in the far corners, so the walls and ceiling read as surfaces
    // rather than as absence.
    for (x, z) in [(-1., -1.), (1., -1.), (-1., 1.)] {
        commands.spawn((
            PointLight {
                color: color(Role::Text),
                intensity: 900_000.,
                range: 26.,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(
                x * (room::HALF - 2.0),
                room::HEIGHT - 2.0,
                z * (room::HALF - 2.0),
            ),
        ));
    }
    commands.insert_resource(GlobalAmbientLight {
        color: color(Role::Text),
        brightness: 320.,
        ..default()
    });
    commands.insert_resource(ClearColor(color(Role::Panel)));

    // The subject: an empty root the shared Guardian rig hangs from, so the
    // attitude can be driven from the plumb while the rig does its own walk.
    commands.spawn((
        Subject,
        Transform::from_translation(room::spawn()),
        Visibility::default(),
    ));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(30),
            top: px(24),
            ..default()
        },
        children![(
            Text::new("PLUMB / 01\nWHICH WAY IS DOWN"),
            font(23.),
            TextColor(color(Role::Text)),
        )],
    ));
    for (field, top) in [(HudField::Caption, 100.), (HudField::State, 132.)] {
        commands.spawn((
            field,
            Text::default(),
            font(16.),
            TextColor(color(Role::Text)),
            Node {
                position_type: PositionType::Absolute,
                left: px(30),
                top: px(top),
                ..default()
            },
        ));
    }
}

/// Attach the rig once the art exists, then keep the root on the subject.
fn sync(
    mut commands: Commands,
    run: Res<Run>,
    art: Option<Res<kinetic_lab::guardian::GuardianArt>>,
    mut subjects: Query<(Entity, &mut Transform), With<Subject>>,
    rigs: Query<&kinetic_lab::guardian::Rig>,
) {
    let Ok((entity, mut transform)) = subjects.single_mut() else {
        return;
    };
    if rigs.is_empty()
        && let Some(art) = art
    {
        kinetic_lab::guardian::spawn(&mut commands, &art, entity, 0);
    }
    transform.translation = run.world.position();
    // The rig stands on whatever the plumb says is the ground.
    transform.rotation = run.world.attitude();
}

fn animate(
    run: Res<Run>,
    mut rigs: kinetic_lab::guardian::Rigs,
    mut limbs: kinetic_lab::guardian::Limbs,
    mut lids: kinetic_lab::guardian::Lids,
) {
    let world = &run.world;
    let attitude = world.attitude();
    // The rig turns within the subject's own frame, so its heading has to be
    // expressed there too: the yaw it applies is about the surface normal.
    let local = attitude.inverse();
    kinetic_lab::guardian::animate_with(world.tick, &mut rigs, &mut limbs, &mut lids, |_| {
        Some(kinetic_lab::guardian::RigSample {
            velocity: local * world.velocity(),
            toward: local * world.heading,
            grounded: world.footing == Footing::Planted,
            staggered: false,
        })
    });
}

fn draw(run: Res<Run>, mut gizmos: Gizmos) {
    let world = &run.world;
    let at = world.position();
    // Which way the plumb says down is. This is the one invisible thing in the
    // room, so it gets the arrow.
    let down = world.down();
    gizmos.line(at, at + down * 2.4, color(Role::Hazard));
    for side in [world.up().cross(down.any_orthonormal_vector()), -world.up()] {
        let _ = side;
    }
    gizmos.line(
        at + down * 2.4,
        at + down * 1.8 + down.any_orthonormal_vector() * 0.35,
        color(Role::Hazard),
    );
    gizmos.line(
        at + down * 2.4,
        at + down * 1.8 - down.any_orthonormal_vector() * 0.35,
        color(Role::Hazard),
    );
    // The surface it is standing on, if any.
    if let Some(normal) = world.contact_normal {
        gizmos.line(at, at + normal * 1.6, color(Role::Powered));
    }
}

fn hud(run: Res<Run>, mut texts: Query<(&mut Text, &HudField)>) {
    let world = &run.world;
    for (mut text, field) in &mut texts {
        text.0 = match field {
            HudField::Caption => run.caption().to_string(),
            HudField::State => format!(
                "down {:>5.1} {:>5.1} {:>5.1}    {}    {}    walked {:.1} m",
                world.down().x,
                world.down().y,
                world.down().z,
                match world.footing {
                    Footing::Planted => "PLANTED",
                    Footing::Falling => "falling",
                },
                world
                    .contact_surface
                    .map_or_else(|| "-".to_string(), |surface| format!("{surface:?}")),
                world.walked,
            ),
        };
    }
}
