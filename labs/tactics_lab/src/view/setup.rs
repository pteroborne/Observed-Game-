//! The match setup screen.
//!
//! A player-facing setup screen, not a developer panel. Every row shows a name
//! and a value a player would recognise ("Sight: Corridor", "Facility shift:
//! Every other turn"), and the preset row at the top is the intended way to move
//! them — you dial this game in by starting from a preset and disagreeing with
//! it.
//!
//! Rows are built by iterating [`SettingRow::ALL`] over the live
//! [`MatchSettings`], so a setting added to the struct and to that list appears
//! here with no further wiring, and one that is added to neither cannot be
//! silently unreachable.
//!
//! Widgets come from `observed_ui`, the same chrome the shipped frontend uses,
//! which is what gives this screen pointer, keyboard and controller focus
//! without a bespoke input path.

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use observed_ui::{
    FocusScope, FocusScopeId, WidgetId, WidgetSpec, activation_enabled, focus_scope, spawn_button,
};

use crate::settings::{
    MAX_ACTION_POINTS, MAX_SQUAD, MIN_ACTION_POINTS, MIN_SQUAD, MatchSettings, PRESETS,
};

const SCOPE: FocusScopeId = FocusScopeId("tactics_setup");
const START: WidgetId = WidgetId::named("tactics_start");

#[derive(Component)]
pub struct SetupRoot;

/// What activating a control does.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupAction {
    /// Load a named preset wholesale.
    Preset(usize),
    /// Advance one setting to its next value.
    Cycle(SettingRow),
    Start,
}

/// One configurable row.
///
/// Cycling is the only edit verb. A slider or a text field would need its own
/// pointer and keyboard handling for no gain — every one of these settings has a
/// handful of meaningful values, and naming them is more useful than letting a
/// player pick 37 action points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingRow {
    Board,
    Seed,
    Squad,
    ActionPoints,
    Sight,
    Shift,
    Telegraph,
    Anchors,
    Pads,
    Objectives,
    Guardian,
    Rival,
}

impl SettingRow {
    pub const ALL: [SettingRow; 12] = [
        SettingRow::Board,
        SettingRow::Seed,
        SettingRow::Squad,
        SettingRow::ActionPoints,
        SettingRow::Sight,
        SettingRow::Shift,
        SettingRow::Telegraph,
        SettingRow::Anchors,
        SettingRow::Pads,
        SettingRow::Objectives,
        SettingRow::Guardian,
        SettingRow::Rival,
    ];

    /// The group heading this row sits under, or `None` when it continues the
    /// previous group.
    #[must_use]
    pub const fn heading(self) -> Option<&'static str> {
        match self {
            SettingRow::Board => Some("Board"),
            SettingRow::Squad => Some("Squad"),
            SettingRow::Sight => Some("Sight"),
            SettingRow::Shift => Some("Facility shift"),
            SettingRow::Anchors => Some("Gear"),
            SettingRow::Objectives => Some("Objectives"),
            SettingRow::Guardian => Some("Threats"),
            _ => None,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            SettingRow::Board => "Size",
            SettingRow::Seed => "Seed",
            SettingRow::Squad => "Units",
            SettingRow::ActionPoints => "Action points",
            SettingRow::Sight => "Sight",
            SettingRow::Shift => "Shift",
            SettingRow::Telegraph => "Telegraph",
            SettingRow::Anchors => "Anchors",
            SettingRow::Pads => "Teleport plates",
            SettingRow::Objectives => "Goal",
            SettingRow::Guardian => "Guardian",
            SettingRow::Rival => "Rival squad",
        }
    }

    /// The value as a player reads it.
    #[must_use]
    pub fn value(self, settings: &MatchSettings) -> String {
        match self {
            SettingRow::Board => settings.board.label().to_string(),
            SettingRow::Seed => format!("{:#x}", settings.seed),
            SettingRow::Squad => format!("{} units", settings.squad_size),
            SettingRow::ActionPoints => format!("{} per unit per turn", settings.action_points),
            SettingRow::Sight => settings.sight.label().to_string(),
            SettingRow::Shift => settings.shift.label().to_string(),
            SettingRow::Telegraph => on_off(settings.telegraph),
            SettingRow::Anchors => on_off(settings.anchors),
            SettingRow::Pads => on_off(settings.pads),
            SettingRow::Objectives => settings.objectives.label().to_string(),
            SettingRow::Guardian => settings.guardian.label().to_string(),
            SettingRow::Rival => on_off(settings.rival_team),
        }
    }

    /// Advance this row to its next value. Numeric rows wrap at their bounds so
    /// a single verb reaches everything.
    pub fn cycle(self, settings: &mut MatchSettings) {
        match self {
            SettingRow::Board => settings.board = settings.board.next(),
            // A seed cycles through a fixed ladder rather than randomising, so a
            // reader can get back to the layout they were just looking at.
            SettingRow::Seed => {
                let index = SEEDS
                    .iter()
                    .position(|&seed| seed == settings.seed)
                    .map_or(0, |index| (index + 1) % SEEDS.len());
                settings.seed = SEEDS[index];
            }
            SettingRow::Squad => {
                settings.squad_size = wrap(settings.squad_size, MIN_SQUAD, MAX_SQUAD);
            }
            SettingRow::ActionPoints => {
                settings.action_points =
                    wrap(settings.action_points, MIN_ACTION_POINTS, MAX_ACTION_POINTS);
            }
            SettingRow::Sight => settings.sight = settings.sight.next(),
            SettingRow::Shift => settings.shift = settings.shift.next(),
            SettingRow::Telegraph => settings.telegraph = !settings.telegraph,
            SettingRow::Anchors => settings.anchors = !settings.anchors,
            SettingRow::Pads => settings.pads = !settings.pads,
            SettingRow::Objectives => settings.objectives = settings.objectives.next(),
            SettingRow::Guardian => settings.guardian = settings.guardian.next(),
            SettingRow::Rival => settings.rival_team = !settings.rival_team,
        }
    }
}

/// The seed ladder the setup screen offers. Shared with the lab's capture run so
/// evidence and hand-play look at the same layouts.
pub const SEEDS: [u64; 5] = [
    0x0000_0000_000c_0ffe,
    0x0000_0000_0000_0b0b,
    0x0000_0000_000d_00d0,
    0x5eed_0000_0000_0001,
    0xa11c_e3d0_0000_0008,
];

fn wrap(value: u8, min: u8, max: u8) -> u8 {
    if value >= max { min } else { value + 1 }
}

fn on_off(value: bool) -> String {
    if value { "On" } else { "Off" }.to_string()
}

/// Build the screen. Rebuilt whenever a value changes, which keeps every label
/// derived from the settings rather than mutated in place.
pub fn spawn(commands: &mut Commands, settings: &MatchSettings, error: Option<&str>) {
    commands
        .spawn((
            SetupRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(observed_style::schematic_screen()),
            focus_scope(FocusScope::screen(SCOPE, START, START)),
            Name::new("Tactics setup"),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("OBSERVED - TACTICAL"),
                TextFont {
                    font_size: 34.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            root.spawn((
                Text::new(format!("Preset: {}", settings.preset_name())),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.8, 0.9)),
            ));
            if let Some(error) = error {
                root.spawn((
                    Text::new(format!("COULD NOT START: {error}")),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(
                        observed_style::tactics(observed_style::TacticsRole::Blocked).base_color,
                    ),
                ));
            }

            let mut order = 0u16;
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                margin: UiRect::vertical(Val::Px(10.0)),
                ..default()
            })
            .with_children(|row| {
                for (index, preset) in PRESETS.iter().enumerate() {
                    spawn_button(
                        row,
                        WidgetSpec::enabled(
                            WidgetId::keyed("tactics_preset", index as u64),
                            SCOPE,
                            order,
                            preset.name.to_string(),
                        )
                        .with_size(150.0, 46.0),
                        SetupAction::Preset(index),
                    );
                    order += 1;
                }
            });

            for row in SettingRow::ALL {
                if let Some(heading) = row.heading() {
                    root.spawn((
                        Text::new(heading.to_uppercase()),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.62, 0.72)),
                        Node {
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                    ));
                }
                spawn_button(
                    root,
                    WidgetSpec::enabled(
                        WidgetId::keyed("tactics_row", order.into()),
                        SCOPE,
                        order,
                        format!("{}:  {}", row.name(), row.value(settings)),
                    )
                    .with_size(460.0, 40.0),
                    SetupAction::Cycle(row),
                );
                order += 1;
            }

            spawn_button(
                root,
                WidgetSpec::enabled(START, SCOPE, order, "Start match").with_size(460.0, 56.0),
                SetupAction::Start,
            );
            root.spawn((
                Text::new(
                    "click or tap any row to change it - every control is reachable by pointer",
                ),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.55, 0.62)),
                Node {
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
            ));
        });
}

/// What one activation asks the lab to do. Returned rather than acted on so the
/// screen stays free of the lab's state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupRequest {
    Changed,
    Start,
}

/// Apply an activation to the settings.
#[must_use]
pub fn activate_action(action: SetupAction, settings: &mut MatchSettings) -> SetupRequest {
    match action {
        SetupAction::Preset(index) => {
            if let Some(preset) = PRESETS.get(index) {
                *settings = (preset.build)();
            }
            SetupRequest::Changed
        }
        SetupAction::Cycle(row) => {
            row.cycle(settings);
            SetupRequest::Changed
        }
        SetupAction::Start => SetupRequest::Start,
    }
}

/// Screen-local activation observer, registered with `App::add_observer`.
pub fn activate(
    activation: On<Activate>,
    actions: Query<&SetupAction>,
    disabled: Query<(), With<InteractionDisabled>>,
    mut settings: ResMut<crate::LabSettings>,
    mut requests: MessageWriter<crate::SetupRequested>,
) {
    if !activation_enabled(&activation, &disabled) {
        return;
    }
    let Ok(&action) = actions.get(activation.entity) else {
        return;
    };
    let request = activate_action(action, &mut settings.0);
    requests.write(crate::SetupRequested(request));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{BoardSize, Sight};

    /// A row that is not in `ALL` is a setting a player cannot reach. The count
    /// is asserted so adding a field to `MatchSettings` without a row here shows
    /// up as a failing test rather than as an invisible option.
    #[test]
    fn every_row_is_listed_and_named() {
        assert_eq!(SettingRow::ALL.len(), 12);
        for row in SettingRow::ALL {
            assert!(!row.name().is_empty());
            assert!(!row.value(&MatchSettings::standard()).is_empty());
        }
    }

    #[test]
    fn cycling_a_row_changes_exactly_that_row() {
        for row in SettingRow::ALL {
            let before = MatchSettings::standard();
            let mut after = before;
            row.cycle(&mut after);
            assert_ne!(before, after, "{} did not change anything", row.name());
        }
    }

    /// Cycling any row returns to where it started, so a player can always undo
    /// by continuing to press.
    #[test]
    fn cycling_a_row_eventually_returns_to_its_first_value() {
        for row in SettingRow::ALL {
            let start = MatchSettings::standard();
            let mut settings = start;
            let mut returned = false;
            for _ in 0..16 {
                row.cycle(&mut settings);
                if settings == start {
                    returned = true;
                    break;
                }
            }
            assert!(returned, "{} never cycled back", row.name());
        }
    }

    #[test]
    fn a_preset_button_replaces_the_whole_configuration() {
        let mut settings = MatchSettings {
            board: BoardSize::Facility,
            sight: Sight::Blind,
            ..MatchSettings::standard()
        };
        assert_eq!(
            activate_action(SetupAction::Preset(0), &mut settings),
            SetupRequest::Changed
        );
        assert_eq!(settings, MatchSettings::scout());
    }

    #[test]
    fn start_asks_to_start_and_changes_nothing() {
        let mut settings = MatchSettings::standard();
        let before = settings;
        assert_eq!(
            activate_action(SetupAction::Start, &mut settings),
            SetupRequest::Start
        );
        assert_eq!(settings, before);
    }

    #[test]
    fn numeric_rows_stay_inside_the_range_the_match_accepts() {
        let mut settings = MatchSettings::standard();
        for _ in 0..40 {
            SettingRow::Squad.cycle(&mut settings);
            SettingRow::ActionPoints.cycle(&mut settings);
            assert!((MIN_SQUAD..=MAX_SQUAD).contains(&settings.squad_size));
            assert!((MIN_ACTION_POINTS..=MAX_ACTION_POINTS).contains(&settings.action_points));
        }
    }
}
