//! The bridge between Bevy's `feathers` widget set and this project's chrome
//! palette.
//!
//! `feathers` ships a dark theme of its own. Using it unmodified would give the
//! studio a look that belongs to no other surface in this repo, so the theme is
//! built in two passes: start from `create_dark_theme` so all 137 tokens are
//! populated, then overwrite everything the studio actually renders with
//! `observed_ui::theme`, which is the documented home for chrome colour.
//!
//! Starting from the full dark theme is not laziness - `UiTheme::color` returns
//! bright fuchsia and logs a warning for any token it does not hold, and
//! `FeathersCorePlugin` installs an *empty* theme. A partial map would paint
//! fuchsia the first time a widget reached for a token nobody had thought about.
//!
//! Gameplay colour is not in here. Pinned/volatile/selected still come from
//! `observed_style`, because those carry solver meaning rather than chrome.

use bevy::feathers::theme::UiTheme;
use bevy::feathers::{dark_theme::create_dark_theme, tokens};
use bevy::prelude::*;
use observed_ui::theme::{ChromeRole, chrome};

/// Install the themed `feathers` palette.
///
/// Runs as a startup system rather than in `Plugin::build` so it lands after
/// `FeathersCorePlugin` has inserted the resource, whatever order the plugins
/// were added in.
pub fn apply_studio_theme(mut commands: Commands) {
    let mut theme = UiTheme(create_dark_theme());

    let surface = chrome(ChromeRole::Surface);
    let control = chrome(ChromeRole::Control);
    let hover = chrome(ChromeRole::ControlHover);
    let pressed = chrome(ChromeRole::ControlPressed);
    let disabled = chrome(ChromeRole::ControlDisabled);
    let fill = chrome(ChromeRole::ControlFill);
    let border = chrome(ChromeRole::Border);
    let accent = chrome(ChromeRole::Accent);
    let text = chrome(ChromeRole::TextMain);
    let dim = chrome(ChromeRole::TextDim);
    let text_off = chrome(ChromeRole::TextDisabled);

    // Typed constants rather than the `&str` overload throughout: `set_color`
    // takes a string, so a typo would silently theme nothing and show up as one
    // fuchsia widget much later.
    for (token, colour) in [
        // Window and text.
        (tokens::WINDOW_BG, surface),
        (tokens::FOCUS_RING, accent),
        (tokens::TEXT_MAIN, text),
        (tokens::TEXT_DIM, dim),
        // Buttons. The studio's only "primary" action is a confirm, so primary
        // borrows the pressed fill to read as louder than a plain button.
        (tokens::BUTTON_BG, control),
        (tokens::BUTTON_BG_HOVER, hover),
        (tokens::BUTTON_BG_PRESSED, pressed),
        (tokens::BUTTON_BG_DISABLED, disabled),
        (tokens::BUTTON_TEXT, text),
        (tokens::BUTTON_TEXT_DISABLED, text_off),
        (tokens::BUTTON_PRIMARY_BG, fill),
        (tokens::BUTTON_PRIMARY_BG_HOVER, hover),
        (tokens::BUTTON_PRIMARY_BG_PRESSED, pressed),
        (tokens::BUTTON_PRIMARY_BG_DISABLED, disabled),
        (tokens::BUTTON_PRIMARY_TEXT, text),
        (tokens::BUTTON_PRIMARY_TEXT_DISABLED, text_off),
        (tokens::BUTTON_PLAIN_BG, Color::NONE),
        (tokens::BUTTON_PLAIN_BG_HOVER, hover),
        (tokens::BUTTON_PLAIN_BG_PRESSED, pressed),
        (tokens::BUTTON_PLAIN_BG_DISABLED, Color::NONE),
        // Sliders: the control the tuning tab is made of.
        (tokens::SLIDER_BG, control),
        (tokens::SLIDER_BG_HOVER, hover),
        (tokens::SLIDER_BG_PRESSED, hover),
        (tokens::SLIDER_BG_DISABLED, disabled),
        (tokens::SLIDER_BAR, fill),
        (tokens::SLIDER_BAR_HOVER, pressed),
        (tokens::SLIDER_BAR_PRESSED, pressed),
        (tokens::SLIDER_BAR_DISABLED, disabled),
        (tokens::SLIDER_TEXT, text),
        (tokens::SLIDER_TEXT_DISABLED, text_off),
        // Scrollbars, for the panels that outgrow the column.
        (tokens::SCROLLBAR_BG, surface),
        (tokens::SCROLLBAR_THUMB, control),
        (tokens::SCROLLBAR_THUMB_HOVER, hover),
        // Checkboxes.
        (tokens::CHECKBOX_BG, control),
        (tokens::CHECKBOX_BG_HOVER, hover),
        (tokens::CHECKBOX_BG_PRESSED, pressed),
        (tokens::CHECKBOX_BG_DISABLED, disabled),
        (tokens::CHECKBOX_BG_CHECKED, fill),
        (tokens::CHECKBOX_BG_CHECKED_HOVER, hover),
        (tokens::CHECKBOX_BG_CHECKED_PRESSED, pressed),
        (tokens::CHECKBOX_BG_CHECKED_DISABLED, disabled),
        (tokens::CHECKBOX_BORDER, border),
        (tokens::CHECKBOX_BORDER_HOVER, accent),
        (tokens::CHECKBOX_BORDER_PRESSED, accent),
        (tokens::CHECKBOX_BORDER_DISABLED, text_off),
        (tokens::CHECKBOX_BORDER_CHECKED, accent),
        (tokens::CHECKBOX_BORDER_CHECKED_HOVER, accent),
        (tokens::CHECKBOX_BORDER_CHECKED_PRESSED, accent),
        (tokens::CHECKBOX_BORDER_CHECKED_DISABLED, text_off),
        (tokens::CHECKBOX_MARK, text),
        (tokens::CHECKBOX_MARK_DISABLED, text_off),
        (tokens::CHECKBOX_TEXT, text),
        (tokens::CHECKBOX_TEXT_DISABLED, text_off),
        // Panes and groups: the containers the panel is about to be built from.
        (tokens::PANE_HEADER_BG, control),
        (tokens::PANE_HEADER_BORDER, border),
        (tokens::PANE_HEADER_TEXT, text),
        (tokens::PANE_HEADER_DIVIDER, border),
        (tokens::PANE_BODY_BG, surface),
        (tokens::SUBPANE_HEADER_BG, control),
        (tokens::SUBPANE_HEADER_BORDER, border),
        (tokens::SUBPANE_HEADER_TEXT, text),
        (tokens::SUBPANE_BODY_BG, surface),
        (tokens::SUBPANE_BODY_BORDER, border),
        (tokens::GROUP_HEADER_BG, control),
        (tokens::GROUP_HEADER_BORDER, border),
        (tokens::GROUP_HEADER_TEXT, text),
        (tokens::GROUP_BODY_BG, surface),
        (tokens::GROUP_BODY_BORDER, border),
        // List rows, for the tabular panels.
        (tokens::LISTROW_BG, Color::NONE),
        (tokens::LISTROW_BG_HOVER, hover),
        (tokens::LISTROW_BG_SELECTED, fill),
        (tokens::LISTROW_TEXT, text),
        (tokens::LISTROW_TEXT_DISABLED, text_off),
        // Menus.
        (tokens::MENU_BG, surface),
        (tokens::MENU_BORDER, border),
        (tokens::MENUITEM_BG_HOVER, hover),
        (tokens::MENUITEM_BG_PRESSED, pressed),
        (tokens::MENUITEM_BG_FOCUSED, control),
        (tokens::MENUITEM_TEXT, text),
        (tokens::MENUITEM_TEXT_DISABLED, text_off),
    ] {
        theme.0.color.insert(token, colour);
    }

    commands.insert_resource(theme);
}
