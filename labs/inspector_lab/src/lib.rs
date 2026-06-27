//! **inspector_lab** — the R11-accepted dev-tools dependency, proven in isolation.
//!
//! Evaluation (see [docs/refactor_r11_evaluation.md]): of the catalogued candidates, the
//! live ECS inspector is the one that clears the dependency policy — and
//! `bevy-inspector-egui` `0.36` is **compatible with the pinned Bevy `0.18.1`** (it
//! requires `bevy ^0.18.0`), so no Bevy upgrade is needed. `bevy_screen_diagnostics` is
//! *not* 0.18-compatible, so FPS/frame timing uses Bevy's built-in
//! `FrameTimeDiagnosticsPlugin` (no extra dependency).
//!
//! The dependency is **default-off** behind the `dev_tools` cargo feature, so the normal
//! workspace build/test never pulls egui. Run with the inspector via:
//! `cargo run -p inspector_lab --features dev_tools`. The lab's domain entities are plain
//! Bevy entities (independent of the inspector's types, per policy rule 4), so the
//! fallback build — no feature — is fully functional and the inspector is purely additive.

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

/// Marks a spinning, inspectable demo entity.
#[derive(Component)]
pub(crate) struct Spin {
    speed: f32,
}

#[derive(Component)]
pub(crate) struct OverlayText;

/// The dev-tools **adapter**: Bevy's built-in frame-time diagnostics always, and — only
/// behind `dev_tools` — the third-party live ECS inspector. This is the single seam where
/// the optional dependency enters; nothing else in the workspace references it.
pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        #[cfg(feature = "dev_tools")]
        app.add_plugins(bevy_inspector_egui::quick::WorldInspectorPlugin::new());
    }
}

pub struct InspectorLabPlugin;

impl Plugin for InspectorLabPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DevToolsPlugin)
            .add_systems(Startup, (setup, setup_ui))
            .add_systems(Update, (spin, update_overlay));
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 4.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Inspector Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.4, 0.0)),
        Name::new("Sun"),
    ));
    let cube = meshes.add(Cuboid::new(1.2, 1.2, 1.2));
    for i in 0..4u32 {
        commands.spawn((
            Spin {
                speed: 0.5 + i as f32 * 0.4,
            },
            Name::new(format!("Inspectable crate {i}")),
            Mesh3d(cube.clone()),
            MeshMaterial3d(materials.add(StandardMaterial::from_color(Color::srgb(
                0.30,
                0.55 + i as f32 * 0.08,
                0.95,
            )))),
            Transform::from_xyz((i as f32 - 1.5) * 2.6, 0.0, 0.0),
        ));
    }
}

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        OverlayText,
        Text::new("inspector_lab"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
    ));
}

fn spin(time: Res<Time>, mut query: Query<(&Spin, &mut Transform)>) {
    for (spin, mut transform) in &mut query {
        transform.rotate_y(spin.speed * time.delta_secs());
    }
}

fn update_overlay(
    spinners: Query<(), With<Spin>>,
    cams: Query<(), With<Camera3d>>,
    mut text: Query<&mut Text, With<OverlayText>>,
) {
    let healthy = spinners.iter().count() == 4 && cams.iter().count() == 1;
    let dev = cfg!(feature = "dev_tools");
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    **text = format!(
        "INSPECTOR LAB  {}\n\
         dev_tools (live ECS inspector) {}\n\
         frame-time diagnostics: Bevy built-in\n\n\
         {}",
        if healthy { "[PASS]" } else { "[FAIL]" },
        if dev {
            "ON"
        } else {
            "OFF (run --features dev_tools)"
        },
        if dev {
            "Drag the egui panel to inspect every entity/component live."
        } else {
            "Built without the inspector; this overlay is the fallback."
        },
    );
}

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.02, 0.03, 0.05)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Inspector Lab".to_string(),
                resolution: WindowResolution::new(1280, 800),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(InspectorLabPlugin)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;

    /// Headless guard for the adapter's **fallback** path (no `dev_tools` feature, so no
    /// egui/window needed): the lab boots, the built-in diagnostics plugin is wired, and
    /// the inspectable demo entities exist. The `dev_tools` path is guarded by a compile
    /// check (`cargo check -p inspector_lab --features dev_tools`), which proves the
    /// optional dependency builds against the pinned Bevy 0.18.1.
    #[test]
    fn boots_with_diagnostics_and_inspectable_entities() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins(InspectorLabPlugin);
        app.update();

        // Bevy's built-in frame-time diagnostics is registered (the no-dep FPS source).
        assert!(
            app.world()
                .get_resource::<bevy::diagnostic::DiagnosticsStore>()
                .and_then(|d| d.get(&FrameTimeDiagnosticsPlugin::FPS))
                .is_some(),
            "frame-time diagnostics must be wired"
        );

        let mut spinners = app.world_mut().query_filtered::<(), With<Spin>>();
        assert_eq!(
            spinners.iter(app.world()).count(),
            4,
            "the four inspectable demo entities spawn"
        );
    }
}
