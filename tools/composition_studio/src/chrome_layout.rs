//! Where the panel sits, as opposed to what it says.
//!
//! Split from `chrome` when that file reached the 600-line review budget. The
//! division is the one the panel itself has: `chrome` builds the tree and keeps
//! its text current, and this decides which edge the tree is docked to.

use bevy::prelude::*;

use crate::chrome::ChromeMenuRoot;

/// The full-window container the panel and the facility share.
///
/// Marked so the layout can turn its axis: the panel sits beside the facility
/// on a desktop and beneath it on a phone.
#[derive(Component)]
pub struct ChromeShellRoot;

/// The column holding the facility and its chrome, beside or above the panel.
#[derive(Component)]
pub struct ChromeViewportColumn;

/// The keyboard cheat sheet. Hidden where there is no keyboard.
#[derive(Component)]
pub struct ChromeActionBar;

/// Tap target that collapses and restores the panel.
///
/// Lives in the viewport column rather than the panel, because a control
/// inside the panel disappears with it and leaves a touch device no way to
/// bring the tools back. `F2` does the same job for a keyboard.
#[derive(Component)]
pub struct ChromePanelToggle;

/// The toggle's label, which says what the tap will do next.
#[derive(Component)]
pub struct ChromePanelToggleLabel;

/// Dock the panel to the edge the current layout calls for.
///
/// Separate from [`update_chrome_ui`], which owns what the panel *says*. This
/// owns where it is: on a desktop it is a full-height column beside the
/// facility, and on a phone a full-width block along the foot, because
/// [`crate::PANEL_WIDTH`] alone is wider than a handset.
#[allow(clippy::type_complexity)] // Four disjoint node queries in one system.
pub fn sync_chrome_layout(
    state: Res<crate::StudioState>,
    mut shell_query: Query<&mut Node, (With<ChromeShellRoot>, Without<ChromeMenuRoot>)>,
    mut panel_query: Query<&mut Node, With<ChromeMenuRoot>>,
    mut column_query: Query<
        &mut Node,
        (
            With<ChromeViewportColumn>,
            Without<ChromeShellRoot>,
            Without<ChromeMenuRoot>,
        ),
    >,
    mut action_query: Query<
        &mut Node,
        (
            With<ChromeActionBar>,
            Without<ChromeShellRoot>,
            Without<ChromeMenuRoot>,
            Without<ChromeViewportColumn>,
        ),
    >,
    mut toggle_query: Query<
        &mut Node,
        (
            With<ChromePanelToggle>,
            Without<ChromeShellRoot>,
            Without<ChromeMenuRoot>,
            Without<ChromeViewportColumn>,
            Without<ChromeActionBar>,
        ),
    >,
    mut toggle_label: Query<&mut Text, With<ChromePanelToggleLabel>>,
) {
    let compact = state.layout.is_compact();
    if let Ok(mut shell) = shell_query.single_mut() {
        // Reversed, so the panel is the last child but the lowest on screen.
        shell.flex_direction = if compact {
            FlexDirection::ColumnReverse
        } else {
            FlexDirection::Row
        };
    }
    if let Ok(mut panel) = panel_query.single_mut() {
        // The panel docks to exactly one edge, and which edge decides both its
        // size and which side carries the divider.
        if compact {
            panel.width = Val::Percent(100.0);
            panel.height = Val::Percent(crate::COMPACT_PANEL_FRACTION * 100.0);
            panel.border = UiRect::top(Val::Px(2.0));
        } else {
            panel.width = Val::Px(crate::PANEL_WIDTH);
            panel.height = Val::Percent(100.0);
            panel.border = UiRect::right(Val::Px(2.0));
        }
    }
    if let Ok(mut column) = column_query.single_mut() {
        // A full height beside the panel; whatever is left above it. Leaving
        // this at 100% in a compact layout pushes the facility's chrome down
        // over the panel, because both claim the whole window.
        column.height = if compact {
            Val::Auto
        } else {
            Val::Percent(100.0)
        };
    }
    if let Ok(mut action) = action_query.single_mut() {
        // The action bar is a keyboard cheat sheet - "[Up/Dn] select a control",
        // "F2 collapse the panel". On a touch screen it is advice that cannot
        // be taken, printed over the facility it is describing.
        action.display = if compact {
            Display::None
        } else {
            Display::Flex
        };
    }
    if let Ok(mut toggle) = toggle_query.single_mut() {
        // Desktop already has F2 and an action bar that says so, and the
        // showcase captures are an evidence contract - so this appears only
        // where the keyboard does not.
        toggle.display = if compact {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut label) = toggle_label.single_mut() {
        // Says what the next tap does, not what is currently true.
        let wanted = if state.panel_open { "HIDE" } else { "TOOLS" };
        if label.as_str() != wanted {
            **label = wanted.to_string();
        }
    }
}
