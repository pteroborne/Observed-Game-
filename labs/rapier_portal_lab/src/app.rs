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
    authoring::{AuthoredModule, ConvexHullData, authored_modules},
    model::{Place, SOURCE_THRESHOLD_Z},
    physics::{DT, PortalSimulation},
};

#[derive(Component)]
struct LabGeometry;

#[derive(Component)]
struct PreviewGeometry;

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct Monitor;

#[derive(Resource, Default)]
struct IntentBuffer(PlayerIntent);

#[derive(Resource)]
struct Lab {
    sim: PortalSimulation,
    geometry_dirty: bool,
    debug: bool,
    resets: u32,
    last_event: String,
}

fn standard_material(treatment: Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: treatment.base_color,
        emissive: treatment.emissive,
        perceptual_roughness: 0.82,
        metallic: 0.08,
        ..default()
    }
}

fn hull_mesh(hull: &ConvexHullData) -> Mesh {
    let points: Vec<Vector> = hull.points.iter().copied().map(Vector::from).collect();
    let collider = ColliderBuilder::convex_hull(&points)
        .expect("imported brush is a convex hull")
        .build();
    let poly = collider
        .shape()
        .as_convex_polyhedron()
        .expect("convex builder returns a polyhedron");
    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut uvs = Vec::<[f32; 2]>::new();
    let mut indices = Vec::<u32>::new();
    for face in poly.faces() {
        let first = face.first_vertex_or_edge as usize;
        let count = face.num_vertices_or_edges as usize;
        let vertices = &poly.vertices_adj_to_face()[first..first + count];
        let base = positions.len() as u32;
        for vertex in vertices {
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
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/maps/portal_modules.map");
    crate::authoring::materialize(&path).expect("materialize editable module kit");
    let modules = authored_modules();
    let sim = PortalSimulation::new(0x000B_1A4E, modules);
    commands.spawn((PlayerCamera, Camera3d::default(), Transform::default()));
    commands.spawn((
        DirectionalLight {
            illuminance: 4_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.7, 0.0)),
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(14.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.008, 0.015, 0.03, 0.91)),
        children![(
            Monitor,
            Text::new("Portal composition lab"),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
        )],
    ));
    commands.insert_resource(Lab {
        sim,
        geometry_dirty: true,
        debug: true,
        resets: 0,
        last_event: "Threshold is player-observed; BLAME is frozen.".into(),
    });
    // Ensure asset stores exist before the first geometry rebuild in headless tests too.
    let _ = (&mut meshes, &mut materials);
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
) {
    commands.spawn((
        LabGeometry,
        Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
        MeshMaterial3d(material),
        Transform::from_translation(center),
    ));
}

fn spawn_room(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    source: bool,
) {
    let floor = materials.add(standard_material(surface(SurfaceRole::Plain)));
    let wall = materials.add(standard_material(surface(SurfaceRole::Wall)));
    spawn_box(
        commands,
        meshes,
        floor,
        Vec3::new(0.0, -0.2, 0.0),
        Vec3::new(16.0, 0.4, 16.0),
    );
    for x in [-8.2, 8.2] {
        spawn_box(
            commands,
            meshes,
            wall.clone(),
            Vec3::new(x, 2.5, 0.0),
            Vec3::new(0.4, 5.0, 16.4),
        );
    }
    let solid_z = if source { 8.2 } else { -8.2 };
    spawn_box(
        commands,
        meshes,
        wall.clone(),
        Vec3::new(0.0, 2.5, solid_z),
        Vec3::new(16.4, 5.0, 0.4),
    );
    let gap_z = -solid_z;
    for x in [-5.2, 5.2] {
        spawn_box(
            commands,
            meshes,
            wall.clone(),
            Vec3::new(x, 2.5, gap_z),
            Vec3::new(5.6, 5.0, 0.4),
        );
    }
}

fn spawn_composition(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    lab: &Lab,
    preview_offset: f32,
    preview: bool,
) {
    for placement in &lab.sim.model.composition.placements {
        let module: &AuthoredModule = lab
            .sim
            .modules()
            .iter()
            .find(|module| module.kind == placement.kind)
            .expect("composition tile exists in authoring catalog");
        let treatment = match placement.kind {
            crate::authoring::ArchitectureKind::Gantry => surface(SurfaceRole::GantryDeck),
            crate::authoring::ArchitectureKind::Colonnade => surface(SurfaceRole::Wall),
        };
        let material = materials.add(standard_material(treatment));
        for hull in &module.hulls {
            let mut entity = commands.spawn((
                LabGeometry,
                Mesh3d(meshes.add(hull_mesh(hull))),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(0.0, 0.0, preview_offset + placement.z_offset),
            ));
            if preview {
                entity.insert(PreviewGeometry);
            }
        }
    }
}

fn spawn_threshold(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    anchored: bool,
) {
    let frame = materials.add(standard_material(surface(SurfaceRole::Wall)));
    for x in [-2.4, 2.4] {
        spawn_box(
            commands,
            meshes,
            frame.clone(),
            Vec3::new(x, 2.5, SOURCE_THRESHOLD_Z),
            Vec3::new(0.25, 5.0, 0.25),
        );
    }
    spawn_box(
        commands,
        meshes,
        frame,
        Vec3::new(0.0, 4.85, SOURCE_THRESHOLD_Z),
        Vec3::new(5.0, 0.3, 0.25),
    );
    let treatment = if anchored {
        marker(MarkerRole::Control)
    } else {
        surface(SurfaceRole::Wall)
    };
    commands.spawn((
        LabGeometry,
        PointLight {
            color: treatment.base_color,
            intensity: if anchored { 2_400.0 } else { 1_000.0 },
            range: 6.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 4.55, SOURCE_THRESHOLD_Z + 0.1),
        Name::new("Anchor-only threshold indicator"),
    ));
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
    match lab.sim.model.place {
        Place::SourceRoom => {
            spawn_room(&mut commands, &mut meshes, &mut materials, true);
            spawn_threshold(
                &mut commands,
                &mut meshes,
                &mut materials,
                lab.sim.model.anchor_indicator_lit(),
            );
            // This is a transformed rendering of the remote hall, not source-room collision.
            spawn_composition(
                &mut commands,
                &mut meshes,
                &mut materials,
                &lab,
                SOURCE_THRESHOLD_Z,
                true,
            );
        }
        Place::Hallway => {
            spawn_composition(&mut commands, &mut meshes, &mut materials, &lab, 0.0, false)
        }
        Place::DestinationRoom => spawn_room(&mut commands, &mut meshes, &mut materials, false),
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

fn simulate(mut lab: ResMut<Lab>, mut intent: ResMut<IntentBuffer>) {
    let before_place = lab.sim.model.place;
    lab.sim.step(intent.0);
    intent.0.jump_pressed = false;
    if lab.sim.model.place != before_place {
        lab.geometry_dirty = true;
        lab.last_event = format!(
            "Threshold crossed into {:?} local space.",
            lab.sim.model.place
        );
    }
}

fn toggles(keys: Res<ButtonInput<KeyCode>>, mut lab: ResMut<Lab>) {
    if keys.just_pressed(KeyCode::KeyB) {
        if lab.sim.request_blame() {
            lab.geometry_dirty = true;
            lab.last_event = "BLAME committed a new WFC order while unobserved.".into();
        } else {
            lab.last_event = "BLAME blocked: player observation or anchor holds the link.".into();
        }
    }
    if keys.just_pressed(KeyCode::KeyE) {
        lab.sim.toggle_anchor();
        lab.geometry_dirty = true;
        lab.last_event = if lab.sim.model.anchored {
            "Anchor placed: link frozen and frame indicator lit.".into()
        } else {
            "Anchor removed: frame indicator idle; player gaze may still freeze.".into()
        };
    }
    if keys.just_pressed(KeyCode::KeyR) {
        lab.sim = PortalSimulation::new(0x000B_1A4E, authored_modules());
        lab.geometry_dirty = true;
        lab.resets += 1;
        lab.last_event = "Lab reset to player-observed, unanchored source room.".into();
    }
    if keys.just_pressed(KeyCode::F1) {
        lab.debug = !lab.debug;
    }
}

fn present(lab: Res<Lab>, mut camera: Single<&mut Transform, With<PlayerCamera>>) {
    let player = lab.sim.player();
    let position = Vec3::new(
        player.position.x,
        player.position.y + 0.55,
        player.position.z,
    );
    let (sin_yaw, cos_yaw) = player.yaw.sin_cos();
    let cos_pitch = player.pitch.cos();
    let look = Vec3::new(
        -sin_yaw * cos_pitch,
        player.pitch.sin(),
        -cos_yaw * cos_pitch,
    );
    camera.translation = position;
    camera.look_to(look, Vec3::Y);
}

fn debug_draw(lab: Res<Lab>, mut gizmos: Gizmos) {
    if !lab.debug || lab.sim.model.place != Place::SourceRoom {
        return;
    }
    let color = if lab.sim.model.anchored {
        Color::srgb(0.1, 1.0, 0.8)
    } else {
        Color::srgb(0.45, 0.62, 0.78)
    };
    gizmos.line(
        Vec3::new(-2.25, 0.05, SOURCE_THRESHOLD_Z),
        Vec3::new(2.25, 0.05, SOURCE_THRESHOLD_Z),
        color,
    );
    let player = lab.sim.player();
    let from = Vec3::new(player.position.x, player.position.y, player.position.z);
    gizmos.line(from, Vec3::new(0.0, 2.0, SOURCE_THRESHOLD_Z), Color::WHITE);
}

fn update_monitor(lab: Res<Lab>, mut monitor: Single<&mut Text, With<Monitor>>) {
    let model = &lab.sim.model;
    **monitor = Text::new(format!(
        "REMOTE PLACE / WFC / RAPIER THRESHOLD\n\n\
         active place       {:?}\n\
         remote composition {} (generation {})\n\
         player observes    {}\n\
         anchor lock        {}\n\
         effective lock     {}\n\
         FRAME LIGHT        {}  <-- anchor only\n\
         Rapier colliders   {}\n\
         BLAME              {} attempts | {} commits\n\
         resets             {}\n\n\
         {}\n\n\
         WASD/Shift/Space move | arrows look\n\
         B BLAME | E anchor | R reset | F1 debug\n\n\
         To recompose: look away, then press B.\n\
         Looking through freezes without changing the light.",
        model.place,
        model.composition.labels(),
        model.composition.generation,
        model.player_observed,
        model.anchored,
        model.connection_locked(),
        if model.anchor_indicator_lit() {
            "ANCHORED"
        } else {
            "IDLE"
        },
        lab.sim.collider_count(),
        model.blame_attempts,
        model.blame_commits,
        lab.resets,
        lab.last_event,
    ));
}

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.004, 0.007, 0.014)))
        .insert_resource(Time::<Fixed>::from_seconds(DT as f64))
        .init_resource::<IntentBuffer>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Rapier Portal Composition Lab".into(),
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
    fn presentation_rebuild_keeps_preview_and_anchor_indicator_independent() {
        let modules = authored_modules();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(Lab {
                sim: PortalSimulation::new(3, modules),
                geometry_dirty: true,
                debug: true,
                resets: 0,
                last_event: String::new(),
            })
            .add_systems(Update, rebuild_geometry);
        app.update();
        let preview_count = app
            .world_mut()
            .query_filtered::<Entity, With<PreviewGeometry>>()
            .iter(app.world())
            .count();
        assert!(preview_count > 10);
        fn geometry_count(app: &mut App) -> usize {
            app.world_mut()
                .query_filtered::<Entity, With<LabGeometry>>()
                .iter(app.world())
                .count()
        }
        let before = geometry_count(&mut app);

        {
            let mut lab = app.world_mut().resource_mut::<Lab>();
            assert!(lab.sim.model.player_observed);
            assert!(!lab.sim.model.anchor_indicator_lit());
            lab.sim.toggle_anchor();
            lab.geometry_dirty = true;
        }
        app.update();
        assert_eq!(geometry_count(&mut app), before);
    }
}
