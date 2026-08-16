//! Where the panel sits, as opposed to what it says.
//!
//! Split from `chrome` when that file reached the 600-line review budget. The
//! division is the one the panel itself has: `chrome` builds the tree and keeps
//! its text current, and this decides which edge the tree is docked to.

use bevy::prelude::*;

use crate::chrome::{ChromeActionBar, ChromeMenuRoot, ChromeShellRoot, ChromeViewportColumn};

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
}
