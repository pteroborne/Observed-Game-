use std::path::{Path, PathBuf};

use bevy::{
    app::AppExit,
    asset::{AssetPlugin, RenderAssetUsages},
    camera::Exposure,
    gltf::GltfAssetLabel,
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
use observed_style::{District, MarkerRole, SurfaceRole, district, marker};
use rapier_aesthetic_lab::visual::{
    SurfaceVisual, standard_material, surface_treatment, visual_for_hull,
};
use rapier_portal_lab::authoring::{ConvexHullData, authored_modules};
use rapier3d::prelude::{ColliderBuilder, Vector};

use crate::manifest::{ContentManifest, architecture_kind, file_fingerprint, style_district};

#[derive(Component)]
struct LabCamera;

#[derive(Component)]
struct LabScene;

#[derive(Component)]
struct SemanticGeometry;

#[derive(Component)]
struct ImportedNormalized;

#[derive(Component)]
struct Monitor;

#[derive(Resource)]
struct Lab {
    manifest: ContentManifest,
    manifest_hash: u64,
    active_district: usize,
    simulated_missing_asset: bool,
    dirty: bool,
    resets: u32,
    collider_count: usize,
    imported_meshes_normalized: usize,
    manifest_path: PathBuf,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("lab lives under labs")
        .parent()
        .expect("workspace root exists")
        .to_owned()
}

fn manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("content_manifest.json")
}

fn hull_mesh(hull: &ConvexHullData) -> Mesh {
    let points: Vec<Vector> = hull.points.iter().copied().map(Vector::from).collect();
    let collider = ColliderBuilder::convex_hull(&points)
        .expect("validated authored brush is convex")
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

fn setup(mut commands: Commands) {
    let path = manifest_path();
    let manifest = ContentManifest::from_path(&path).expect("content manifest parses");
    manifest
        .validate(&workspace_root())
        .expect("content manifest validates");
    let manifest_hash = manifest.deterministic_hash();

    commands.spawn((
        LabCamera,
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        Exposure::default(),
        Msaa::Off,
        DistanceFog::default(),
        Transform::from_xyz(0.0, 6.5, 10.5).looking_at(Vec3::new(0.0, 1.6, -11.0), Vec3::Y),
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(14.0),
            padding: UiRect::all(Val::Px(12.0)),
            max_width: Val::Px(510.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.004, 0.008, 0.018, 0.92)),
        children![(
            Monitor,
            Text::new("Content manifest loading"),
            TextFont {
                font_size: 15.0,
                ..default()
            },
            TextColor(Color::WHITE),
        )],
    ));
    commands.insert_resource(Lab {
        manifest,
        manifest_hash,
        active_district: 0,
        simulated_missing_asset: false,
        dirty: true,
        resets: 0,
        collider_count: 0,
        imported_meshes_normalized: 0,
        manifest_path: path,
    });
}

fn semantic_material(
    materials: &mut Assets<StandardMaterial>,
    role: SurfaceRole,
    district_role: District,
) -> Handle<StandardMaterial> {
    materials.add(standard_material(surface_treatment(SurfaceVisual {
        role,
        district: district_role,
    })))
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
    name: &'static str,
) {
    commands.spawn((
        LabScene,
        SemanticGeometry,
        Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
        MeshMaterial3d(material),
        Transform::from_translation(center),
        Name::new(name),
    ));
}

fn spawn_fallback(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    fallback: &str,
    district_role: District,
) {
    let material = semantic_material(materials, SurfaceRole::Wall, district_role);
    match fallback {
        "cable_bundle" => {
            for (index, y) in [0.35, 0.58, 0.81].into_iter().enumerate() {
                commands.spawn((
                    LabScene,
                    SemanticGeometry,
                    Mesh3d(meshes.add(Cylinder::new(0.07, 3.2))),
                    MeshMaterial3d(material.clone()),
                    Transform::from_xyz(index as f32 * 0.18 - 0.18, y, -4.0)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    Name::new("Procedural cable fallback"),
                ));
            }
        }
        "portal_frame" => {
            for x in [-1.4, 1.4] {
                spawn_box(
                    commands,
                    meshes,
                    material.clone(),
                    Vec3::new(x, 1.5, -4.0),
                    Vec3::new(0.24, 3.0, 0.35),
                    "Procedural gate fallback post",
                );
            }
            spawn_box(
                commands,
                meshes,
                material,
                Vec3::new(0.0, 2.9, -4.0),
                Vec3::new(3.05, 0.24, 0.35),
                "Procedural gate fallback lintel",
            );
        }
        other => panic!("validated fallback renderer is missing for `{other}`"),
    }
}

fn rebuild_scene(
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    existing: Query<Entity, With<LabScene>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut camera: Single<&mut DistanceFog, With<LabCamera>>,
) {
    if !lab.dirty {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    lab.imported_meshes_normalized = 0;

    let definition = lab.manifest.district(lab.active_district).clone();
    let district_role = style_district(&definition.style_district).expect("validated district");
    let palette = district(district_role);
    **camera = DistanceFog {
        color: palette.fog_color,
        falloff: FogFalloff::Linear {
            start: palette.fog_start,
            end: palette.fog_end * definition.fog_end_scale,
        },
        ..default()
    };
    commands.insert_resource(GlobalAmbientLight {
        color: palette.ambient_color,
        brightness: palette.ambient_brightness,
        ..default()
    });

    let module_definition = lab.manifest.module(&definition.architecture_module).clone();
    let kind = architecture_kind(&module_definition.architecture_kind).expect("validated kind");
    let module = authored_modules()
        .into_iter()
        .find(|module| module.kind == kind)
        .expect("authored module catalog agrees with manifest");
    lab.collider_count = 0;
    for hull in &module.hulls {
        let points: Vec<Vector> = hull.points.iter().copied().map(Vector::from).collect();
        let _collider =
            ColliderBuilder::convex_hull(&points).expect("manifest-selected brush remains convex");
        lab.collider_count += 1;
        let mut visual = visual_for_hull(kind, hull);
        visual.district = district_role;
        commands.spawn((
            LabScene,
            SemanticGeometry,
            Mesh3d(meshes.add(hull_mesh(hull))),
            MeshMaterial3d(materials.add(standard_material(surface_treatment(visual)))),
            Name::new(format!("Manifest-selected {} convex brush", kind.label())),
        ));
    }

    let slot = lab.manifest.asset(&definition.asset_slots[0]).clone();
    let full_path = workspace_root().join("assets").join(&slot.path);
    if !lab.simulated_missing_asset && full_path.is_file() {
        commands.spawn((
            LabScene,
            SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(slot.path.clone()))),
            Transform::from_xyz(0.0, 0.08, -4.0).with_scale(Vec3::splat(slot.scale)),
            Name::new(format!("CC0 dressing slot {}", slot.id)),
        ));
    } else {
        spawn_fallback(
            &mut commands,
            &mut meshes,
            &mut materials,
            &slot.fallback,
            district_role,
        );
    }

    let control = marker(MarkerRole::Control);
    commands.spawn((
        LabScene,
        SemanticGeometry,
        Mesh3d(meshes.add(Sphere::new(0.16))),
        MeshMaterial3d(materials.add(standard_material(control))),
        Transform::from_xyz(0.0, 3.45, -4.0),
        Name::new("Semantic control signal above dressing"),
    ));
    commands.spawn((
        LabScene,
        PointLight {
            color: palette.light_color,
            intensity: 210_000.0 * definition.practical_intensity_scale,
            range: 13.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-3.2, 4.8, -3.0),
        Name::new("Manifest-scaled district practical"),
    ));
    lab.dirty = false;
}

/// glTF materials are presentation input, not semantic authority. Every imported mesh
/// is replaced with the current district's shared Wall treatment after scene spawning.
fn normalize_imported_materials(
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    imported: Query<Entity, (Added<Mesh3d>, Without<SemanticGeometry>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let definition = lab.manifest.district(lab.active_district);
    let district_role = style_district(&definition.style_district).expect("validated district");
    let material = semantic_material(&mut materials, SurfaceRole::Wall, district_role);
    for entity in &imported {
        commands.entity(entity).insert((
            ImportedNormalized,
            MeshMaterial3d(material.clone()),
            Name::new("CC0 mesh normalized through observed_style"),
        ));
        lab.imported_meshes_normalized += 1;
    }
}

fn input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut lab: ResMut<Lab>,
    mut exit: MessageWriter<AppExit>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    if keyboard.just_pressed(KeyCode::Tab) {
        lab.active_district = (lab.active_district + 1) % lab.manifest.districts.len();
        lab.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyM) {
        lab.simulated_missing_asset = !lab.simulated_missing_asset;
        lab.dirty = true;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        lab.resets += 1;
        lab.dirty = true;
    }
}

fn update_monitor(lab: Res<Lab>, mut text: Single<&mut Text, With<Monitor>>) {
    let definition = lab.manifest.district(lab.active_district);
    let module = lab.manifest.module(&definition.architecture_module);
    let asset = lab.manifest.asset(&definition.asset_slots[0]);
    let full_path = workspace_root().join("assets").join(&asset.path);
    let asset_status = if lab.simulated_missing_asset || !full_path.is_file() {
        format!("FALLBACK ({})", asset.fallback)
    } else {
        format!(
            "IMPORTED + NORMALIZED ({} meshes)",
            lab.imported_meshes_normalized
        )
    };
    let fingerprint = full_path
        .is_file()
        .then(|| file_fingerprint(&full_path).ok())
        .flatten()
        .unwrap_or_else(|| "missing".into());
    **text = Text::new(format!(
        "CONTENT MANIFEST LAB  [PASS]\n\n\
         manifest      {}\n\
         schema/hash   v{} / {:016x}\n\
         district      {}\n\
         style owner   observed_style::{}\n\
         architecture  {} (weight {})\n\
         typed ports   {}\n\
         Rapier hulls  {} convex colliders\n\n\
         dressing      {}\n\
         state         {}\n\
         source        {} / {}\n\
         license       {}\n\
         fingerprint   {}\n\n\
         Control orb is semantic signal-tier; imported art is not.\n\
         TAB district | M simulate missing asset | R reset | Esc quit",
        lab.manifest_path.display(),
        lab.manifest.schema_version,
        lab.manifest_hash,
        definition.label,
        definition.style_district,
        module.id,
        module.wfc_weight,
        module.ports.join(" <-> "),
        lab.collider_count,
        asset.id,
        asset_status,
        asset.author,
        asset.source_url,
        asset.license,
        fingerprint,
    ));
}

#[derive(Resource)]
struct CaptureRun {
    directory: String,
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
    if capture.phase == 0 && capture.frames >= 75 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(format!(
                "{}/content_manifest_archive.png",
                capture.directory
            )));
        capture.phase = 1;
    } else if capture.phase == 1 && capture.frames >= 100 {
        lab.active_district = 1;
        lab.dirty = true;
        capture.phase = 2;
    } else if capture.phase == 2 && capture.frames >= 175 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(format!(
                "{}/content_manifest_reactor.png",
                capture.directory
            )));
        capture.phase = 3;
    } else if capture.phase == 3 && capture.frames >= 230 {
        exit.write(AppExit::Success);
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.003, 0.005, 0.012)))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "../../assets".into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Observed 2 — Content Manifest Lab".into(),
                        resolution: WindowResolution::new(1440, 900),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                input,
                rebuild_scene,
                normalize_imported_materials,
                update_monitor,
            )
                .chain(),
        );
    if let Ok(directory) = std::env::var("OBSERVED2_CAPTURE") {
        std::fs::create_dir_all(&directory).expect("create content-manifest evidence directory");
        app.insert_resource(CaptureRun {
            directory,
            frames: 0,
            phase: 0,
        })
        .add_systems(Update, capture_progress.after(update_monitor));
    }
    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_manifest_selected_brush_still_builds_a_rapier_convex_collider() {
        let manifest = ContentManifest::from_path(&manifest_path()).unwrap();
        manifest.validate(&workspace_root()).unwrap();
        let modules = authored_modules();
        for district in &manifest.districts {
            let definition = manifest.module(&district.architecture_module);
            let kind = architecture_kind(&definition.architecture_kind).unwrap();
            let module = modules.iter().find(|module| module.kind == kind).unwrap();
            assert!(!module.hulls.is_empty());
            for hull in &module.hulls {
                let points: Vec<Vector> = hull.points.iter().copied().map(Vector::from).collect();
                assert!(ColliderBuilder::convex_hull(&points).is_some());
            }
        }
    }

    #[test]
    fn each_committed_asset_has_a_known_procedural_projection() {
        let manifest = ContentManifest::from_path(&manifest_path()).unwrap();
        for asset in manifest.asset_packs {
            assert!(matches!(
                asset.fallback.as_str(),
                "cable_bundle" | "portal_frame"
            ));
        }
    }

    #[test]
    fn pack_assets_are_dressing_only_and_never_architecture_sources() {
        let manifest = ContentManifest::from_path(&manifest_path()).unwrap();
        for module in &manifest.architecture_modules {
            assert!(module.source_path.ends_with(".map"));
        }
        for asset in &manifest.asset_packs {
            assert!(asset.path.ends_with(".glb"));
            assert!(
                !manifest
                    .architecture_modules
                    .iter()
                    .any(|module| module.source_path == asset.path)
            );
        }
    }
}
