//! **style_lab** — the neon-noir visual language for Observed 2, proven in
//! isolation. It renders every semantic role from the pure [`observed_style`] crate
//! as a neon-noir swatch (surfaces, signal markers, and observed states) with an
//! on-screen legend, so the look can be aligned and screenshot-verified before any
//! lab or the assembled `game` adopts it.
//!
//! The technical question: *can a code-only visual language (no textures/meshes —
//! just primitives + colour/emission/light/fog) render every gameplay role
//! distinctly and keep signals legible through neon-noir fog/bloom?* The pure rules
//! live in [`observed_style`] and are unit-tested there; this lab is their
//! projection.

// The pure rules were promoted into `crates/observed_style` (refactor R1). Keep the
// familiar `style::` spelling in this presentation code by aliasing the crate.
use observed_style as style;

use bevy::{
    app::AppExit,
    input::InputSystems,
    pbr::{DistanceFog, FogFalloff},
    post_process::bloom::Bloom,
    prelude::*,
    render::view::{
        Hdr,
        screenshot::{Screenshot, save_to_disk},
    },
    window::{PresentMode, WindowResolution},
};

use observed_style::{DoorIdentityRole, MarkerRole, ObservedState, SurfaceRole, Treatment};

/// Marks the showcase camera.
#[derive(Component)]
pub(crate) struct StyleCam;

/// Every entity the lab spawns, so a reset can clear and rebuild the scene without
/// restarting the app and without leaking.
#[derive(Component)]
pub(crate) struct StyleSpawned;

/// Root of the overlay UI.
#[derive(Component)]
pub(crate) struct StyleUiRoot;

/// A panel that the F1 key toggles.
#[derive(Component)]
pub(crate) struct OverlayPanel;

#[derive(Component)]
pub(crate) struct DebugText;

/// A neon edge to draw as a wireframe outline of the entity's box.
#[derive(Component)]
pub(crate) struct NeonEdge {
    color: Color,
}

pub struct StyleLabPlugin;

impl Plugin for StyleLabPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (
                reset_input.after(InputSystems),
                toggle_overlay,
                draw_edges,
                update_debug_text,
            ),
        );
    }
}

fn material_for(t: &Treatment) -> StandardMaterial {
    StandardMaterial {
        base_color: t.base_color,
        emissive: t.emissive,
        perceptual_roughness: 0.7,
        ..default()
    }
}

fn place_tile(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    t: &Treatment,
    position: Vec3,
) {
    let mut entity = commands.spawn((
        StyleSpawned,
        Mesh3d(mesh.clone()),
        MeshMaterial3d(materials.add(material_for(t))),
        Transform::from_translation(position).with_scale(Vec3::new(2.6, 0.25, 2.6)),
        Name::new("Surface swatch"),
    ));
    if let Some(color) = t.edge {
        entity.insert(NeonEdge { color });
    }
}

fn place_beacon(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    t: &Treatment,
    position: Vec3,
) {
    let beacon = commands
        .spawn((
            StyleSpawned,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(materials.add(material_for(t))),
            Transform::from_translation(position).with_scale(Vec3::new(0.55, 2.2, 0.55)),
            Name::new("Marker beacon"),
        ))
        .id();
    if let Some(color) = t.edge {
        commands.entity(beacon).insert(NeonEdge { color });
    }
    if t.signal {
        // Spill the marker's neon onto the dark floor so it reads as a light source.
        let srgb = t.base_color.to_srgba();
        commands.entity(beacon).with_children(|parent| {
            parent.spawn((
                PointLight {
                    color: Color::srgb(srgb.red, srgb.green, srgb.blue),
                    intensity: 9_000.0,
                    range: 9.0,
                    shadows_enabled: false,
                    ..default()
                },
                // Child transform is in the parent's scaled space (y-scale 2.2), so a
                // small local offset lifts the light to just above the beacon.
                Transform::from_xyz(0.0, 0.7, 0.0),
            ));
        });
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Neon-noir camera: HDR + bloom so the emissive neon glows, distance fog over a
    // near-black scene for atmosphere.
    commands.spawn((
        StyleCam,
        StyleSpawned,
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        DistanceFog {
            color: Color::srgb(0.015, 0.02, 0.05),
            falloff: FogFalloff::Linear {
                start: 14.0,
                end: 62.0,
            },
            ..default()
        },
        Transform::from_xyz(0.0, 9.5, 17.0).looking_at(Vec3::new(0.0, 0.6, -2.0), Vec3::Y),
        Name::new("Style Showcase Camera"),
    ));
    // A dim key light and low ambient — the emissive neon does most of the lighting.
    commands.spawn((
        StyleSpawned,
        DirectionalLight {
            illuminance: 1_500.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.4, 0.0)),
        Name::new("Key light"),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.4, 0.5, 0.7),
        brightness: 60.0,
        ..default()
    });

    let unit_cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let backdrop = meshes.add(Plane3d::default().mesh().size(60.0, 60.0));

    commands.spawn((
        StyleSpawned,
        Mesh3d(backdrop),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.012, 0.016, 0.03),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.15, 0.0),
        Name::new("Backdrop"),
    ));

    let spacing = 3.4;
    let row_x = |i: usize, n: usize| (i as f32 - (n as f32 - 1.0) * 0.5) * spacing;

    // Row 1 — structural surfaces.
    for (i, role) in SurfaceRole::ALL.iter().enumerate() {
        let t = style::surface(*role);
        place_tile(
            &mut commands,
            &unit_cube,
            &mut materials,
            &t,
            Vec3::new(row_x(i, SurfaceRole::ALL.len()), 0.0, -1.0),
        );
    }

    // Row 2 (front) — gameplay markers / signals.
    for (i, role) in MarkerRole::ALL.iter().enumerate() {
        let t = style::marker(*role);
        place_beacon(
            &mut commands,
            &unit_cube,
            &mut materials,
            &t,
            Vec3::new(row_x(i, MarkerRole::ALL.len()), 1.1, 3.2),
        );
    }

    // Row 3 (front-most) - doorframe identity reads.
    for (i, role) in DoorIdentityRole::ALL.iter().enumerate() {
        let t = style::door_identity(*role);
        place_tile(
            &mut commands,
            &unit_cube,
            &mut materials,
            &t,
            Vec3::new(row_x(i, DoorIdentityRole::ALL.len()), 0.0, 6.8),
        );
    }

    // Row 4 (back) - the spine surface in each observed state.
    let spine = style::surface(SurfaceRole::Spine);
    for (i, state) in ObservedState::ALL.iter().enumerate() {
        let t = style::observed_modulate(spine, *state);
        place_tile(
            &mut commands,
            &unit_cube,
            &mut materials,
            &t,
            Vec3::new(row_x(i, ObservedState::ALL.len()), 0.0, -5.4),
        );
    }

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    let mut legend = String::from("NEON-NOIR VISUAL LANGUAGE\n\nSURFACES (middle row, L→R):\n");
    for role in SurfaceRole::ALL {
        legend.push_str(&format!("  - {}\n", role.label()));
    }
    legend.push_str("\nMARKERS / SIGNALS (front row, L→R):\n");
    for role in MarkerRole::ALL {
        legend.push_str(&format!("  - {}\n", role.label()));
    }
    legend.push_str("\nDOORFRAME READS (front-most row, L→R):\n");
    for role in DoorIdentityRole::ALL {
        legend.push_str(&format!(
            "  - {} [{}; {}]\n",
            role.label(),
            role.glyph(),
            role.ambience_label()
        ));
    }
    legend.push_str("\nOBSERVED STATES (back row — spine):\n");
    for state in ObservedState::ALL {
        legend.push_str(&format!("  - {}\n", state.label()));
    }

    commands
        .spawn((
            StyleUiRoot,
            StyleSpawned,
            Node {
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(16)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            GlobalZIndex(20),
            Name::new("Style Lab UI Root"),
        ))
        .with_children(|root| {
            root.spawn((
                OverlayPanel,
                Node {
                    width: px(340),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.3, 0.9, 1.0, 0.6)),
                children![(
                    DebugText,
                    Text::new("…"),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.95, 1.0)),
                )],
            ));
            root.spawn((
                OverlayPanel,
                Node {
                    width: px(340),
                    padding: UiRect::all(px(14)),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.94)),
                BorderColor::all(Color::srgba(0.3, 0.9, 1.0, 0.6)),
                children![(
                    Text::new(legend),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.94, 0.98)),
                )],
            ));
        });
}

fn draw_edges(mut gizmos: Gizmos, edges: Query<(&GlobalTransform, &NeonEdge)>) {
    // The 8 corners of a unit cube and the 12 edges connecting them. Drawing the
    // outline with plain lines keeps the neon edge independent of any one Bevy
    // version's box-gizmo helper.
    let corners = [
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, 0.5),
        Vec3::new(-0.5, -0.5, 0.5),
        Vec3::new(-0.5, 0.5, -0.5),
        Vec3::new(0.5, 0.5, -0.5),
        Vec3::new(0.5, 0.5, 0.5),
        Vec3::new(-0.5, 0.5, 0.5),
    ];
    let lines = [
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
    ];
    for (transform, edge) in &edges {
        let world: Vec<Vec3> = corners
            .iter()
            .map(|c| transform.transform_point(*c))
            .collect();
        for (a, b) in lines {
            gizmos.line(world[a], world[b], edge.color);
        }
    }
}

fn reset_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    spawned: Query<Entity, With<StyleSpawned>>,
    mut commands: Commands,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        for entity in &spawned {
            commands.entity(entity).despawn();
        }
        commands.run_system_cached(setup);
    }
}

fn toggle_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panels: Query<&mut Visibility, With<OverlayPanel>>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        for mut visibility in &mut panels {
            *visibility = if *visibility == Visibility::Hidden {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

fn update_debug_text(
    cams: Query<(), With<StyleCam>>,
    ui_roots: Query<(), With<StyleUiRoot>>,
    edges: Query<(), With<NeonEdge>>,
    mut text: Query<&mut Text, With<DebugText>>,
) {
    let cams = cams.iter().count();
    let ui_roots = ui_roots.iter().count();
    let healthy = cams == 1 && ui_roots == 1;
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    **text = format!(
        "STYLE LAB  {}\n\n\
         neon-noir visual language\n\
         surfaces {} | markers {} | door reads {} | observed {}\n\
         neon edges {}\n\
         signal min luminance {:.1}\n\n\
         cameras {cams}  UI {ui_roots}\n\n\
         R reset · F1 toggle overlay",
        if healthy { "[PASS]" } else { "[FAIL]" },
        SurfaceRole::ALL.len(),
        MarkerRole::ALL.len(),
        DoorIdentityRole::ALL.len(),
        ObservedState::ALL.len(),
        edges.iter().count(),
        style::SIGNAL_MIN_LUMINANCE,
    );
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.006, 0.009, 0.018)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Neon-Noir Style Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(StyleLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 && elapsed >= 0.8 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 1.6 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::AssetPlugin, gizmos::GizmoPlugin, input::InputPlugin};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            InputPlugin,
            GizmoPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(StyleLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_one_camera_overlay_and_every_role_rendered() {
        let mut app = test_app();
        assert_eq!(count::<StyleCam>(&mut app), 1);
        assert_eq!(count::<StyleUiRoot>(&mut app), 1);
        // Edged surfaces + 8 markers + 9 door reads + 3 observed spine swatches
        // are neon-edged; only the ceiling surface intentionally has no edge.
        assert!(
            count::<NeonEdge>(&mut app) >= 26,
            "every distinct role renders a swatch with a neon edge",
        );
    }

    #[test]
    fn reset_rebuilds_the_scene_without_leaking() {
        let mut app = test_app();
        let baseline = count::<StyleSpawned>(&mut app);
        assert!(baseline > 0);
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyR);
            // Run only the Update schedule so PreUpdate's input-clear does not wipe
            // the press before `reset_input` reads it.
            app.world_mut().run_schedule(Update);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyR);
            app.world_mut().run_schedule(Update);
            assert_eq!(
                count::<StyleSpawned>(&mut app),
                baseline,
                "a reset rebuilds exactly the same scene",
            );
        }
    }
}
