//! Cosmetics are inspected before equipping; previewing never changes the profile.
mod preview;

use super::widgets::{
    self, FocusScope, FocusScopeId, WidgetId, WidgetLabel, WidgetSpec, activation_enabled,
};
use crate::view::theme::{ACCENT, BORDER, DIM, PANEL, TITLE, screen_root, text};
use crate::{GameState, flow::Career};
use bevy::{
    ecs::system::SystemParam,
    input_focus::{InputFocus, tab_navigation::TabIndex},
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::Activate,
};
use observed_progression::progression::{Cosmetic, Slot, Unlock, catalog, cosmetic};

const SCOPE: FocusScopeId = FocusScopeId("loadout");
const EQUIP: WidgetId = WidgetId::named("loadout.equip");
const BACK: WidgetId = WidgetId::named("loadout.back");

#[derive(Resource, Default)]
pub(crate) struct CosmeticSelection(pub(crate) u16);
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoadoutAction {
    Select(u16),
    Equip(u16),
    Back,
}
#[derive(Component)]
pub(crate) struct LoadoutHeader;
#[derive(Component)]
pub(crate) struct Comparison;
#[derive(Component)]
pub(crate) struct SelectedDescription;

pub(crate) fn setup(mut commands: Commands, career: Res<Career>) {
    let initial = career
        .profile
        .equipped
        .get(&Slot::Color)
        .copied()
        .filter(|&id| cosmetic(id).is_some_and(|item| item.slot == Slot::Color))
        .unwrap_or(0);
    commands.insert_resource(CosmeticSelection(initial));
    commands
        .spawn(screen_root(GameState::Loadout))
        .with_children(|root| {
            root.spawn(text("COSMETICS", 40.0, TITLE));
            root.spawn((LoadoutHeader, text(profile_summary(&career), 17.0, ACCENT)));
            root.spawn((
                Node {
                    width: px(1120),
                    height: px(590),
                    flex_shrink: 0.0,
                    padding: UiRect::all(px(18)),
                    border: UiRect::all(px(1)),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(22),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(BORDER),
                widgets::focus_scope(FocusScope::screen(
                    SCOPE,
                    WidgetId::keyed("loadout.cosmetic", u64::from(initial)),
                    BACK,
                )),
            ))
            .with_children(|body| {
                body.spawn(Node {
                    width: px(580),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|list| {
                    list.spawn(text("Select an item to compare", 17.0, TITLE));
                    for (order, item) in catalog().iter().enumerate() {
                        widgets::spawn_button(
                            list,
                            WidgetSpec::enabled(
                                WidgetId::keyed("loadout.cosmetic", u64::from(item.id)),
                                SCOPE,
                                order as u16,
                                cosmetic_label(item, &career),
                            )
                            .with_size(580.0, 42.0),
                            LoadoutAction::Select(item.id),
                        );
                    }
                });
                body.spawn(Node {
                    width: px(480),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    ..default()
                })
                .with_children(|detail| {
                    detail.spawn((
                        Comparison,
                        Node {
                            width: px(480),
                            height: px(320),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Row,
                            column_gap: px(16),
                            ..default()
                        },
                    ));
                    detail.spawn((
                        SelectedDescription,
                        text("", 16.0, TITLE),
                        Node {
                            width: px(470),
                            min_height: px(80),
                            ..default()
                        },
                    ));
                    widgets::spawn_button(
                        detail,
                        WidgetSpec::disabled(EQUIP, SCOPE, 10, "Already equipped")
                            .with_size(470.0, 44.0),
                        LoadoutAction::Equip(0),
                    );
                    widgets::spawn_button(
                        detail,
                        WidgetSpec::enabled(BACK, SCOPE, 11, "Back to main menu")
                            .with_size(470.0, 44.0),
                        LoadoutAction::Back,
                    );
                });
            });
            root.spawn(text(
                "Your design applies at match start | Iris shows team role",
                14.0,
                DIM,
            ));
            root.spawn(text(
                "Enter / A / pointer selects or equips | Esc / B back",
                14.0,
                DIM,
            ));
        });
}

pub(crate) fn activate(
    activation: On<Activate>,
    actions: Query<&LoadoutAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    selection: Option<ResMut<CosmeticSelection>>,
    mut career: ResMut<Career>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(action) = actions.get(activation.entity) else {
        return;
    };
    if *action == LoadoutAction::Back {
        next.set(GameState::MainMenu);
        return;
    }
    let Some(mut selection) = selection else {
        return;
    };
    match *action {
        LoadoutAction::Select(id) if cosmetic(id).is_some() => selection.0 = id,
        LoadoutAction::Equip(id) if id == selection.0 && career.profile.equip(id) => {
            #[cfg(not(test))]
            crate::flow::save_profile(&career);
        }
        _ => {}
    }
}

#[derive(SystemParam)]
pub(crate) struct LoadoutRefreshContext<'w, 's> {
    commands: Commands<'w, 's>,
    header: Query<'w, 's, &'static mut Text, With<LoadoutHeader>>,
    description:
        Query<'w, 's, &'static mut Text, (With<SelectedDescription>, Without<LoadoutHeader>)>,
    comparisons: Query<'w, 's, (Entity, Option<&'static Children>), With<Comparison>>,
    buttons: Query<
        'w,
        's,
        (
            Entity,
            &'static mut LoadoutAction,
            &'static mut WidgetLabel,
            Has<InteractionDisabled>,
        ),
    >,
    focus: ResMut<'w, InputFocus>,
}

pub(crate) fn refresh(
    career: Res<Career>,
    selection: Res<CosmeticSelection>,
    ui: LoadoutRefreshContext,
) {
    let LoadoutRefreshContext {
        mut commands,
        mut header,
        mut description,
        comparisons,
        mut buttons,
        mut focus,
    } = ui;
    if !career.is_changed() && !selection.is_changed() {
        return;
    }
    let Some(item) = cosmetic(selection.0) else {
        return;
    };
    if let Ok(mut header) = header.single_mut() {
        **header = profile_summary(&career);
    }
    let equipped = preview::Look::equipped(&career.profile);
    for (entity, children) in &comparisons {
        if let Some(children) = children {
            for child in children {
                commands.entity(*child).despawn();
            }
        }
        commands.entity(entity).with_children(|parent| {
            preview::card(parent, "Equipped", equipped);
            preview::card(parent, "Selected preview", equipped.with_item(item.id));
        });
    }
    let unlocked = career.profile.is_unlocked(item.id);
    let already = career.profile.is_equipped(item.id);
    for mut description in &mut description {
        **description = format!(
            "{} | {}\n{}\nPreview changes only this slot.",
            item.name,
            item.slot.label(),
            if already {
                "Equipped and saved.".into()
            } else if unlocked {
                "Unlocked. Equip to save this choice.".into()
            } else {
                format!("Locked: {}.", unlock_requirement(item.unlock))
            }
        );
    }
    for (entity, mut action, mut label, disabled) in &mut buttons {
        match *action {
            LoadoutAction::Select(id) => {
                if let Some(cosmetic) = cosmetic(id) {
                    label.0 = format!(
                        "{}{}",
                        if id == item.id { "[Preview] " } else { "" },
                        cosmetic_label(&cosmetic, &career)
                    );
                }
            }
            LoadoutAction::Equip(_) => {
                *action = LoadoutAction::Equip(item.id);
                label.0 = if already {
                    "Already equipped".into()
                } else if unlocked {
                    format!("Equip {}", item.name)
                } else {
                    "Locked - preview only".into()
                };
                let enabled = unlocked && !already;
                if enabled && disabled {
                    commands
                        .entity(entity)
                        .remove::<InteractionDisabled>()
                        .insert(TabIndex(10));
                }
                if !enabled && !disabled {
                    commands
                        .entity(entity)
                        .insert(InteractionDisabled)
                        .remove::<TabIndex>();
                    if focus.get() == Some(entity) {
                        focus.clear();
                    }
                }
            }
            LoadoutAction::Back => {}
        }
    }
}

pub(crate) fn cleanup(mut commands: Commands) {
    commands.remove_resource::<CosmeticSelection>();
}
fn profile_summary(career: &Career) -> String {
    format!(
        "Level {} | {} wins | unlocked {} / {} | cosmetic choices save when equipped",
        career.profile.level(),
        career.profile.wins,
        career.profile.unlocked.len(),
        catalog().len()
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
        Unlock::Wins(1) => "win 1 match".into(),
        Unlock::Wins(wins) => format!("win {wins} matches"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locked_cosmetic_labels_explain_the_requirement() {
        assert_eq!(
            cosmetic_label(&cosmetic(1).unwrap(), &Career::default()),
            "Ember | Color | LOCKED: reach level 2"
        );
    }
    #[test]
    fn comparison_changes_only_the_selected_slot() {
        let look = preview::Look::equipped(&Career::default().profile);
        assert_eq!(look.with_item(2), preview::Look { color: 2, ..look });
        assert_eq!(look.with_item(6), preview::Look { trail: 6, ..look });
        assert_eq!(look.with_item(9), preview::Look { badge: 9, ..look });
        assert_eq!(look.with_item(u16::MAX), look);
    }

    #[test]
    fn locked_items_can_be_inspected_without_equipping_and_exit_cleans_the_preview() {
        let mut app = crate::tests::test_app();
        crate::tests::go(&mut app, GameState::Loadout);
        let before = app.world().resource::<Career>().profile.clone();
        let select = {
            let world = app.world_mut();
            let mut actions = world.query::<(Entity, &LoadoutAction, Has<InteractionDisabled>)>();
            let (entity, _, disabled) = actions
                .iter(world)
                .find(|(_, action, _)| **action == LoadoutAction::Select(1))
                .unwrap();
            assert!(!disabled, "locked items remain inspectable");
            entity
        };
        app.world_mut().trigger(Activate { entity: select });
        app.update();
        let equip = {
            let world = app.world_mut();
            let mut actions = world.query::<(
                Entity,
                &LoadoutAction,
                Has<InteractionDisabled>,
                Option<&TabIndex>,
            )>();
            let (entity, _, disabled, tab) = actions
                .iter(world)
                .find(|(_, action, _, _)| **action == LoadoutAction::Equip(1))
                .unwrap();
            assert!(disabled);
            assert!(
                tab.is_none(),
                "locked Equip is skipped by keyboard/controller navigation"
            );
            entity
        };
        app.world_mut().trigger(Activate { entity: equip });
        assert_eq!(app.world().resource::<Career>().profile, before);
        assert_eq!(app.world().resource::<CosmeticSelection>().0, 1);
        crate::tests::go(&mut app, GameState::MainMenu);
        assert!(!app.world().contains_resource::<CosmeticSelection>());
        let world = app.world_mut();
        let mut previews = world.query_filtered::<Entity, With<Comparison>>();
        assert_eq!(previews.iter(world).count(), 0);
        crate::tests::go(&mut app, GameState::Loadout);
        assert_eq!(app.world().resource::<CosmeticSelection>().0, 0);
    }
}
