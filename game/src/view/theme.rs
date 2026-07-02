//! The shared UI theme: the neon-noir palette every screen draws from, the per-team
//! colours, and the small bundle helpers (screen root, panel, text) the menu-like
//! screens compose. Gameplay colours (markers, surfaces, districts) do NOT live here —
//! they come from `observed_style`; this is chrome for menus/HUD panels only.

use bevy::prelude::*;
use observed_match::facility::TEAM_COUNT;

use crate::GameState;

pub(crate) const TITLE: Color = Color::srgb(0.95, 0.97, 1.0);
pub(crate) const ACCENT: Color = Color::srgb(0.40, 0.92, 1.0);
pub(crate) const DIM: Color = Color::srgb(0.58, 0.64, 0.74);
pub(crate) const PANEL: Color = Color::srgba(0.04, 0.06, 0.10, 0.92);
pub(crate) const BORDER: Color = Color::srgba(0.40, 0.92, 1.0, 0.5);

pub(crate) const TEAM_COLORS: [Color; TEAM_COUNT] = [
    Color::srgb(0.96, 0.28, 0.34),
    Color::srgb(0.32, 0.62, 1.0),
    Color::srgb(0.72, 0.46, 1.0),
    Color::srgb(1.0, 0.62, 0.20),
];

/// Tac-map overlay panel size (pixels); the per-element layout lives in the HUD module.
pub(crate) const TAC_MAP_SIZE: f32 = 300.0;

/// Marks the root UI node of the currently active screen (exactly one alive at a time;
/// despawned on state exit).
#[derive(Component)]
pub(crate) struct ScreenRoot;

pub(crate) fn screen_root(state: GameState) -> impl Bundle {
    (
        ScreenRoot,
        DespawnOnExit(state),
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(10),
            ..default()
        },
    )
}

pub(crate) fn panel() -> impl Bundle {
    (
        Node {
            min_width: px(560),
            padding: UiRect::all(px(26)),
            border: UiRect::all(px(1)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(8),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(BORDER),
    )
}

pub(crate) fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s.into()),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}
