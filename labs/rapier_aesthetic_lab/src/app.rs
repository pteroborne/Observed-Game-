use bevy::{
    app::AppExit,
    asset::RenderAssetUsages,
    camera::Exposure,
    mesh::{Indices, PrimitiveTopology},
    pbr::{DistanceFog, FogFalloff},
    post_process::bloom::Bloom,
    prelude::*,
    render::view::{
        Hdr,
        screenshot::{Screenshot, save_to_disk},
    },
    window::{PresentMode, WindowResolution},
};
use observed_style::{District, MarkerRole, SurfaceRole, Treatment, district, marker};
use player_input::PlayerIntent;
use rapier_portal_lab::{
    authoring::{ArchitectureKind, AuthoredModule, ConvexHullData, authored_modules},
    model::{Place, SOURCE_THRESHOLD_Z},
    physics::{DT, PortalSimulation},
};
use rapier3d::prelude::{ColliderBuilder, Vector};

use crate::visual::{
    SurfaceVisual, palette_for, standard_material, surface_treatment, threshold_indicator,
    visual_for_hull,
};

#[derive(Component)]
struct StyledGeometry;

#[derive(Component)]
struct PreviewGeometry;

#[derive(Component)]
struct LabCamera;

#[derive(Component)]
struct Monitor;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
enum VisualMeaning {
    Surface(SurfaceRole),
    Marker(MarkerRole),
    DistrictAccent,
    ThresholdIdle,
}

#[derive(Resource, Default)]
struct IntentBuffer(PlayerIntent);

#[derive(Resource)]
struct Lab {
    sim: PortalSimulation,
    geometry_dirty: bool,
    overlay: bool,
    resets: u32,
    last_event: String,
}

fn hull_mesh(hull: &ConvexHullData) -> Mesh {
    let points: Vec<Vector> = hull.points.iter().copied().map(Vector::from).collect();
    let collider = ColliderBuilder::convex_hull(&points)
        .expect("authored brush is convex")
        .build();
    let poly = collider
        .shape()
        .as_convex_polyhedron()
        .expect("convex hull builder returns polyhedron topology");
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

fn setup(mut commands: Commands, capture: Option<Res<CaptureRun>>) {
    let archive = district(District::Archive);
    commands.spawn((
        LabCamera,
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        Exposure::default(),
        Msaa::Off,
        DistanceFog {
            color: archive.fog_color,
            falloff: FogFalloff::Linear {
                start: archive.fog_start,
                end: archive.fog_end,
            },
            ..default()
        },
        Transform::default(),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: archive.ambient_color,
        brightness: archive.ambient_brightness,
        ..default()
    });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(14.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.005, 0.01, 0.022, 0.90)),
        children![(
            Monitor,
            Text::new("Authored neon-noir recreation"),
            TextFont {
                font_size: 15.0,
                ..default()
            },
            TextColor(Color::WHITE),
        )],
    ));
    let mut sim = PortalSimulation::new(0xA357_E71C, authored_modules());
    if capture.is_some() {
        sim.toggle_anchor();
    }
    commands.insert_resource(Lab {
        sim,
        geometry_dirty: true,
        overlay: true,
        resets: 0,
        last_event: "Archive threshold observes a remote authored composition.".into(),
    });
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
    size: Vec3,
    visual: SurfaceVisual,
    name: &'static str,
) {
    commands.spawn((
        StyledGeometry,
        VisualMeaning::Surface(visual.role),
        Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
        MeshMaterial3d(materials.add(standard_material(surface_treatment(visual)))),
        Transform::from_translation(center),
        Name::new(name),
    ));
}

fn archive_visual(role: SurfaceRole) -> SurfaceVisual {
    SurfaceVisual {
        role,
        district: District::Archive,
    }
}

fn spawn_source_room(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    source: bool,
) {
    spawn_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, -0.2, 0.0),
        Vec3::new(16.0, 0.4, 16.0),
        archive_visual(SurfaceRole::Plain),
        "Archive room floor",
    );
    for x in [-8.2, 8.2] {
        spawn_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 2.5, 0.0),
            Vec3::new(0.4, 5.0, 16.4),
            archive_visual(SurfaceRole::Wall),
            "Archive room wall",
        );
    }
    let solid_z = if source { 8.2 } else { -8.2 };
    spawn_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, 2.5, solid_z),
        Vec3::new(16.4, 5.0, 0.4),
        archive_visual(SurfaceRole::Wall),
        "Archive room back wall",
    );
    for x in [-5.2, 5.2] {
        spawn_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 2.5, -solid_z),
            Vec3::new(5.6, 5.0, 0.4),
            archive_visual(SurfaceRole::Wall),
            "Split threshold wall",
        );
    }
    let palette = district(District::Archive);
    let accent = Treatment {
        base_color: Color::BLACK,
        emissive: palette.accent,
        signal: false,
        edge: None,
    };
    let accent_material = materials.add(standard_material(accent));
    for x in [-7.95, 7.95] {
        commands.spawn((
            StyledGeometry,
            VisualMeaning::DistrictAccent,
            Mesh3d(meshes.add(Cuboid::new(0.07, 0.07, 15.6))),
            MeshMaterial3d(accent_material.clone()),
            Transform::from_xyz(x, 0.12, 0.0),
            Name::new("Archive baseboard seam"),
        ));
    }
    commands.spawn((
        StyledGeometry,
        VisualMeaning::DistrictAccent,
        Mesh3d(meshes.add(Cuboid::new(15.6, 0.07, 0.07))),
        MeshMaterial3d(accent_material.clone()),
        Transform::from_xyz(0.0, 0.12, solid_z.signum() * 7.95),
        Name::new("Archive back-wall seam"),
    ));
    for x in [-5.2, 5.2] {
        commands.spawn((
            StyledGeometry,
            VisualMeaning::DistrictAccent,
            Mesh3d(meshes.add(Cuboid::new(5.5, 0.07, 0.07))),
            MeshMaterial3d(accent_material.clone()),
            Transform::from_xyz(x, 0.12, -solid_z.signum() * 7.95),
            Name::new("Archive threshold-wall seam"),
        ));
    }
}

fn spawn_threshold(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    anchored: bool,
) {
    for x in [-2.4, 2.4] {
        spawn_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 2.5, SOURCE_THRESHOLD_Z),
            Vec3::new(0.22, 5.0, 0.22),
            archive_visual(SurfaceRole::Wall),
            "Threshold post",
        );
    }
    spawn_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, 4.82, SOURCE_THRESHOLD_Z),
        Vec3::new(5.0, 0.28, 0.22),
        archive_visual(SurfaceRole::Wall),
        "Threshold lintel",
    );
    let treatment = threshold_indicator(anchored);
    commands.spawn((
        StyledGeometry,
        if anchored {
            VisualMeaning::Marker(MarkerRole::Control)
        } else {
            VisualMeaning::ThresholdIdle
        },
        Mesh3d(meshes.add(Cuboid::new(1.2, 0.14, 0.12))),
        MeshMaterial3d(materials.add(standard_material(treatment))),
        Transform::from_xyz(0.0, 4.54, SOURCE_THRESHOLD_Z + 0.14),
        Name::new("Anchor-only frame light"),
    ));
    commands.spawn((
        PointLight {
            color: treatment.base_color,
            intensity: if anchored { 9_000.0 } else { 1_000.0 },
            range: 5.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 4.4, SOURCE_THRESHOLD_Z + 0.25),
        Name::new("Frame light spill"),
    ));
}

fn spawn_source_key(commands: &mut Commands) {
    let palette = district(District::Archive);
    commands.spawn((
        StyledGeometry,
        SpotLight {
            color: palette.key_color,
            intensity: palette.key_intensity * 0.08,
            range: 34.0,
            radius: palette.key_radius,
            inner_angle: 0.32,
            outer_angle: 0.72,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-4.5, 6.5, 3.0)
            .looking_at(Vec3::new(0.0, 1.4, SOURCE_THRESHOLD_Z - 5.0), Vec3::Y),
        Name::new("Archive threshold key"),
    ));
}

fn spawn_hull(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    module: &AuthoredModule,
    hull: &ConvexHullData,
    z_offset: f32,
    preview: bool,
) {
    let visual = visual_for_hull(module.kind, hull);
    let mut entity = commands.spawn((
        StyledGeometry,
        VisualMeaning::Surface(visual.role),
        Mesh3d(meshes.add(hull_mesh(hull))),
        MeshMaterial3d(materials.add(standard_material(surface_treatment(visual)))),
        Transform::from_xyz(0.0, 0.0, z_offset),
        Name::new(format!("Styled {} brush", module.kind.label())),
    ));
    if preview {
        entity.insert(PreviewGeometry);
    }
}

fn spawn_module_lighting(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    kind: ArchitectureKind,
    z_offset: f32,
    length: f32,
    preview: bool,
) {
    let palette = palette_for(kind);
    match kind {
        ArchitectureKind::Gantry => {
            // BLAME register: one distant pale key and silhouette crossmembers.
            commands.spawn((
                StyledGeometry,
                SpotLight {
                    color: palette.key_color,
                    intensity: palette.key_intensity * 0.12,
                    range: palette.key_range,
                    radius: palette.key_radius,
                    inner_angle: palette.key_inner_angle,
                    outer_angle: palette.key_outer_angle,
                    shadows_enabled: !preview,
                    ..default()
                },
                Transform::from_xyz(4.0, 9.0, z_offset - length * 0.82)
                    .looking_at(Vec3::new(0.0, 0.8, z_offset - length * 0.35), Vec3::Y),
                Name::new("Gantry distant pale key"),
            ));
            for (x, tilt) in [(-5.8, 0.32), (5.8, -0.28)] {
                let transform = Transform::from_xyz(x, 5.5, z_offset - length * 0.52)
                    .with_rotation(Quat::from_rotation_z(tilt));
                let visual = archive_visual(SurfaceRole::Wall);
                commands.spawn((
                    StyledGeometry,
                    VisualMeaning::Surface(visual.role),
                    Mesh3d(meshes.add(Cuboid::new(0.55, 12.0, 0.55))),
                    MeshMaterial3d(materials.add(standard_material(surface_treatment(visual)))),
                    transform,
                    Name::new("BLAME silhouette strut"),
                ));
            }
        }
        ArchitectureKind::Colonnade => {
            // Silo register: separated warm practical pools, darkness between them.
            let practical = marker(MarkerRole::NextRoom);
            for (index, fraction) in [0.18_f32, 0.48, 0.78].into_iter().enumerate() {
                let x = if index % 2 == 0 { -4.2 } else { 4.2 };
                let position = Vec3::new(x, 1.75, z_offset - length * fraction);
                commands.spawn((
                    StyledGeometry,
                    VisualMeaning::Marker(MarkerRole::NextRoom),
                    Mesh3d(meshes.add(Cuboid::new(0.18, 0.3, 0.18))),
                    MeshMaterial3d(materials.add(standard_material(practical))),
                    Transform::from_translation(position),
                    Name::new("Warm caged practical"),
                ));
                commands.spawn((
                    PointLight {
                        color: palette.light_color,
                        intensity: 260_000.0,
                        range: 7.5,
                        shadows_enabled: !preview && index == 0,
                        ..default()
                    },
                    Transform::from_translation(position),
                    Name::new("Colonnade practical pool"),
                ));
            }
        }
    }
}

fn spawn_composition(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    lab: &Lab,
    base_z: f32,
    preview: bool,
) {
    for placement in &lab.sim.model.composition.placements {
        let module = lab
            .sim
            .modules()
            .iter()
            .find(|module| module.kind == placement.kind)
            .expect("WFC tile exists in authored catalog");
        let offset = base_z + placement.z_offset;
        for hull in &module.hulls {
            spawn_hull(commands, meshes, materials, module, hull, offset, preview);
            if module.kind == ArchitectureKind::Gantry && hull.max.y > 1.5 {
                let center = (hull.min + hull.max) * 0.5;
                let size = hull.max - hull.min;
                for x in [hull.min.x, hull.max.x] {
                    spawn_box(
                        commands,
                        meshes,
                        materials,
                        Vec3::new(x, hull.max.y + 0.04, center.z + offset),
                        Vec3::new(0.08, 0.08, size.z.max(0.2)),
                        SurfaceVisual {
                            role: SurfaceRole::GantryEdge,
                            district: District::Archive,
                        },
                        "Gantry commitment edge",
                    );
                }
            }
        }
        spawn_module_lighting(
            commands,
            meshes,
            materials,
            placement.kind,
            offset,
            placement.length,
            preview,
        );
    }
}

fn atmosphere_for(lab: &Lab) -> observed_style::DistrictPalette {
    match lab.sim.model.place {
        Place::SourceRoom => district(District::Archive),
        Place::Hallway => palette_for(lab.sim.model.composition.placements[0].kind),
        Place::DestinationRoom => district(District::Reactor),
    }
}

fn rebuild_scene(
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    existing: Query<Entity, With<StyledGeometry>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut camera: Single<&mut DistanceFog, With<LabCamera>>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    if !lab.geometry_dirty {
        return;
    }
    lab.geometry_dirty = false;
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let palette = atmosphere_for(&lab);
    ambient.color = palette.ambient_color;
    ambient.brightness = palette.ambient_brightness;
    camera.color = palette.fog_color;
    camera.falloff = FogFalloff::Linear {
        start: palette.fog_start,
        end: palette.fog_end,
    };
    match lab.sim.model.place {
        Place::SourceRoom => {
            spawn_source_room(&mut commands, &mut meshes, &mut materials, true);
            spawn_source_key(&mut commands);
            spawn_threshold(
                &mut commands,
                &mut meshes,
                &mut materials,
                lab.sim.model.anchor_indicator_lit(),
            );
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
        Place::DestinationRoom => {
            spawn_source_room(&mut commands, &mut meshes, &mut materials, false)
        }
    }
}

fn sample_input(keys: Res<ButtonInput<KeyCode>>, mut intent: ResMut<IntentBuffer>) {
    let axis =
        |negative, positive| (keys.pressed(positive) as i32 - keys.pressed(negative) as i32) as f32;
    intent.0 = PlayerIntent {
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
    let place = lab.sim.model.place;
    lab.sim.step(intent.0);
    intent.0.jump_pressed = false;
    if lab.sim.model.place != place {
        lab.geometry_dirty = true;
        lab.last_event = format!(
            "Crossed into {:?}; atmosphere followed the Place.",
            lab.sim.model.place
        );
    }
}

fn toggles(keys: Res<ButtonInput<KeyCode>>, mut lab: ResMut<Lab>) {
    if keys.just_pressed(KeyCode::KeyB) {
        if lab.sim.request_blame() {
            lab.geometry_dirty = true;
            lab.last_event = "BLAME recomposed the unseen authored registers.".into();
        } else {
            lab.last_event = "BLAME held by player observation or anchor.".into();
        }
    }
    if keys.just_pressed(KeyCode::KeyE) {
        lab.sim.toggle_anchor();
        lab.geometry_dirty = true;
        lab.last_event = if lab.sim.model.anchored {
            "Anchor applied: Control-purple frame signal is now durable.".into()
        } else {
            "Anchor removed: idle frame restored; gaze may still freeze.".into()
        };
    }
    if keys.just_pressed(KeyCode::KeyR) {
        lab.sim = PortalSimulation::new(0xA357_E71C, authored_modules());
        lab.geometry_dirty = true;
        lab.resets += 1;
        lab.last_event = "Aesthetic scene reset.".into();
    }
    if keys.just_pressed(KeyCode::F1) {
        lab.overlay = !lab.overlay;
    }
}

fn present_camera(
    lab: Res<Lab>,
    capture: Option<Res<CaptureRun>>,
    mut camera: Single<&mut Transform, With<LabCamera>>,
) {
    if capture.is_some() && lab.sim.model.place == Place::SourceRoom {
        **camera = Transform::from_xyz(0.0, 1.65, 2.6)
            .looking_at(Vec3::new(0.0, 1.65, SOURCE_THRESHOLD_Z - 14.0), Vec3::Y);
        return;
    }
    let player = lab.sim.player();
    let (sin_yaw, cos_yaw) = player.yaw.sin_cos();
    let cos_pitch = player.pitch.cos();
    camera.translation = Vec3::new(
        player.position.x,
        player.position.y + 0.55,
        player.position.z,
    );
    camera.look_to(
        Vec3::new(
            -sin_yaw * cos_pitch,
            player.pitch.sin(),
            -cos_yaw * cos_pitch,
        ),
        Vec3::Y,
    );
}

fn update_monitor(
    lab: Res<Lab>,
    capture: Option<Res<CaptureRun>>,
    mut panel: Single<(&mut Text, &mut Visibility), With<Monitor>>,
) {
    let (text, visibility) = &mut *panel;
    **visibility = if lab.overlay && capture.is_none() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let model = &lab.sim.model;
    **text = Text::new(format!(
        "AUTHORED NEON-NOIR RECREATION\n\n\
         composition   {}\n\
         active Place  {:?}\n\
         player lock   {}\n\
         anchor lock   {}\n\
         frame signal  {} (anchor only)\n\
         atmosphere    {}\n\
         resets        {}\n\n\
         {}\n\n\
         Archive Gantry: cool mass + amber commitment edges\n\
         Reactor Colonnade: warm isolated practical pools\n\
         HDR + bloom + district fog; signals stay semantic\n\n\
         WASD/Shift/Space | arrows | B BLAME | E anchor | R reset",
        model.composition.labels(),
        model.place,
        model.player_observed,
        model.anchored,
        if model.anchor_indicator_lit() {
            "CONTROL"
        } else {
            "IDLE"
        },
        match model.place {
            Place::SourceRoom => "archive",
            Place::Hallway => model.composition.placements[0].kind.label(),
            Place::DestinationRoom => "reactor",
        },
        lab.resets,
        lab.last_event,
    ));
}

#[derive(Resource)]
struct CaptureRun {
    dir: String,
    frames: u32,
    phase: u8,
}

fn capture_progress(
    mut capture: ResMut<CaptureRun>,
    mut lab: ResMut<Lab>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    capture.frames += 1;
    if capture.phase == 0 && capture.frames >= 45 {
        let path = format!("{}/rapier_aesthetic_gantry.png", capture.dir);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        capture.phase = 1;
    } else if capture.phase == 1 && capture.frames >= 75 {
        // Temporarily release both locks, let BLAME collapse the opposite WFC order,
        // then restore gaze + anchor so only the architecture changes between frames.
        lab.sim.toggle_anchor();
        lab.sim
            .set_player_pose_for_test(Vector::new(0.0, 0.93, 2.5), std::f32::consts::PI);
        assert!(lab.sim.request_blame());
        lab.sim
            .set_player_pose_for_test(Vector::new(0.0, 0.93, 2.5), 0.0);
        lab.sim.toggle_anchor();
        lab.geometry_dirty = true;
        capture.phase = 2;
    } else if capture.phase == 2 && capture.frames >= 115 {
        let path = format!("{}/rapier_aesthetic_colonnade.png", capture.dir);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
        capture.phase = 3;
    } else if capture.phase == 3 && capture.frames >= 175 {
        exit.write(AppExit::Success);
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.003, 0.005, 0.012)))
        .insert_resource(Time::<Fixed>::from_seconds(DT as f64))
        .init_resource::<IntentBuffer>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Authored Neon-Noir Recreation".into(),
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
                sample_input,
                toggles,
                rebuild_scene,
                present_camera,
                update_monitor,
            )
                .chain(),
        )
        .add_systems(FixedUpdate, simulate);
    if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
        std::fs::create_dir_all(&dir).expect("create aesthetic evidence directory");
        app.insert_resource(CaptureRun {
            dir,
            frames: 0,
            phase: 0,
        })
        .add_systems(Update, capture_progress.after(rebuild_scene));
    }
    app.run();
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetPlugin;

    use super::*;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(GlobalAmbientLight::default())
            .add_systems(Startup, setup)
            .add_systems(Update, rebuild_scene);
        app.update();
        app
    }

    #[test]
    fn camera_carries_the_proven_hdr_bloom_and_fog_stack() {
        let mut app = test_app();
        let count = app
            .world_mut()
            .query_filtered::<Entity, (With<LabCamera>, With<Hdr>, With<Bloom>, With<DistanceFog>)>(
            )
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn every_styled_mesh_has_a_semantic_visual_meaning() {
        let mut app = test_app();
        let meshes = app
            .world_mut()
            .query_filtered::<Entity, (With<StyledGeometry>, With<Mesh3d>)>()
            .iter(app.world())
            .count();
        let meanings = app
            .world_mut()
            .query_filtered::<Entity, (With<StyledGeometry>, With<Mesh3d>, With<VisualMeaning>)>()
            .iter(app.world())
            .count();
        assert_eq!(meshes, meanings);
    }

    #[test]
    fn anchor_rebuild_changes_signal_without_leaking_styled_geometry() {
        let mut app = test_app();
        fn count(app: &mut App) -> usize {
            app.world_mut()
                .query_filtered::<Entity, With<StyledGeometry>>()
                .iter(app.world())
                .count()
        }
        let before = count(&mut app);
        {
            let mut lab = app.world_mut().resource_mut::<Lab>();
            assert!(lab.sim.model.player_observed);
            assert!(!lab.sim.model.anchor_indicator_lit());
            lab.sim.toggle_anchor();
            lab.geometry_dirty = true;
        }
        app.update();
        assert_eq!(count(&mut app), before);
        assert!(
            app.world()
                .resource::<Lab>()
                .sim
                .model
                .anchor_indicator_lit()
        );
    }
}
