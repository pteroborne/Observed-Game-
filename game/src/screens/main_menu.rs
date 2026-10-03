//! Main-menu composition. Business actions stay local; the shared widget layer owns
//! focus, pointer/controller parity, accessibility, and visual state.

use bevy::{app::AppExit, prelude::*, ui::InteractionDisabled, ui_widgets::Activate};

use super::widgets::{self, FocusScope, FocusScopeId, WidgetId, WidgetSpec, activation_enabled};
use crate::GameState;
use crate::view::theme::{ACCENT, BORDER, DIM, TITLE, menu_panel, screen_root, text};

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

pub(crate) fn setup(mut commands: Commands) {
    commands.spawn(screen_root(GameState::MainMenu)).with_children(|root| {
        root.spawn(Node { column_gap: px(64), align_items: AlignItems::Center, ..default() }).with_children(|columns| {
            columns.spawn((menu_panel(440.0), widgets::focus_scope(FocusScope::root(SCOPE, PLAY)))).with_children(|menu| {
                menu.spawn(text("OBSERVED 2", 48.0, TITLE));
                menu.spawn(text("THE FACILITY IS WATCHING", 15.0, ACCENT));
                menu.spawn((text("Climb as an Observer. Shape the route as an Architect. Get your team to the summit.", 20.0, DIM), Node { margin: UiRect::vertical(px(18)), ..default() }));
                for (order, id, label, action) in [(0, PLAY, "Play", MainAction::Play), (1, LOADOUT, "Cosmetics", MainAction::Loadout), (2, SETTINGS, "Settings", MainAction::Settings), (3, QUIT, "Quit", MainAction::Quit)] {
                    widgets::spawn_button(menu, WidgetSpec::enabled(id, SCOPE, order, label).with_size(384.0, 54.0), action);
                }
            });
            columns.spawn(Node { width: px(400), height: px(440), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, align_items: AlignItems::Center, row_gap: px(16), ..default() }).with_children(|art| {
                art.spawn(text("ASCENT", 20.0, ACCENT));
                for width in [112.0, 184.0, 256.0, 328.0] {
                    art.spawn((Node { width: px(width), height: px(54), border: UiRect::all(px(2)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, BorderColor::all(BORDER))).with_children(|floor| {
                        floor.spawn((Node { width: px(12), height: px(12), ..default() }, BackgroundColor(ACCENT)));
                    });
                }
                art.spawn(text("OBSERVE. BUILD. SURVIVE.", 16.0, DIM));
            });
        });
        root.spawn(text("Arrow keys / D-pad / stick / pointer | Enter / A select | choose Quit to exit", 14.0, DIM));
    });
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
