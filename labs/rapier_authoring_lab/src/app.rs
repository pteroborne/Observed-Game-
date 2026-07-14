use std::path::PathBuf;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    window::{PresentMode, WindowResolution},
};
use observed_style::{MarkerRole, SurfaceRole, Treatment, marker, surface};
use player_input::PlayerIntent;
use rapier3d::prelude::{ColliderBuilder, Vector};

use crate::{
    import::{ConvexBrush, DoorState, Facility, parse_course},
    map_source,
    physics::{CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, DT, RapierCourse},
};

#[derive(Component)]
struct LabGeometry;

#[derive(Component)]
struct PlayerVisual;

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct Monitor;

#[derive(Resource, Default)]
struct IntentBuffer(PlayerIntent);

#[derive(Resource)]
struct Lab {
    facility: Facility,
    course: RapierCourse,
    debug: bool,
    reset_count: u32,
    geometry_dirty: bool,
}

fn material(treatment: Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        perceptual_roughness: 0.82,
        metallic: 0.08,
        ..default()
    }
}

fn hull_mesh(hull: &ConvexBrush) -> Mesh {
    let points: Vec<Vector> = hull
        .points
        .iter()
        .map(|point| Vector::from(*point))
        .collect();
    let collider = ColliderBuilder::convex_hull(&points)
        .expect("projected hull is valid")
        .build();
    let poly = collider
        .shape()
        .as_convex_polyhedron()
        .expect("convex hull builder returns a polyhedron");
    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut indices = Vec::<u32>::new();
    for face in poly.faces() {
        let first = face.first_vertex_or_edge as usize;
        let count = face.num_vertices_or_edges as usize;
        let adjacent = &poly.vertices_adj_to_face()[first..first + count];
        let base = positions.len() as u32;
        for vertex in adjacent {
            positions.push(poly.points()[*vertex as usize].into());
            normals.push(face.normal.into());
            uvs.push([0.0, 0.0]);
        }
        for index in 1..count - 1 {
            indices.extend_from_slice(&[base, base + index as u32, base + index as u32 + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/maps/rapier_course.map");
    map_source::materialize(&path).expect("materialize editable TrenchBroom map");
    let facility = parse_course();
    let course = RapierCourse::new(facility.clone());

    commands.spawn((
        PlayerCamera,
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 3_500.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.7, 0.0)),
    ));
    commands.spawn((
        PlayerVisual,
        Mesh3d(meshes.add(Capsule3d::new(CAPSULE_RADIUS, CAPSULE_HALF_HEIGHT * 2.0))),
        MeshMaterial3d(materials.add(material(marker(MarkerRole::You)))),
        Transform::default(),
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(18.0),
            top: Val::Px(14.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.01, 0.02, 0.035, 0.9)),
        children![(
            Monitor,
            Text::new("Rapier authoring lab"),
            TextFont {
                font_size: 17.0,
                ..default()
            },
            TextColor(Color::WHITE),
        )],
    ));
    commands.insert_resource(Lab {
        facility,
        course,
        debug: true,
        reset_count: 0,
        geometry_dirty: true,
    });
}

fn rebuild_geometry(
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    existing: Query<Entity, With<LabGeometry>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !lab.geometry_dirty {
        return;
    }
    lab.geometry_dirty = false;
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    for hull in &lab.facility.structural {
        let role = if hull.max.y <= 0.05 {
            SurfaceRole::Plain
        } else if hull.points.len() == 6 {
            SurfaceRole::Spine
        } else {
            SurfaceRole::Wall
        };
        commands.spawn((
            LabGeometry,
            Mesh3d(meshes.add(hull_mesh(hull))),
            MeshMaterial3d(materials.add(material(surface(role)))),
        ));
    }
    for (index, door) in lab.facility.doors.iter().enumerate() {
        if lab.course.door_states()[index] == DoorState::Closed {
            commands.spawn((
                LabGeometry,
                Mesh3d(meshes.add(hull_mesh(&door.hull))),
                MeshMaterial3d(materials.add(material(surface(SurfaceRole::TrapArmed)))),
            ));
        }
    }
}

fn sample_input(keys: Res<ButtonInput<KeyCode>>, mut buffer: ResMut<IntentBuffer>) {
    let axis =
        |negative, positive| (keys.pressed(positive) as i32 - keys.pressed(negative) as i32) as f32;
    buffer.0 = PlayerIntent {
        movement: Vec2::new(
            axis(KeyCode::KeyA, KeyCode::KeyD),
            axis(KeyCode::KeyS, KeyCode::KeyW),
        ),
        look: Vec2::new(
            axis(KeyCode::ArrowLeft, KeyCode::ArrowRight),
            axis(KeyCode::ArrowUp, KeyCode::ArrowDown),
        ),
        jump_pressed: keys.just_pressed(KeyCode::Space),
        sprint_held: keys.pressed(KeyCode::ShiftLeft),
        ..Default::default()
    };
}

fn simulate(mut lab: ResMut<Lab>, mut buffer: ResMut<IntentBuffer>) {
    lab.course.step(buffer.0);
    buffer.0.jump_pressed = false;
}

fn toggles(keys: Res<ButtonInput<KeyCode>>, mut lab: ResMut<Lab>) {
    if keys.just_pressed(KeyCode::KeyO) {
        lab.course.toggle_door(0);
        lab.geometry_dirty = true;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        lab.course = RapierCourse::new(lab.facility.clone());
        lab.reset_count += 1;
        lab.geometry_dirty = true;
    }
    if keys.just_pressed(KeyCode::F1) {
        lab.debug = !lab.debug;
    }
}

fn present(
    lab: Res<Lab>,
    mut player: Single<&mut Transform, (With<PlayerVisual>, Without<PlayerCamera>)>,
    mut camera: Single<&mut Transform, (With<PlayerCamera>, Without<PlayerVisual>)>,
) {
    let state = lab.course.player();
    let position = Vec3::new(state.position.x, state.position.y, state.position.z);
    player.translation = position;
    let (sin_yaw, cos_yaw) = state.yaw.sin_cos();
    let cos_pitch = state.pitch.cos();
    let look = Vec3::new(
        -sin_yaw * cos_pitch,
        state.pitch.sin(),
        -cos_yaw * cos_pitch,
    );
    camera.translation = position + Vec3::Y * 0.55;
    camera.look_to(look, Vec3::Y);
}

fn debug_draw(lab: Res<Lab>, mut gizmos: Gizmos) {
    if !lab.debug {
        return;
    }
    for room in &lab.facility.rooms {
        draw_box(
            &mut gizmos,
            room.min,
            room.max,
            Color::srgb(0.15, 0.65, 1.0),
        );
    }
    for port in &lab.facility.ports {
        gizmos.line(
            port.position,
            port.position + Vec3::Y * 2.5,
            Color::srgb(0.85, 0.25, 1.0),
        );
    }
    for (index, door) in lab.facility.doors.iter().enumerate() {
        let color = if lab.course.door_states()[index] == DoorState::Closed {
            Color::srgb(1.0, 0.12, 0.08)
        } else {
            Color::srgb(0.15, 1.0, 0.35)
        };
        draw_box(&mut gizmos, door.hull.min, door.hull.max, color);
    }
}

fn draw_box(gizmos: &mut Gizmos, min: Vec3, max: Vec3, color: Color) {
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    for (a, b) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        gizmos.line(corners[a], corners[b], color);
    }
}

fn update_monitor(lab: Res<Lab>, mut monitor: Single<&mut Text, With<Monitor>>) {
    let player = lab.course.player();
    let door = lab.course.door_states()[0];
    **monitor = Text::new(format!(
        "RAPIER + TRENCHBROOM — CONVEX CAPSULE SLICE\n\n\
         map semantics   {} rooms | {} ports | {} door\n\
         collision       {} convex hulls | ramp = triangular prism\n\
         door 2          {door:?} (O toggles model + collider)\n\
         player          [{:.2}, {:.2}, {:.2}] | grounded {}\n\
         contacts/tick   {} | hash {:016x}\n\
         reset count     {}\n\n\
         WASD move | Shift sprint | Space jump | arrows look\n\
         O door | R reset | F1 semantic debug\n\n\
         Legend: cyan room bounds | purple ports | red/green door\n\
         gold surface = imported convex ramp",
        lab.facility.rooms.len(),
        lab.facility.ports.len(),
        lab.facility.doors.len(),
        lab.course.collider_count(),
        player.position.x,
        player.position.y,
        player.position.z,
        player.grounded,
        player.collisions_this_tick,
        lab.course.state_hash(),
        lab.reset_count,
    ));
}

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.004, 0.007, 0.014)))
        .insert_resource(Time::<Fixed>::from_seconds(DT as f64))
        .init_resource::<IntentBuffer>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Rapier Authoring Lab".into(),
                resolution: WindowResolution::new(1280, 720),
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                sample_input,
                toggles,
                rebuild_geometry,
                present,
                debug_draw,
                update_monitor,
            )
                .chain(),
        )
        .add_systems(FixedUpdate, simulate)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_door_rebuild_and_reset_restore_geometry_entity_count() {
        let facility = parse_course();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(Lab {
                course: RapierCourse::new(facility.clone()),
                facility,
                debug: true,
                reset_count: 0,
                geometry_dirty: true,
            })
            .add_systems(Update, rebuild_geometry);

        fn geometry_count(app: &mut App) -> usize {
            app.world_mut()
                .query_filtered::<Entity, With<LabGeometry>>()
                .iter(app.world())
                .count()
        }
        app.update();
        let baseline = geometry_count(&mut app);

        {
            let mut lab = app.world_mut().resource_mut::<Lab>();
            lab.course.toggle_door(0);
            lab.geometry_dirty = true;
        }
        app.update();
        assert_eq!(geometry_count(&mut app), baseline - 1);

        {
            let mut lab = app.world_mut().resource_mut::<Lab>();
            lab.course = RapierCourse::new(lab.facility.clone());
            lab.geometry_dirty = true;
        }
        app.update();
        assert_eq!(geometry_count(&mut app), baseline);
    }
}
