//! Real slider controls for the Tuning tab.
//!
//! The tab used to draw its eighteen controls as `[----O-------]` text. That is
//! honest about the value and silent about everything else: it does not say the
//! control is draggable, it does not say where the pointer has to go, and it
//! offers nothing to a mouse at all. The complaint it produced was not "I can't
//! read this" but "I don't know how to interact with it".
//!
//! These are `bevy_ui_widgets` sliders, so dragging works and the hit target is
//! a real rect rather than a run of hyphens. The keyboard scheme is unchanged
//! and still authoritative: `bevy_ui_widgets`' own arrow-key handling runs off
//! `InputFocus`, which this tool never sets, so Lt/Rt cannot be claimed twice.
//! That is the same rule the docked panel enforces - a key never means two
//! things at once - held at the widget layer.
//!
//! Data flows one way. The profile is the single source of truth; the slider
//! reports a change and never writes its own `SliderValue`. `sync_tuning_rows`
//! pushes the profile back out. Letting the widget hold its own copy would give
//! the tool two versions of a hashed artifact, and the one you could see would
//! not be the one you would save.

use bevy::prelude::*;
use bevy::ui_widgets::{Slider, SliderRange, SliderStep, SliderThumb, SliderValue, ValueChange};
use observed_style::{SchematicRole, schematic};

use crate::tunables::{TUNABLE_FIELDS, TunableField};
use crate::{LabMenuState, StudioState, StudioTab};

/// Track length, and thumb width, in logical pixels.
///
/// The core slider subtracts the measured thumb size from the travel so the
/// thumb tracks the pointer exactly, so the visual placement below must do the
/// same or dragging drifts from the cursor at the ends.
const TRACK_WIDTH: f32 = 150.0;
const THUMB_WIDTH: f32 = 10.0;
const ROW_HEIGHT: f32 = 16.0;
/// The unfilled part of a track: visible enough to show the control's extent,
/// dim enough that eighteen of them do not become the loudest thing on screen.
const TRACK_ALPHA: f32 = 0.22;
const LABEL_WIDTH: f32 = 180.0;
const VALUE_WIDTH: f32 = 52.0;

/// Container for every tuning row; hidden wholesale when another tab is up.
#[derive(Component)]
pub struct TuningRoot;

/// Index of the field **within the Tuning tab**, which is what
/// `LabMenuState::selected_item` counts. Not an index into [`TUNABLE_FIELDS`].
#[derive(Component)]
pub struct TuningSlider(pub usize);

/// The moving part of row `.0`, positioned by [`sync_tuning_rows`].
#[derive(Component)]
pub struct TuningThumb(pub usize);

#[derive(Component)]
pub struct TuningNameLabel(pub usize);

#[derive(Component)]
pub struct TuningValueLabel(pub usize);

/// One line of copy for the selected row, so the panel explains the control the
/// author is actually holding rather than all eighteen at once.
#[derive(Component)]
pub struct TuningConsequence;

/// The fields shown on the Tuning tab, paired with their tab-local index.
pub fn tuning_fields() -> impl Iterator<Item = (usize, &'static TunableField)> {
    TUNABLE_FIELDS
        .iter()
        .filter(|field| field.tab == StudioTab::Tuning)
        .enumerate()
}

/// Spawn the row list. Called from `setup_chrome` inside the docked panel, as a
/// sibling *after* the panel text node: on the Tuning tab that text holds only
/// the header, and these rows are the body.
pub fn spawn_tuning_rows(panel: &mut ChildSpawnerCommands) {
    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            TuningRoot,
        ))
        .with_children(|list| {
            for (index, field) in tuning_fields() {
                list.spawn(Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(ROW_HEIGHT),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Node {
                            width: Val::Px(LABEL_WIDTH),
                            ..default()
                        },
                        Text::new(field.label),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(schematic(SchematicRole::Grid).base_color),
                        TuningNameLabel(index),
                    ));

                    row.spawn((
                        Node {
                            width: Val::Px(TRACK_WIDTH),
                            height: Val::Px(ROW_HEIGHT - 4.0),
                            ..default()
                        },
                        // Track and thumb draw only from `Grid` and `Selected`.
                        // `Pinned` and `Volatile` are spoken for: green and red
                        // mean "the solver will not / will rewire this" in the
                        // viewport legend, and a red slider track would be
                        // making a claim about the facility that it is not
                        // making.
                        BackgroundColor(
                            schematic(SchematicRole::Grid)
                                .base_color
                                .with_alpha(TRACK_ALPHA),
                        ),
                        Slider::default(),
                        #[allow(clippy::cast_possible_truncation)]
                        SliderRange::new(field.min as f32, field.max as f32),
                        #[allow(clippy::cast_possible_truncation)]
                        SliderStep(field.step as f32),
                        TuningSlider(index),
                    ))
                    .with_children(|track| {
                        track.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                width: Val::Px(THUMB_WIDTH),
                                height: Val::Px(ROW_HEIGHT - 4.0),
                                ..default()
                            },
                            BackgroundColor(schematic(SchematicRole::Grid).base_color),
                            SliderThumb,
                            TuningThumb(index),
                        ));
                    });

                    row.spawn((
                        Node {
                            width: Val::Px(VALUE_WIDTH),
                            ..default()
                        },
                        Text::new(""),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(schematic(SchematicRole::Grid).base_color),
                        TuningValueLabel(index),
                    ));
                });
            }

            list.spawn((
                Node {
                    width: Val::Percent(100.0),
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                },
                Text::new(""),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(schematic(SchematicRole::Selected).base_color),
                TuningConsequence,
            ));
        });
}

/// Where the thumb sits, in pixels from the track's left edge.
///
/// Travel is the track minus the thumb, matching what the core slider measures
/// when it converts a drag into a value. Using the full track width here would
/// put the thumb ahead of the pointer everywhere except the extremes.
#[must_use]
pub fn thumb_offset(value: f64, min: f64, max: f64) -> f32 {
    let span = max - min;
    let norm = if span.abs() < f64::EPSILON {
        0.0
    } else {
        ((value - min) / span).clamp(0.0, 1.0)
    };
    #[allow(clippy::cast_possible_truncation)]
    let offset = norm as f32 * (TRACK_WIDTH - THUMB_WIDTH);
    offset
}

/// The five disjoint pieces of one row, gathered so the sync system stays a
/// system rather than an argument list.
#[derive(bevy::ecs::system::SystemParam)]
#[allow(clippy::type_complexity)]
pub struct TuningRowQueries<'w, 's> {
    root: Query<'w, 's, &'static mut Node, (With<TuningRoot>, Without<TuningThumb>)>,
    sliders: Query<'w, 's, (Entity, &'static TuningSlider, &'static SliderValue)>,
    thumbs: Query<
        'w,
        's,
        (
            &'static TuningThumb,
            &'static mut Node,
            &'static mut BackgroundColor,
        ),
        Without<TuningRoot>,
    >,
    names: Query<
        'w,
        's,
        (&'static TuningNameLabel, &'static mut TextColor),
        Without<TuningValueLabel>,
    >,
    values: Query<
        'w,
        's,
        (
            &'static TuningValueLabel,
            &'static mut Text,
            &'static mut TextColor,
        ),
        Without<TuningNameLabel>,
    >,
    consequence:
        Query<'w, 's, &'static mut Text, (With<TuningConsequence>, Without<TuningValueLabel>)>,
}

/// Push the profile out to the controls. One way, every frame.
pub fn sync_tuning_rows(
    state: Res<StudioState>,
    menu_state: Res<LabMenuState>,
    mut rows: TuningRowQueries,
    mut commands: Commands,
) {
    let showing = state.panel_open && menu_state.tab() == StudioTab::Tuning;
    if let Ok(mut node) = rows.root.single_mut() {
        node.display = if showing {
            Display::Flex
        } else {
            Display::None
        };
    }
    if !showing {
        return;
    }

    let fields: Vec<&TunableField> = tuning_fields().map(|(_, field)| field).collect();
    let selected = menu_state.selected_item;

    for (entity, slider, current) in &rows.sliders {
        let Some(field) = fields.get(slider.0) else {
            continue;
        };
        #[allow(clippy::cast_possible_truncation)]
        let wanted = (field.get)(&state.profile) as f32;
        // `SliderValue` is an immutable component: replace rather than mutate,
        // and only when it actually moved, so a drag is not fighting a write.
        if (current.0 - wanted).abs() > f32::EPSILON {
            commands.entity(entity).insert(SliderValue(wanted));
        }
    }

    // The selected row lights up and every other row stays recessive. The whole
    // row moves together - label, thumb, and value - because a highlight on one
    // part of it is a highlight the eye has to hunt for.
    let role_for = |index: usize| {
        if index == selected {
            SchematicRole::Selected
        } else {
            SchematicRole::Grid
        }
    };

    for (thumb, mut node, mut colour) in &mut rows.thumbs {
        let Some(field) = fields.get(thumb.0) else {
            continue;
        };
        node.left = Val::Px(thumb_offset(
            (field.get)(&state.profile),
            field.min,
            field.max,
        ));
        colour.0 = schematic(role_for(thumb.0)).base_color;
    }

    for (name, mut colour) in &mut rows.names {
        colour.0 = schematic(role_for(name.0)).base_color;
    }

    for (value, mut text, mut colour) in &mut rows.values {
        if let Some(field) = fields.get(value.0) {
            **text = format!("{:.2}", (field.get)(&state.profile));
            colour.0 = schematic(role_for(value.0)).base_color;
        }
    }

    if let Ok(mut text) = rows.consequence.single_mut() {
        **text = fields
            .get(selected)
            .map(|field| field.consequence.to_string())
            .unwrap_or_default();
    }
}

/// A drag reported a new value: write it into the profile and re-solve.
///
/// The same clamp and the same `touch_profile` debounce as the keyboard path,
/// so dragging and arrow keys cannot diverge in what they produce.
pub fn apply_slider_change(
    change: On<ValueChange<f32>>,
    sliders: Query<&TuningSlider>,
    time: Res<Time>,
    mut state: ResMut<StudioState>,
) {
    let Ok(slider) = sliders.get(change.source) else {
        return;
    };
    let Some((_, field)) = tuning_fields().find(|(index, _)| *index == slider.0) else {
        return;
    };
    let value = f64::from(change.value).clamp(field.min, field.max);
    if ((field.get)(&state.profile) - value).abs() < f64::EPSILON {
        return;
    }
    (field.set)(&mut state.profile, value);
    let now = time.elapsed_secs();
    state.touch_profile(now);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The thumb must reach both ends and stop where the core slider's own
    /// travel calculation stops, or dragging drifts from the pointer.
    #[test]
    fn the_thumb_spans_exactly_the_track_minus_its_own_width() {
        assert!((thumb_offset(0.25, 0.25, 4.0) - 0.0).abs() < 0.01);
        assert!((thumb_offset(4.0, 0.25, 4.0) - (TRACK_WIDTH - THUMB_WIDTH)).abs() < 0.01);
        let middle = thumb_offset(2.125, 0.25, 4.0);
        assert!(
            (middle - (TRACK_WIDTH - THUMB_WIDTH) / 2.0).abs() < 0.5,
            "midpoint landed at {middle}"
        );
    }

    /// A degenerate range must not divide by zero and park the thumb at NaN,
    /// which Bevy resolves to a silently unlaid-out node.
    #[test]
    fn a_zero_width_range_is_not_a_division_by_zero() {
        let offset = thumb_offset(1.0, 1.0, 1.0);
        assert!(offset.is_finite(), "offset was {offset}");
    }

    /// Row indices are tab-local because `selected_item` is tab-local. If these
    /// ever counted over all of `TUNABLE_FIELDS`, every row past the first tab
    /// would drive the wrong field - and it would still look plausible.
    #[test]
    fn row_indices_are_tab_local_and_contiguous() {
        let indices: Vec<usize> = tuning_fields().map(|(index, _)| index).collect();
        assert!(!indices.is_empty(), "the Tuning tab has no fields");
        assert_eq!(indices, (0..indices.len()).collect::<Vec<_>>());
        for (index, field) in tuning_fields() {
            assert_eq!(
                field.tab,
                StudioTab::Tuning,
                "row {index} is on another tab"
            );
        }
    }
}
