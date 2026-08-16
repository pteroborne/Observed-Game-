//! The panel's tab strip.
//!
//! Split out of `chrome.rs` when that file crossed the 600-line review budget.
//! The strip is genuinely its own concern: it owns which tab looks selected,
//! and nothing else in the chrome needs to know how that is drawn.
//!
//! Previously the whole row was one string, with the active tab wrapped in
//! brackets and pushed into the same `Text` node as the panel body. That has
//! two costs. Brackets are a convention a reader has to be taught, and a single
//! string wraps mid-label - seven labels do not fit one line at this panel
//! width. Real nodes fix both: the row wraps between tabs, and the selection is
//! shown the way a selection is normally shown.

use bevy::ecs::relationship::RelatedSpawnerCommands;
use bevy::prelude::*;
use observed_ui::theme::{ChromeRole, chrome};

use crate::chrome::{LabMenuState, StudioTab};
use crate::typography;

/// One tab header. Carries its own index so the update system can colour it
/// without recomputing the row.
#[derive(Component)]
pub struct ChromeTab(pub usize);

/// The label inside a [`ChromeTab`], so text colour and weight can change with
/// state without respawning anything.
#[derive(Component)]
pub struct ChromeTabLabel(pub usize);

/// Build the strip. One node per tab.
pub fn spawn_tab_row(menu: &mut RelatedSpawnerCommands<ChildOf>, assets: &AssetServer) {
    menu.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(2.0),
            row_gap: Val::Px(2.0),
            margin: UiRect::bottom(Val::Px(12.0)),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|row| {
        for (index, tab) in StudioTab::ALL.iter().enumerate() {
            row.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)),
                    // Only the bottom edge is drawn, so an inactive tab costs no
                    // visual weight and the active one reads as a selected edge
                    // rather than a chip floating in the panel.
                    border: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                ChromeTab(index),
            ))
            // Tapping a tab selects it. Without this the strip is decoration on
            // a touch device: `Tab` cycles the selection and there is no Tab
            // key. The index is captured rather than read back off the trigger,
            // so each header knows its own answer.
            .observe(
                move |_: On<Pointer<Click>>, mut menu_state: ResMut<LabMenuState>| {
                    menu_state.active_tab = index;
                    menu_state.selected_item = 0;
                },
            )
            .with_children(|cell| {
                cell.spawn((
                    Text::new(tab.label()),
                    typography::font(assets, typography::Role::TabIdle),
                    TextColor(typography::Role::TabIdle.colour()),
                    ChromeTabLabel(index),
                ));
            });
        }
    });
}

/// Paint the active tab.
///
/// Three cues rather than one - filled face, lit underline, and bold label - so
/// the selection survives both a colour-vision difference and a dim monitor.
pub fn update_tab_row(
    menu_state: Res<LabMenuState>,
    mut cells: Query<(&ChromeTab, &mut BackgroundColor, &mut BorderColor)>,
    mut labels: Query<(&ChromeTabLabel, &mut TextColor, &mut TextFont)>,
    assets: Res<AssetServer>,
) {
    let active = menu_state.active_tab % StudioTab::ALL.len();
    for (tab, mut background, mut border) in &mut cells {
        let is_active = tab.0 == active;
        *background = BackgroundColor(if is_active {
            chrome(ChromeRole::Control)
        } else {
            Color::NONE
        });
        *border = BorderColor::all(if is_active {
            chrome(ChromeRole::Accent)
        } else {
            Color::NONE
        });
    }
    for (label, mut colour, mut font) in &mut labels {
        let role = if label.0 == active {
            typography::Role::TabActive
        } else {
            typography::Role::TabIdle
        };
        *colour = TextColor(role.colour());
        *font = typography::font(&assets, role);
    }
}
