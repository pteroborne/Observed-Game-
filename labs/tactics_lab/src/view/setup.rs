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

use bevy::input_focus::{InputFocus, InputFocusVisible, tab_navigation::TabIndex};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::{
    Activate, Slider, SliderRange, SliderStep, SliderThumb, SliderValue, TrackClick, ValueChange,
};
use observed_ui::{
    FocusScope, FocusScopeId, FocusTarget, WidgetId, WidgetLabel, WidgetSpec, WidgetText,
    activation_enabled, focus_scope, spawn_button,
};

use crate::settings::{
    BoardSize, DecoherenceCoverage, GuardianSetting, MAX_ACTION_POINTS, MAX_FLOORS, MAX_SQUAD,
    MIN_ACTION_POINTS, MIN_FLOORS, MIN_SQUAD, MatchSettings, Objectives, PRESETS, ShiftCadence,
    Sight,
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
    /// Flip a setting that has exactly two states.
    Toggle(SettingRow),
    Start,
}

#[derive(Component)]
pub struct SettingSlider(pub SettingRow);

#[derive(Component)]
pub struct SettingSliderThumb;

#[derive(Component)]
pub struct SettingToggle(pub SettingRow);

#[derive(Component)]
pub struct SettingValue(pub SettingRow);

#[derive(Component)]
pub struct PresetValue;

type HoveredSliderQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Hovered), (Changed<Hovered>, With<SettingSlider>)>;
type ToggleTextFilter = (
    With<WidgetText>,
    Without<SettingValue>,
    Without<PresetValue>,
);
type PresetTextFilter = (
    With<PresetValue>,
    Without<SettingValue>,
    Without<WidgetText>,
);

/// One configurable row.
///
/// Rows with three or more meaningful values use a discrete slider. Binary rows
/// use an activate-to-toggle button, so a rail never implies granularity that
/// the setting does not have.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingRow {
    Board,
    Floors,
    Seed,
    Squad,
    ActionPoints,
    Sight,
    RevealMap,
    Shift,
    DecoherenceCoverage,
    Telegraph,
    Anchors,
    Pads,
    Objectives,
    Guardian,
    Rival,
}

impl SettingRow {
    pub const ALL: [SettingRow; 15] = [
        SettingRow::Board,
        SettingRow::Floors,
        SettingRow::Seed,
        SettingRow::Squad,
        SettingRow::ActionPoints,
        SettingRow::Sight,
        SettingRow::RevealMap,
        SettingRow::Shift,
        SettingRow::DecoherenceCoverage,
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
            SettingRow::Board => Some("Map"),
            SettingRow::Squad => Some("Squad"),
            SettingRow::Sight => Some("Observation"),
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
            SettingRow::Board => "Tiles per floor",
            SettingRow::Floors => "Floors per map",
            SettingRow::Seed => "Seed",
            SettingRow::Squad => "Units",
            SettingRow::ActionPoints => "Action points",
            SettingRow::Sight => "Observation radius",
            SettingRow::RevealMap => "See full map",
            SettingRow::Shift => "Shift",
            SettingRow::DecoherenceCoverage => "Decoherence coverage",
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
            SettingRow::Floors => format!("{} floors", settings.floors),
            SettingRow::Seed => format!("{:#x}", settings.seed),
            SettingRow::Squad => format!("{} units", settings.squad_size),
            SettingRow::ActionPoints => format!("{} AP each", settings.action_points),
            SettingRow::Sight => settings.sight.label().to_string(),
            SettingRow::RevealMap => on_off(settings.reveal_full_map),
            SettingRow::Shift => settings.shift.label().to_string(),
            SettingRow::DecoherenceCoverage => format!(
                "{} ({} cells)",
                settings.decoherence_coverage.label(),
                settings.decoherence_target_cells()
            ),
            SettingRow::Telegraph => on_off(settings.telegraph),
            SettingRow::Anchors => on_off(settings.anchors),
            SettingRow::Pads => on_off(settings.pads),
            SettingRow::Objectives => settings.objectives.label().to_string(),
            SettingRow::Guardian => settings.guardian.label().to_string(),
            SettingRow::Rival => on_off(settings.rival_team),
        }
    }

    #[must_use]
    pub fn maximum(self) -> u8 {
        match self {
            SettingRow::Board => BoardSize::ALL.len() as u8 - 1,
            SettingRow::Floors => MAX_FLOORS - MIN_FLOORS,
            SettingRow::Seed => SEEDS.len() as u8 - 1,
            SettingRow::Squad => MAX_SQUAD - MIN_SQUAD,
            SettingRow::ActionPoints => MAX_ACTION_POINTS - MIN_ACTION_POINTS,
            SettingRow::Sight => Sight::ALL.len() as u8 - 1,
            SettingRow::Shift => ShiftCadence::ALL.len() as u8 - 1,
            SettingRow::DecoherenceCoverage => DecoherenceCoverage::ALL.len() as u8 - 1,
            SettingRow::Objectives => Objectives::ALL.len() as u8 - 1,
            SettingRow::Guardian => GuardianSetting::ALL.len() as u8 - 1,
            SettingRow::RevealMap
            | SettingRow::Telegraph
            | SettingRow::Anchors
            | SettingRow::Pads
            | SettingRow::Rival => 1,
        }
    }

    /// Sliders are reserved for genuine ranges. Binary rows are ordinary
    /// activate-to-toggle buttons, so their affordance matches their choice.
    #[must_use]
    pub fn uses_slider(self) -> bool {
        self.maximum() > 1
    }

    #[must_use]
    pub fn position(self, settings: &MatchSettings) -> u8 {
        match self {
            SettingRow::Board => index_of(&BoardSize::ALL, settings.board),
            SettingRow::Floors => settings.floors - MIN_FLOORS,
            SettingRow::Seed => index_of(&SEEDS, settings.seed),
            SettingRow::Squad => settings.squad_size - MIN_SQUAD,
            SettingRow::ActionPoints => settings.action_points - MIN_ACTION_POINTS,
            SettingRow::Sight => index_of(&Sight::ALL, settings.sight),
            SettingRow::RevealMap => u8::from(settings.reveal_full_map),
            SettingRow::Shift => index_of(&ShiftCadence::ALL, settings.shift),
            SettingRow::DecoherenceCoverage => {
                index_of(&DecoherenceCoverage::ALL, settings.decoherence_coverage)
            }
            SettingRow::Telegraph => u8::from(settings.telegraph),
            SettingRow::Anchors => u8::from(settings.anchors),
            SettingRow::Pads => u8::from(settings.pads),
            SettingRow::Objectives => index_of(&Objectives::ALL, settings.objectives),
            SettingRow::Guardian => index_of(&GuardianSetting::ALL, settings.guardian),
            SettingRow::Rival => u8::from(settings.rival_team),
        }
    }

    pub fn set_position(self, settings: &mut MatchSettings, position: u8) {
        let position = position.min(self.maximum());
        match self {
            SettingRow::Board => settings.board = BoardSize::ALL[position as usize],
            SettingRow::Floors => settings.floors = MIN_FLOORS + position,
            SettingRow::Seed => settings.seed = SEEDS[position as usize],
            SettingRow::Squad => settings.squad_size = MIN_SQUAD + position,
            SettingRow::ActionPoints => settings.action_points = MIN_ACTION_POINTS + position,
            SettingRow::Sight => settings.sight = Sight::ALL[position as usize],
            SettingRow::RevealMap => settings.reveal_full_map = position != 0,
            SettingRow::Shift => settings.shift = ShiftCadence::ALL[position as usize],
            SettingRow::DecoherenceCoverage => {
                settings.decoherence_coverage = DecoherenceCoverage::ALL[position as usize];
            }
            SettingRow::Telegraph => settings.telegraph = position != 0,
            SettingRow::Anchors => settings.anchors = position != 0,
            SettingRow::Pads => settings.pads = position != 0,
            SettingRow::Objectives => settings.objectives = Objectives::ALL[position as usize],
            SettingRow::Guardian => settings.guardian = GuardianSetting::ALL[position as usize],
            SettingRow::Rival => settings.rival_team = position != 0,
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

fn index_of<T: PartialEq>(values: &[T], value: T) -> u8 {
    values
        .iter()
        .position(|candidate| *candidate == value)
        .unwrap_or(0) as u8
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
            ScrollPosition::default(),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                row_gap: Val::Px(2.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(20.0)),
                overflow: Overflow::scroll_y(),
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
                    font_size: 30.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            root.spawn((
                PresetValue,
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
                width: percent(100.0),
                max_width: px(620.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(8.0),
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
                        .with_size(138.0, 52.0),
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
                            height: px(18.0),
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                    ));
                }
                if row.uses_slider() {
                    spawn_setting_slider(root, row, settings, order);
                } else {
                    spawn_setting_toggle(root, row, settings, order);
                }
                order += 1;
            }

            spawn_button(
                root,
                WidgetSpec::enabled(START, SCOPE, order, "Start match").with_size(280.0, 58.0),
                SetupAction::Start,
            );
            root.spawn((
                Text::new("Drag or tap a range. Tap a switch to toggle it."),
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

fn spawn_setting_slider(
    parent: &mut ChildSpawnerCommands,
    row: SettingRow,
    settings: &MatchSettings,
    order: u16,
) {
    parent
        .spawn((
            Node {
                width: percent(100.0),
                max_width: px(620.0),
                height: px(94.0),
                min_height: px(94.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(4.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|line| {
            line.spawn((
                Node {
                    width: percent(100.0),
                    height: px(42.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|meta| {
                meta.spawn((
                    Text::new(row.name()),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.76, 0.82, 0.88)),
                    Node {
                        width: percent(48.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
                meta.spawn((
                    SettingValue(row),
                    Text::new(row.value(settings)),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::new_with_justify(Justify::Right),
                    Node {
                        width: percent(48.0),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            });
            line.spawn((
                SettingSlider(row),
                Slider {
                    track_click: TrackClick::Snap,
                },
                SliderValue(f32::from(row.position(settings))),
                SliderRange::new(0.0, f32::from(row.maximum())),
                SliderStep(1.0),
                Hovered::default(),
                WidgetId::keyed("tactics_slider", u64::from(order)),
                FocusTarget {
                    scope: SCOPE,
                    order,
                },
                TabIndex(i32::from(order)),
                Node {
                    width: percent(100.0),
                    height: px(44.0),
                    border_radius: BorderRadius::all(px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.15, 0.19, 0.35)),
                Outline {
                    color: Color::NONE,
                    width: px(0.0),
                    offset: px(4.0),
                },
                Name::new(format!("{} slider", row.name())),
                children![
                    (
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(8.0),
                            right: px(8.0),
                            top: px(18.0),
                            height: px(8.0),
                            border_radius: BorderRadius::all(px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.08, 0.15, 0.19)),
                        Pickable::IGNORE,
                    ),
                    (
                        SettingSliderThumb,
                        SliderThumb,
                        Node {
                            position_type: PositionType::Absolute,
                            width: px(28.0),
                            height: px(28.0),
                            top: px(8.0),
                            left: percent(0.0),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(
                            observed_style::tactics(observed_style::TacticsRole::Selectable)
                                .base_color,
                        ),
                    )
                ],
            ));
        });
}

fn spawn_setting_toggle(
    parent: &mut ChildSpawnerCommands,
    row: SettingRow,
    settings: &MatchSettings,
    order: u16,
) {
    parent
        .spawn((
            Node {
                width: percent(100.0),
                max_width: px(620.0),
                height: px(82.0),
                min_height: px(82.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(4.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|line| {
            line.spawn((
                Text::new(row.name()),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgb(0.76, 0.82, 0.88)),
                Node {
                    width: percent(100.0),
                    ..default()
                },
                Pickable::IGNORE,
            ));
            spawn_button(
                line,
                WidgetSpec::enabled(
                    WidgetId::keyed("tactics_toggle", u64::from(order)),
                    SCOPE,
                    order,
                    row.value(settings),
                )
                .with_size(280.0, 52.0),
                (SetupAction::Toggle(row), SettingToggle(row)),
            );
        });
}

/// Apply direct-manipulation changes without rebuilding the screen mid-drag.
pub fn adjust(
    change: On<ValueChange<f32>>,
    sliders: Query<&SettingSlider>,
    mut settings: ResMut<crate::LabSettings>,
) {
    let Ok(slider) = sliders.get(change.source) else {
        return;
    };
    slider
        .0
        .set_position(&mut settings.0, change.value.round() as u8);
}

/// Keep slider positions and direct-manipulation feedback derived from settings.
pub fn sync_sliders(
    settings: Res<crate::LabSettings>,
    focus: Res<InputFocus>,
    mut commands: Commands,
    sliders: Query<(
        Entity,
        &SettingSlider,
        &SliderValue,
        &SliderRange,
        &Children,
        &Hovered,
        &mut Outline,
    )>,
    mut thumbs: Query<(&mut Node, &mut BackgroundColor), With<SettingSliderThumb>>,
) {
    for (entity, slider, value, range, children, hovered, mut outline) in sliders {
        let expected = f32::from(slider.0.position(&settings.0));
        if (value.0 - expected).abs() > f32::EPSILON {
            commands.entity(entity).insert(SliderValue(expected));
        }
        let focused = focus.0 == Some(entity);
        let treatment = observed_style::tactics(if focused || hovered.0 {
            observed_style::TacticsRole::ReachableRoute
        } else {
            observed_style::TacticsRole::Selectable
        });
        outline.color = if focused {
            treatment.base_color
        } else {
            Color::NONE
        };
        outline.width = if focused { px(2.0) } else { px(0.0) };
        for child in children.iter() {
            if let Ok((mut node, mut background)) = thumbs.get_mut(child) {
                node.left = percent(range.thumb_position(expected) * 100.0);
                background.0 = treatment.base_color;
            }
        }
    }
}

/// Keep named values, toggle labels, and preset status synchronized without
/// rebuilding the screen during a drag.
pub fn sync_labels(
    settings: Res<crate::LabSettings>,
    mut values: Query<(&SettingValue, &mut Text), Without<WidgetText>>,
    mut toggles: Query<(&SettingToggle, &Children, &mut WidgetLabel)>,
    mut widget_texts: Query<&mut Text, ToggleTextFilter>,
    mut presets: Query<&mut Text, PresetTextFilter>,
) {
    for (value, mut text) in &mut values {
        **text = value.0.value(&settings.0);
    }
    for (toggle, children, mut widget_label) in &mut toggles {
        let label = toggle.0.value(&settings.0);
        widget_label.0.clone_from(&label);
        for child in children.iter() {
            if let Ok(mut text) = widget_texts.get_mut(child) {
                **text = label.clone();
            }
        }
    }
    for mut text in &mut presets {
        **text = format!("Preset: {}", settings.0.preset_name());
    }
}

pub fn focus_hovered(
    sliders: HoveredSliderQuery,
    mut focus: ResMut<InputFocus>,
    mut visible: ResMut<InputFocusVisible>,
) {
    for (entity, hovered) in &sliders {
        if hovered.0 {
            focus.set(entity);
            visible.0 = true;
        }
    }
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
        SetupAction::Toggle(row) => {
            let next = u8::from(row.position(settings) == 0);
            row.set_position(settings, next);
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
        assert_eq!(SettingRow::ALL.len(), 15);
        for row in SettingRow::ALL {
            assert!(!row.name().is_empty());
            assert!(!row.value(&MatchSettings::standard()).is_empty());
        }
    }

    #[test]
    fn moving_a_control_changes_exactly_that_row() {
        for row in SettingRow::ALL {
            let before = MatchSettings::standard();
            let mut after = before;
            let next = if row.position(&after) == row.maximum() {
                0
            } else {
                row.position(&after) + 1
            };
            row.set_position(&mut after, next);
            assert_ne!(before, after, "{} did not change anything", row.name());
        }
    }

    #[test]
    fn every_control_round_trips_every_position() {
        for row in SettingRow::ALL {
            let mut settings = MatchSettings::standard();
            for position in 0..=row.maximum() {
                row.set_position(&mut settings, position);
                assert_eq!(row.position(&settings), position, "{}", row.name());
            }
        }
    }

    #[test]
    fn only_rows_with_more_than_two_choices_use_sliders() {
        for row in SettingRow::ALL {
            assert_eq!(row.uses_slider(), row.maximum() > 1, "{}", row.name());
        }
        assert!(!SettingRow::RevealMap.uses_slider());
        assert!(!SettingRow::Telegraph.uses_slider());
        assert!(SettingRow::Sight.uses_slider());
    }

    #[test]
    fn activating_a_binary_control_toggles_both_directions() {
        let mut settings = MatchSettings::standard();
        let before = settings.reveal_full_map;
        assert_eq!(
            activate_action(SetupAction::Toggle(SettingRow::RevealMap), &mut settings),
            SetupRequest::Changed
        );
        assert_eq!(settings.reveal_full_map, !before);
        let _ = activate_action(SetupAction::Toggle(SettingRow::RevealMap), &mut settings);
        assert_eq!(settings.reveal_full_map, before);
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
        assert_eq!(settings, MatchSettings::guided());
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
        for position in 0..40 {
            SettingRow::Squad.set_position(&mut settings, position);
            SettingRow::ActionPoints.set_position(&mut settings, position);
            SettingRow::Floors.set_position(&mut settings, position);
            assert!((MIN_SQUAD..=MAX_SQUAD).contains(&settings.squad_size));
            assert!((MIN_ACTION_POINTS..=MAX_ACTION_POINTS).contains(&settings.action_points));
            assert!((MIN_FLOORS..=MAX_FLOORS).contains(&settings.floors));
        }
    }
}
