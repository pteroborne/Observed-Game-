mod lab;
// The slot manifest was promoted into `crates/observed_assets` (refactor R2). Keep the
// familiar `manifest::` spelling in this lab by re-exporting the crate under that name.
pub use observed_assets as manifest;

use bevy::{
    app::AppExit,
    asset::AssetPlugin,
    input::InputSystems,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

pub use lab::{AssetStatus, SoundSlot};

pub struct AssetLabPlugin;

impl Plugin for AssetLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundSlot>()
            .init_resource::<AssetStatus>()
            // `init_resource` keeps a root a caller already inserted, which is how a
            // test asks for the empty drop-in case.
            .init_resource::<lab::AssetRoot>()
            .add_systems(Startup, lab::setup_lab)
            .add_systems(
                Update,
                (
                    lab::handle_input.after(InputSystems),
                    lab::play_sound,
                    lab::update_debug_text,
                )
                    .chain(),
            );
    }
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.02, 0.024, 0.03)))
        .add_plugins(
            DefaultPlugins
                // Resolve assets against the same workspace `assets/` directory the
                // drop-in manifest checks. Bevy otherwise reads `assets/` relative to
                // CARGO_MANIFEST_DIR (the crate dir) under `cargo run`, so a file
                // dropped at the repo root would report "LOADED" yet never render.
                .set(AssetPlugin {
                    file_path: manifest::assets_root().to_string_lossy().into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Observed 2 — Asset Drop-In Showcase".to_string(),
                        resolution: WindowResolution::new(1440, 900),
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(AssetLabPlugin);

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
    // Give a couple of frames for any present assets to finish loading, then shoot.
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
    use lab::{AssetCam, AssetUiRoot};
    use manifest::manifest;

    /// A root with no drop-ins, so the fallback case stays testable now that the
    /// workspace ships real assets of its own.
    fn empty_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join("observed2-asset-lab-empty-root");
        std::fs::create_dir_all(&root).expect("empty drop-in root is creatable");
        root
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.insert_resource(lab::AssetRoot(empty_root()))
            .add_plugins((
                MinimalPlugins,
                AssetPlugin::default(),
                InputPlugin,
                // Registers the mesh assets that 0.19's new skinned-mesh-bounds gizmo validates.
                bevy::mesh::MeshPlugin,
                GizmoPlugin,
            ))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            // The lab loads texture, glTF scene, and audio drop-ins. The headless app
            // renders and plays nothing, but the asset server still allocates a handle per
            // slot, and a handle needs its asset type registered.
            .init_asset::<Image>()
            .init_asset::<Scene>()
            .init_asset::<AudioSource>()
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins(AssetLabPlugin);
        app.update();
        app
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    #[test]
    fn boots_with_camera_ui_and_a_status_per_slot() {
        let mut app = test_app();
        assert_eq!(count::<AssetCam>(&mut app), 1);
        assert_eq!(count::<AssetUiRoot>(&mut app), 1);
        let status = app.world().resource::<AssetStatus>();
        assert_eq!(status.slots.len(), manifest().len());
    }

    #[test]
    fn with_no_files_every_slot_falls_back_to_a_placeholder() {
        // The CI/default case: nothing dropped, so the lab is all placeholders and
        // still healthy — that is the graceful fallback the convention promises.
        let mut app = test_app();
        let status = app.world().resource::<AssetStatus>();
        assert_eq!(status.loaded_count(), 0, "no asset files are committed");
        assert!(status.slots.iter().all(|s| !s.present));
        // No sound dropped -> nothing to play.
        assert!(app.world().resource::<SoundSlot>().0.is_none());
        // Placeholder meshes still spawned (floor, wall, pedestal, prop cube...).
        assert!(count::<Mesh3d>(&mut app) >= 4);
    }
}
