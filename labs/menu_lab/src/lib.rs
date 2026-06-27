mod diagnostics;
mod session;
mod ui;

use bevy::{
    input_focus::InputFocus,
    prelude::*,
    window::{PresentMode, WindowResolution},
};

pub use diagnostics::{LifecycleDiagnostics, LifecycleHealth};
pub use session::{GameOwned, GameplaySession, LabPlayerId, SessionRoot};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
pub enum AppState {
    #[default]
    Boot,
    MainMenu,
    Settings,
    Controls,
    Loading,
    Gameplay,
    Paused,
}

#[derive(Resource, Debug, Clone)]
pub struct LabSettings {
    pub high_contrast: bool,
    pub diagnostics_visible: bool,
}

impl Default for LabSettings {
    fn default() -> Self {
        Self {
            high_contrast: false,
            diagnostics_visible: true,
        }
    }
}

pub struct MenuLabPlugin;

impl Plugin for MenuLabPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<LabSettings>()
            .init_resource::<InputFocus>()
            .init_resource::<LifecycleDiagnostics>()
            .init_resource::<session::SessionResetRequested>()
            .add_systems(Startup, (setup_camera, diagnostics::setup_overlay))
            .add_systems(OnEnter(AppState::Boot), ui::setup_boot)
            .add_systems(OnExit(AppState::Boot), ui::remove_boot_timer)
            .add_systems(
                OnEnter(AppState::MainMenu),
                (session::cleanup_session, ui::setup_main_menu).chain(),
            )
            .add_systems(OnEnter(AppState::Settings), ui::setup_settings)
            .add_systems(OnEnter(AppState::Controls), ui::setup_controls)
            .add_systems(OnEnter(AppState::Loading), ui::setup_loading)
            .add_systems(OnExit(AppState::Loading), ui::remove_loading_timer)
            .add_systems(
                OnEnter(AppState::Gameplay),
                (session::ensure_session, ui::setup_gameplay_hud).chain(),
            )
            .add_systems(OnEnter(AppState::Paused), ui::setup_pause_menu)
            .add_systems(
                Update,
                (
                    ui::boot_countdown.run_if(in_state(AppState::Boot)),
                    ui::loading_countdown.run_if(in_state(AppState::Loading)),
                    ui::button_visuals,
                    ui::handle_button_actions,
                    ui::handle_keyboard,
                    ui::update_settings_labels.run_if(in_state(AppState::Settings)),
                    session::perform_requested_reset
                        .after(ui::handle_button_actions)
                        .after(ui::handle_keyboard),
                    session::animate_players.run_if(in_state(AppState::Gameplay)),
                    diagnostics::update_overlay,
                ),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Name::new("Menu Lab Camera")));
}

pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.055)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Menu Lab".to_string(),
                resolution: WindowResolution::new(1280, 720),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(MenuLabPlugin)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(ClearColor(Color::BLACK))
            .add_plugins(MenuLabPlugin);
        app.update();
        app
    }

    fn transition(app: &mut App, state: AppState) {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(state);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            state,
            "state transition should complete in one update"
        );
    }

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn assert_one_screen(app: &mut App) {
        assert_eq!(count::<ui::ScreenRoot>(app), 1);
    }

    #[test]
    fn lifecycle_health_reports_expected_invariants() {
        assert_eq!(
            diagnostics::evaluate_health(AppState::MainMenu, 1, 0, None),
            LifecycleHealth {
                screen_roots_ok: true,
                session_entities_ok: true,
                session_resource_ok: true,
            }
        );

        assert_eq!(
            diagnostics::evaluate_health(AppState::Paused, 1, 12, Some(12)),
            LifecycleHealth {
                screen_roots_ok: true,
                session_entities_ok: true,
                session_resource_ok: true,
            }
        );

        assert!(!diagnostics::evaluate_health(AppState::Gameplay, 2, 12, Some(12)).screen_roots_ok);
        assert!(!diagnostics::evaluate_health(AppState::MainMenu, 1, 1, None).session_entities_ok);
    }

    #[test]
    fn pause_resume_preserves_session_and_reset_replaces_it() {
        let mut app = test_app();
        transition(&mut app, AppState::MainMenu);
        transition(&mut app, AppState::Loading);
        transition(&mut app, AppState::Gameplay);

        let first_generation = app.world().resource::<GameplaySession>().generation;
        let expected = app.world().resource::<GameplaySession>().owned_entities;
        assert_eq!(count::<GameOwned>(&mut app), expected);

        transition(&mut app, AppState::Paused);
        transition(&mut app, AppState::Gameplay);
        assert_eq!(
            app.world().resource::<GameplaySession>().generation,
            first_generation
        );
        assert_eq!(count::<GameOwned>(&mut app), expected);

        app.world_mut()
            .resource_mut::<session::SessionResetRequested>()
            .0 = true;
        app.update();

        let reset_session = app.world().resource::<GameplaySession>();
        assert_eq!(reset_session.generation, first_generation + 1);
        assert_eq!(reset_session.owned_entities, expected);
        assert_eq!(count::<GameOwned>(&mut app), expected);

        let diagnostics = app.world().resource::<LifecycleDiagnostics>();
        assert_eq!(diagnostics.last_cleanup, Some((expected, 0)));
        assert_eq!(diagnostics.resets, 1);
    }

    #[test]
    fn repeated_menu_cycles_do_not_leak_or_duplicate_entities() {
        let mut app = test_app();
        transition(&mut app, AppState::MainMenu);

        for _ in 0..10 {
            assert_one_screen(&mut app);
            assert_eq!(count::<GameOwned>(&mut app), 0);
            assert!(!app.world().contains_resource::<GameplaySession>());

            transition(&mut app, AppState::Settings);
            assert_one_screen(&mut app);
            transition(&mut app, AppState::MainMenu);
            transition(&mut app, AppState::Controls);
            assert_one_screen(&mut app);
            transition(&mut app, AppState::MainMenu);

            transition(&mut app, AppState::Loading);
            assert_one_screen(&mut app);
            transition(&mut app, AppState::Gameplay);
            assert_one_screen(&mut app);

            let expected = app.world().resource::<GameplaySession>().owned_entities;
            assert_eq!(count::<GameOwned>(&mut app), expected);

            transition(&mut app, AppState::Paused);
            assert_one_screen(&mut app);
            assert_eq!(count::<GameOwned>(&mut app), expected);
            transition(&mut app, AppState::Gameplay);
            assert_eq!(count::<GameOwned>(&mut app), expected);
            transition(&mut app, AppState::Paused);
            transition(&mut app, AppState::MainMenu);

            assert_one_screen(&mut app);
            assert_eq!(count::<GameOwned>(&mut app), 0);
            assert!(!app.world().contains_resource::<GameplaySession>());
            assert_eq!(
                app.world().resource::<LifecycleDiagnostics>().last_cleanup,
                Some((expected, 0))
            );
        }
    }
}
