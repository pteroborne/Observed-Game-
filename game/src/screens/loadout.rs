//! Cosmetic loadout composition. The shared widget layer owns focus, pointer,
//! controller, accessibility, and disabled presentation; this screen owns only
//! cosmetic-specific labels and actions.

use bevy::{prelude::*, ui::InteractionDisabled, ui_widgets::Activate};
use observed_progression::progression::{Cosmetic, Slot, Unlock, catalog, cosmetic};

use super::widgets::{
    self, FocusScope, FocusScopeId, WidgetId, WidgetLabel, WidgetSpec, activation_enabled,
};
use crate::GameState;
use crate::flow::Career;
use crate::view::theme::{ACCENT, BORDER, DIM, PANEL, TITLE, screen_root, text};

const SCOPE: FocusScopeId = FocusScopeId("loadout");
const FIRST_COSMETIC: WidgetId = WidgetId::keyed("loadout.cosmetic", 0);
const BACK: WidgetId = WidgetId::named("loadout.back");
const LOADOUT_COLUMNS: usize = 2;
const LOADOUT_GRID_WIDTH: f32 = 1000.0;
const LOADOUT_COLUMN_GAP: f32 = 12.0;
const LOADOUT_BUTTON_WIDTH: f32 = (LOADOUT_GRID_WIDTH
    - LOADOUT_COLUMN_GAP * (LOADOUT_COLUMNS as f32 - 1.0))
    / LOADOUT_COLUMNS as f32;

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoadoutAction {
    Equip(u16),
    Back,
}

#[derive(Component)]
pub(crate) struct LoadoutHeader;

pub(crate) fn setup(mut commands: Commands, career: Res<Career>) {
    let cosmetics = catalog();
    commands
        .spawn(screen_root(GameState::Loadout))
        .with_children(|root| {
            root.spawn(text("LOADOUT", 40.0, TITLE));
            root.spawn((LoadoutHeader, text(profile_summary(&career), 17.0, ACCENT)));
            root.spawn((
                loadout_panel(),
                widgets::focus_scope(FocusScope::grid(SCOPE, FIRST_COSMETIC, BACK, 2)),
            ))
            .with_children(|panel| {
                panel
                    .spawn(Node {
                        width: px(LOADOUT_GRID_WIDTH),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::Center,
                        column_gap: px(LOADOUT_COLUMN_GAP),
                        row_gap: px(2),
                        ..default()
                    })
                    .with_children(|grid| {
                        for (order, item) in cosmetics.iter().enumerate() {
                            let id = WidgetId::keyed("loadout.cosmetic", u64::from(item.id));
                            let order = u16::try_from(order).expect("cosmetic catalog fits in u16");
                            let label = cosmetic_label(item, &career);
                            let spec = if career.profile.is_unlocked(item.id) {
                                WidgetSpec::enabled(id, SCOPE, order, label)
                            } else {
                                WidgetSpec::disabled(id, SCOPE, order, label)
                            };
                            widgets::spawn_button(
                                grid,
                                spec.with_size(LOADOUT_BUTTON_WIDTH, 44.0),
                                LoadoutAction::Equip(item.id),
                            );
                        }
                    });
                widgets::spawn_button(
                    panel,
                    WidgetSpec::enabled(
                        BACK,
                        SCOPE,
                        u16::try_from(cosmetics.len()).expect("cosmetic catalog fits in u16"),
                        "Back to main menu",
                    )
                    .with_size(LOADOUT_BUTTON_WIDTH, 44.0),
                    LoadoutAction::Back,
                );
            });
            root.spawn(text(
                "Locked cosmetics are skipped by focus | Enter / A equips | Esc / B back",
                15.0,
                DIM,
            ));
        });
}

fn loadout_panel() -> impl Bundle {
    (
        Node {
            width: px(1040),
            padding: UiRect::all(px(18)),
            border: UiRect::all(px(1)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BORDER),
    )
}

pub(crate) fn activate(
    activation: On<Activate>,
    actions: Query<&LoadoutAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut career: ResMut<Career>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    match *action {
        LoadoutAction::Equip(id) => {
            if career.profile.equip(id) {
                #[cfg(not(test))]
                crate::flow::save_profile(&career);
            }
        }
        LoadoutAction::Back => next.set(GameState::MainMenu),
    }
}

pub(crate) fn refresh(
    career: Res<Career>,
    mut header: Query<&mut Text, With<LoadoutHeader>>,
    mut buttons: Query<(&LoadoutAction, &mut WidgetLabel)>,
) {
    if !career.is_changed() {
        return;
    }
    if let Ok(mut header) = header.single_mut() {
        **header = profile_summary(&career);
    }
    for (action, mut label) in &mut buttons {
        let LoadoutAction::Equip(id) = *action else {
            continue;
        };
        if let Some(item) = cosmetic(id) {
            label.0 = cosmetic_label(&item, &career);
        }
    }
}

fn profile_summary(career: &Career) -> String {
    let profile = &career.profile;
    let equipped = |slot: Slot| {
        profile
            .equipped
            .get(&slot)
            .and_then(|id| cosmetic(*id))
            .map(|item| item.name)
            .unwrap_or("--")
    };
    format!(
        "Level {} | unlocked {} / {} | equipped  Color {}  Trail {}  Badge {}",
        profile.level(),
        profile.unlocked.len(),
        catalog().len(),
        equipped(Slot::Color),
        equipped(Slot::Trail),
        equipped(Slot::Badge),
    )
}

fn cosmetic_label(item: &Cosmetic, career: &Career) -> String {
    if career.profile.is_equipped(item.id) {
        return format!("{} | {} | EQUIPPED", item.name, item.slot.label());
    }
    if career.profile.is_unlocked(item.id) {
        return format!("{} | {}", item.name, item.slot.label());
    }
    format!(
        "{} | {} | LOCKED: {}",
        item.name,
        item.slot.label(),
        unlock_requirement(item.unlock)
    )
}

fn unlock_requirement(unlock: Unlock) -> String {
    match unlock {
        Unlock::Level(level) => format!("reach level {level}"),
        Unlock::Wins(1) => "win 1 match".to_string(),
        Unlock::Wins(wins) => format!("win {wins} matches"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_cosmetic_labels_explain_the_requirement() {
        let career = Career::default();
        let ember = cosmetic(1).expect("Ember exists");
        assert_eq!(
            cosmetic_label(&ember, &career),
            "Ember | Color | LOCKED: reach level 2"
        );
    }

    #[test]
    fn equipped_cosmetic_labels_report_the_active_choice() {
        let career = Career::default();
        let ash = cosmetic(0).expect("Ash exists");
        assert_eq!(cosmetic_label(&ash, &career), "Ash | Color | EQUIPPED");
    }

    #[test]
    fn cosmetic_catalog_fits_the_compact_two_column_layout() {
        let cosmetics = catalog();
        let rows = cosmetics.len().div_ceil(LOADOUT_COLUMNS);

        assert_eq!(
            cosmetics.len(),
            10,
            "layout contract covers the full catalog"
        );
        assert_eq!(rows, 5);
    }
}
