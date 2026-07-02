#[cfg(test)]
mod arch_check;
mod bot;
mod camera;
mod evidence;
pub mod flow;
pub mod guardian;
pub mod hallway;
pub mod items;
pub mod keystones;
mod layout;
pub mod map_validation;
pub mod maze;
mod navmesh;
pub mod rivals;
mod screens;
mod sim;
pub mod tacmap;
pub mod teleport;
mod view;

use bevy::{
    asset::AssetPlugin,
    prelude::*,
    window::{PresentMode, WindowResolution},
};

pub use flow::{Career, MatchResult};

/// The assembled game's top-level flow. Each variant is a self-contained screen
/// whose entities are state-scoped (despawned on exit), so the UX never leaks.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
pub enum GameState {
    #[default]
    Splash,
    MainMenu,
    Loadout,
    Lobby,
    Match,
    Results,
}

pub struct ObservedGamePlugin;

impl Plugin for ObservedGamePlugin {
    fn build(&self, app: &mut App) {
        // Top-level composition: register the state machine + persistent career, the
        // shared camera, and the two screen/match plugins. Each plugin owns the systems
        // for its responsibility (see `screens`); evidence capture is wired in `run`.
        app.init_state::<GameState>()
            .init_resource::<Career>()
            .init_resource::<crate::flow::ActiveMatchSeed>()
            .add_systems(Startup, setup_camera)
            .add_plugins((screens::ScreensPlugin, screens::MatchPlugin));
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        crate::view::components::GameCam,
        Transform::from_xyz(0.0, 2.0, 0.0),
        Name::new("Observed 2 Camera"),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: crate::view::components::MENU_SUN_ILLUMINANCE,
            ..default()
        },
        crate::view::components::GameSun,
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.85, -0.6, 0.0)),
        Name::new("Observed 2 Sun"),
    ));
    // Generous ambient so the vertical walls (which catch little directional light
    // indoors) read clearly.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.7, 0.74, 0.85),
        brightness: 900.0,
        ..default()
    });
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.02, 0.03, 0.05)))
        .add_plugins(
            DefaultPlugins
                // Resolve drop-in assets against the workspace `assets/` directory
                // (Bevy otherwise reads `assets/` relative to the crate dir under
                // `cargo run`, missing files dropped at the repo root).
                .set(AssetPlugin {
                    file_path: crate::view::assets::assets_dir()
                        .to_string_lossy()
                        .into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Observed 2".to_string(),
                        resolution: WindowResolution::new(1440, 900),
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(ObservedGamePlugin);

    // Opt-in evidence capture (no-op in normal play).
    evidence::configure(&mut app);

    app.run();
}

#[cfg(test)]
mod tests;
