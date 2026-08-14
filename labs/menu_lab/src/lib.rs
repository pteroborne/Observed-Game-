mod diagnostics;
mod session;
mod ui;
mod widgets;

use bevy::{
    input_focus::{InputFocus, InputFocusVisible},
    prelude::*,
    ui_widgets::ButtonPlugin,
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
        // 0.19's `DefaultPlugins` ships `UiWidgetsPlugins`, which already contains
        // `ButtonPlugin`; adding it again panics in the windowed lab. Headless
        // tests build on `MinimalPlugins` and still need it here.
        if !app.is_plugin_added::<ButtonPlugin>() {
            app.add_plugins(ButtonPlugin);
        }
        app.init_state::<AppState>()
            .init_resource::<LabSettings>()
            .init_resource::<InputFocus>()
            .insert_resource(InputFocusVisible(true))
            .init_resource::<LifecycleDiagnostics>()
            .init_resource::<session::SessionResetRequested>()
            .init_resource::<widgets::FocusMemory>()
            .init_resource::<widgets::GamepadNavigationLatch>()
            .add_observer(ui::handle_widget_activation)
            .add_systems(Startup, (setup_camera, diagnostics::setup_overlay))
            .add_systems(
                OnEnter(AppState::Boot),
                (ui::setup_boot, widgets::clear_focus).chain(),
            )
            .add_systems(OnExit(AppState::Boot), ui::remove_boot_timer)
            .add_systems(
                OnEnter(AppState::MainMenu),
                (
                    session::cleanup_session,
                    ui::setup_main_menu,
                    widgets::initialize_focus,
                )
                    .chain(),
            )
            .add_systems(
                OnEnter(AppState::Settings),
                (ui::setup_settings, widgets::initialize_focus).chain(),
            )
            .add_systems(
                OnEnter(AppState::Controls),
                (ui::setup_controls, widgets::initialize_focus).chain(),
            )
            .add_systems(
                OnEnter(AppState::Loading),
                (ui::setup_loading, widgets::clear_focus).chain(),
            )
            .add_systems(OnExit(AppState::Loading), ui::remove_loading_timer)
            .add_systems(
                OnEnter(AppState::Gameplay),
                (
                    session::ensure_session,
                    ui::setup_gameplay_hud,
                    widgets::initialize_focus,
                )
                    .chain(),
            )
            .add_systems(
                OnEnter(AppState::Paused),
                (ui::setup_pause_menu, widgets::initialize_focus).chain(),
            )
            .add_systems(
                Update,
                (
                    ui::boot_countdown.run_if(in_state(AppState::Boot)),
                    ui::loading_countdown.run_if(in_state(AppState::Loading)),
                    ui::update_settings_labels.run_if(in_state(AppState::Settings)),
                    (
                        ui::handle_global_input,
                        widgets::focus_hovered_widget,
                        widgets::handle_focus_input,
                        session::perform_requested_reset,
                        widgets::capture_focus_memory,
                    )
                        .chain(),
                    widgets::update_button_visuals,
                    widgets::sync_accessibility.after(ui::update_settings_labels),
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
    use std::time::Duration;

    use super::*;
    use bevy::{
        camera::NormalizedRenderTarget,
        input_focus::InputFocus,
        picking::{
            backend::HitData,
            events::{Click, Pointer},
            pointer::{Location, PointerId},
        },
        state::app::StatesPlugin,
        ui::{InteractionDisabled, Pressed},
        ui_widgets::Activate,
    };

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

    fn widget_entity(app: &mut App, id: widgets::WidgetId) -> Entity {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &widgets::WidgetId)>();
        query
            .iter(world)
            .find_map(|(entity, candidate)| (*candidate == id).then_some(entity))
            .unwrap_or_else(|| panic!("expected widget {id:?}"))
    }

    fn focused_widget(app: &App) -> Option<widgets::WidgetId> {
        let entity = app.world().resource::<InputFocus>().get()?;
        app.world().get::<widgets::WidgetId>(entity).copied()
    }

    fn tap_key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset(key);
        app.update();
    }

    fn click_widget(app: &mut App, entity: Entity) {
        app.world_mut().entity_mut(entity).insert(Pressed);
        // Bevy 0.19 made `Pointer`'s propagation field private, so this is
        // built through the constructor rather than as a literal.
        app.world_mut().trigger(Pointer::<Click>::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: 0,
                    height: 0,
                },
                position: Vec2::ZERO,
            },
            Click {
                button: PointerButton::Primary,
                hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
                duration: Duration::from_millis(20),
                count: 1,
            },
            entity,
        ));
        app.update();
        app.update();
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
    fn keyboard_navigation_uses_semantic_order_and_restores_focus() {
        let mut app = test_app();
        transition(&mut app, AppState::MainMenu);

        assert_eq!(focused_widget(&app), Some(widgets::WidgetId::MainStart));

        tap_key(&mut app, KeyCode::ArrowDown);
        assert_eq!(focused_widget(&app), Some(widgets::WidgetId::MainSettings));
        tap_key(&mut app, KeyCode::ArrowUp);
        assert_eq!(focused_widget(&app), Some(widgets::WidgetId::MainStart));
        tap_key(&mut app, KeyCode::Tab);
        assert_eq!(focused_widget(&app), Some(widgets::WidgetId::MainSettings));

        tap_key(&mut app, KeyCode::Enter);
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Settings
        );
        assert_eq!(
            focused_widget(&app),
            Some(widgets::WidgetId::SettingsHighContrast)
        );

        let original = app.world().resource::<LabSettings>().high_contrast;
        tap_key(&mut app, KeyCode::Space);
        assert_eq!(
            app.world().resource::<LabSettings>().high_contrast,
            !original
        );
        tap_key(&mut app, KeyCode::ArrowDown);
        tap_key(&mut app, KeyCode::ArrowDown);
        assert_eq!(focused_widget(&app), Some(widgets::WidgetId::SettingsBack));
        tap_key(&mut app, KeyCode::Enter);

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::MainMenu
        );
        assert_eq!(
            focused_widget(&app),
            Some(widgets::WidgetId::MainSettings),
            "returning to a scope should restore its last semantic widget"
        );
    }

    #[test]
    fn pointer_hover_and_click_share_focus_and_activation() {
        let mut app = test_app();
        transition(&mut app, AppState::MainMenu);
        let controls = widget_entity(&mut app, widgets::WidgetId::MainControls);

        *app.world_mut()
            .entity_mut(controls)
            .get_mut::<Interaction>()
            .expect("widgets carry pointer interaction state") = Interaction::Hovered;
        app.update();
        assert_eq!(focused_widget(&app), Some(widgets::WidgetId::MainControls));

        click_widget(&mut app, controls);
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Controls,
            "Bevy's pointer Click event should activate the same typed action as keyboard input"
        );
        assert_eq!(focused_widget(&app), Some(widgets::WidgetId::ControlsBack));
    }

    #[test]
    fn disabled_widget_is_accessible_but_never_focusable_or_activatable() {
        let mut app = test_app();
        transition(&mut app, AppState::MainMenu);
        let disabled = widget_entity(&mut app, widgets::WidgetId::MainContinueUnavailable);

        assert!(app.world().get::<InteractionDisabled>(disabled).is_some());
        assert!(
            app.world()
                .get::<bevy::input_focus::tab_navigation::TabIndex>(disabled)
                .is_none()
        );
        let accessibility = app
            .world()
            .get::<bevy::a11y::AccessibilityNode>(disabled)
            .expect("headless button should declare an accessibility node");
        assert_eq!(format!("{:?}", accessibility.role()), "Button");
        assert_eq!(accessibility.label(), Some("Continue — no saved session"));
        assert!(accessibility.is_disabled());

        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(disabled, bevy::input_focus::FocusCause::Navigated);
        tap_key(&mut app, KeyCode::Enter);
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::MainMenu
        );

        app.world_mut().trigger(Activate { entity: disabled });
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::MainMenu,
            "disabled state must guard even a direct activation event"
        );

        app.world_mut().resource_mut::<InputFocus>().clear();
        tap_key(&mut app, KeyCode::Tab);
        assert_eq!(
            focused_widget(&app),
            Some(widgets::WidgetId::MainStart),
            "ordered navigation must skip disabled widgets"
        );
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
