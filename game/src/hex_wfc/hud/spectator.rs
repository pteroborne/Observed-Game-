//! Spectator chrome describes the existing camera adapter's controls.
use super::play::{Field, Panel, panel as background, text};
use crate::hex_wfc::view::spectate::SpectatorOverview;
use crate::settings::{Settings, key_name};
use crate::view::theme::{DIM, TITLE};
use bevy::prelude::*;

pub(super) fn panel() -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            top: px(20),
            right: px(20),
            max_width: percent(38),
            padding: UiRect::axes(px(14), px(10)),
            border_radius: BorderRadius::all(px(3)),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            ..default()
        },
        background(Panel::Spectator),
        children![
            text(Field::SpectatorMode, 15.0, TITLE),
            text(Field::SpectatorControls, 13.0, DIM)
        ],
    )
}

pub(super) fn mode(overview: &SpectatorOverview) -> String {
    format!(
        "SPECTATING / {}",
        if overview.active {
            "OVERVIEW"
        } else if overview.eyes {
            "OBSERVER'S EYES"
        } else {
            "CHASE CAMERA"
        }
    )
}

pub(super) fn controls(settings: &Settings) -> String {
    format!(
        "F / D-pad left  next Observer\nO / Y  overview    V / X  eyes\nR / D-pad right  rotate overview\n[ ] / D-pad down up  zoom overview\n{} / RB  team map    {} / Start  pause",
        key_name(settings.bindings.tac_map),
        key_name(settings.bindings.pause)
    )
}
