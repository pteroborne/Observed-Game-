//! Main-menu composition. Business actions stay local; the shared widget layer owns
//! focus, pointer/controller parity, accessibility, and visual state.

use bevy::{app::AppExit, prelude::*, ui::InteractionDisabled, ui_widgets::Activate};

use super::widgets::{self, FocusScope, FocusScopeId, WidgetId, WidgetSpec, activation_enabled};
use crate::GameState;
use crate::flow::Career;
use crate::view::theme::{ACCENT, DIM, TITLE, panel, screen_root, text};

const SCOPE: FocusScopeId = FocusScopeId("main");
const PLAY: WidgetId = WidgetId::named("main.play");
const LOADOUT: WidgetId = WidgetId::named("main.loadout");
const SETTINGS: WidgetId = WidgetId::named("main.settings");
const QUIT: WidgetId = WidgetId::named("main.quit");

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MainAction {
    Play,
    Loadout,
    Settings,
    Quit,
}

#[derive(Component)]
pub(crate) struct MainMenuBanner;

pub(crate) fn setup(mut commands: Commands) {
    commands
        .spawn(screen_root(GameState::MainMenu))
        .with_children(|root| {
            root.spawn(text("OBSERVED 2", 52.0, TITLE));
            root.spawn((MainMenuBanner, text("", 18.0, ACCENT)));
            root.spawn((panel(), widgets::focus_scope(FocusScope::root(SCOPE, PLAY))))
                .with_children(|panel| {
                    widgets::spawn_button(
                        panel,
                        WidgetSpec::enabled(PLAY, SCOPE, 0, "Play"),
                        MainAction::Play,
                    );
                    widgets::spawn_button(
                        panel,
                        WidgetSpec::enabled(LOADOUT, SCOPE, 1, "Loadout"),
                        MainAction::Loadout,
                    );
                    widgets::spawn_button(
                        panel,
                        WidgetSpec::enabled(SETTINGS, SCOPE, 2, "Settings"),
                        MainAction::Settings,
                    );
                    widgets::spawn_button(
                        panel,
                        WidgetSpec::enabled(QUIT, SCOPE, 3, "Quit"),
                        MainAction::Quit,
                    );
                });
            root.spawn(text(
                "Arrow keys / D-pad / stick / pointer | Enter / A select | choose Quit to exit",
                15.0,
                DIM,
            ));
        });
}

pub(crate) fn update_banner(
    career: Res<Career>,
    mut banner: Query<&mut Text, With<MainMenuBanner>>,
) {
    if let Ok(mut text) = banner.single_mut() {
        **text = format!(
            "Level {} | {} XP | {} matches played",
            career.profile.level(),
            career.profile.xp,
            career.profile.matches_played,
        );
    }
}

pub(crate) fn activate(
    activation: On<Activate>,
    actions: Query<&MainAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    match action {
        MainAction::Play => next.set(GameState::Play),
        MainAction::Loadout => next.set(GameState::Loadout),
        MainAction::Settings => next.set(GameState::Settings),
        MainAction::Quit => {
            exit.write(AppExit::Success);
        }
    }
}
