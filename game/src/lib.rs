#[cfg(test)]
mod arch_check;
mod bot;
mod camera;
mod content;
mod evidence;
pub mod flow;
pub mod full_wfc;
pub mod guardian;
pub mod hallway;
pub mod hex_wfc;
pub mod items;
pub mod keystones;
mod lan;
mod layout;
pub mod map_catalog;
pub mod map_validation;
pub mod maze;
mod navmesh;
pub mod play_setup;
pub mod rivals;
mod screens;
pub mod settings;
mod sim;
pub mod tacmap;
pub mod teleport;
mod view;
mod wfc_interior;

use bevy::{
    asset::AssetPlugin,
    prelude::*,
    render::{
        RenderPlugin,
        diagnostic::RenderDiagnosticsPlugin,
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
    },
    window::{PresentMode, WindowResolution},
};
use bevy_sprite3d::prelude::Sprite3dPlugin;

pub use flow::{Career, MatchResult};

/// The assembled game's top-level flow. Each variant is a self-contained screen
/// whose entities are state-scoped (despawned on exit), so the UX never leaks.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
pub enum GameState {
    #[default]
    Splash,
    MainMenu,
    /// Preset-first launch hub; advanced match rules are a deliberate second layer.
    Play,
    PlayAdvanced,
    Loadout,
    LanBrowser,
    Lobby,
    /// [DEPRECATED] Legacy place-based match system. Sunsetted in favor of `HexWfc`.
    /// Kept only as a regression testing fixture for unit/integration tests.
    Match,
    /// [DEMOTED] Square-lattice continuous-facility regression fixture. It is not a
    /// production menu destination; tests may enter it directly.
    FullWfc,
    /// The canonical Arc L hex-prism facility Play flow.
    HexWfc,
    Results,
    Replay,
    /// Responsive preparation screen before the canonical facility becomes active.
    Loading,
    Settings,
}

pub struct ObservedGamePlugin;

impl Plugin for ObservedGamePlugin {
    fn build(&self, app: &mut App) {
        // Top-level composition: register the state machine + persistent career, the
        // shared camera, and the two screen/match plugins. Each plugin owns the systems
        // for its responsibility (see `screens`); evidence capture is wired in `run`.
        app.init_state::<GameState>()
            .insert_resource(crate::content::GameContent::committed())
            .init_resource::<crate::flow::ActiveMatchSeed>()
            .insert_resource(crate::play_setup::load_play_setup())
            .insert_resource(crate::flow::load_career())
            .insert_resource(crate::settings::load_settings())
            .insert_resource(crate::lan::LanRuntime::new())
            .insert_resource(crate::view::components::DebugHud(
                crate::evidence::debug_hud_enabled(),
            ))
            .add_systems(Startup, (setup_camera, setup_ui_assets))
            .add_plugins((
                Sprite3dPlugin,
                screens::ScreensPlugin,
                #[allow(deprecated)]
                screens::MatchPlugin,
                full_wfc::FullWfcPlugin,
                hex_wfc::HexWfcPlugin,
            ));
    }
}

fn setup_ui_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load_sound = |slot: observed_assets::AssetSlot| {
        crate::view::assets::asset_present(slot.path)
            .then(|| asset_server.load::<AudioSource>(slot.path))
    };
    commands.insert_resource(crate::view::components::UiAssets {
        click: load_sound(observed_assets::UI_CLICK),
        hover: load_sound(observed_assets::UI_HOVER),
    });
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        crate::view::components::GameCam,
        // Say which camera the UI belongs to rather than letting Bevy infer it. With no
        // marker, a root node targets the highest-order camera on the primary window and
        // `is_active` is not part of that choice — so the hex match's order-1 survivor-map
        // camera silently captured every menu, HUD, and pause overlay while sitting
        // inactive, and they rendered nowhere. Anything that wants a different camera
        // (the survivor map's own legend) now has to ask for it by name.
        bevy::ui::IsDefaultUiCamera,
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
    // Opt-in GPU pass timing. Timestamp queries must be requested at device creation, so
    // unlike the rest of the evidence harness this cannot be switched on after the fact.
    // Requesting features can fail renderer init on adapters that lack them, which is why
    // it stays behind the flag rather than being always-on.
    let gpu_profiling = std::env::var(crate::hex_wfc::GPU_PROFILE_ENV).is_ok();
    let mut plugins = DefaultPlugins
        // Resolve drop-in assets beside a packaged executable, from the workspace
        // during development, or from OBSERVED2_ASSET_ROOT when explicitly set.
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
        });
    if gpu_profiling {
        plugins = plugins.set(RenderPlugin {
            render_creation: RenderCreation::Automatic(WgpuSettings {
                features: WgpuFeatures::TIMESTAMP_QUERY
                    | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_PASSES
                    | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS,
                ..default()
            }),
            ..default()
        });
    }

    app.insert_resource(ClearColor(Color::srgb(0.02, 0.03, 0.05)))
        .add_plugins(plugins)
        .add_plugins(ObservedGamePlugin);
    if gpu_profiling {
        app.add_plugins(RenderDiagnosticsPlugin);
    }

    // Opt-in evidence capture (no-op in normal play).
    evidence::configure(&mut app);

    app.run();
}

#[cfg(test)]
mod tests;
