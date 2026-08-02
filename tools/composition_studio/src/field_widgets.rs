//! Real slider controls for every tab that declares tunables.
//!
//! The Tuning tab used to draw its eighteen controls as `[----O-------]` text. That is
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
//! reports a change and never writes its own `SliderValue`. `sync_field_rows`
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

/// Which control a row drives: a tab, and an index **within that tab**.
///
/// The pair is carried together because neither half identifies a field on its
/// own - index 0 exists on every tab - and splitting them across components
/// would let a row drift onto the wrong tab's field without ever failing to
/// compile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldRef {
    pub tab: StudioTab,
    pub index: usize,
}

impl FieldRef {
    /// The field this points at, or `None` if the table no longer has one.
    #[must_use]
    pub fn field(self) -> Option<&'static TunableField> {
        fields_on(self.tab)
            .find(|(index, _)| *index == self.index)
            .map(|(_, field)| field)
    }
}

/// Container for one tab's rows; shown only while that tab is up.
#[derive(Component)]
pub struct FieldRows(pub StudioTab);

/// The draggable track for one field.
#[derive(Component)]
pub struct FieldSlider(pub FieldRef);

/// The moving part of row `.0`, positioned by [`sync_field_rows`].
#[derive(Component)]
pub struct FieldThumb(pub FieldRef);

#[derive(Component)]
pub struct FieldName(pub FieldRef);

#[derive(Component)]
pub struct FieldValue(pub FieldRef);

/// One line of copy for the selected row, so the panel explains the control the
/// author is actually holding rather than all eighteen at once.
#[derive(Component)]
pub struct FieldConsequence(pub StudioTab);

/// The fields declared on `tab`, paired with their **tab-local** index.
///
/// Tab-local because `LabMenuState::selected_item` is tab-local. Counting over
/// all of [`TUNABLE_FIELDS`] instead would drive the wrong field on every tab
/// past the first, and would still look entirely plausible.
pub fn fields_on(tab: StudioTab) -> impl Iterator<Item = (usize, &'static TunableField)> {
    TUNABLE_FIELDS
        .iter()
        .filter(move |field| field.tab == tab)
        .enumerate()
}

/// The tabs that declare at least one tunable, in display order.
pub fn tabs_with_fields() -> impl Iterator<Item = StudioTab> {
    StudioTab::ALL
        .into_iter()
        .filter(|tab| fields_on(*tab).next().is_some())
}

/// Spawn one row group per tab that declares tunables. Called from
/// `setup_chrome` inside the docked panel, as siblings *after* the panel text
/// node: on those tabs the text holds only the header, and these rows are the
/// body.
pub fn spawn_field_rows(panel: &mut ChildSpawnerCommands) {
    for tab in tabs_with_fields() {
        panel
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    // Every tab's rows exist from startup and are shown by
                    // display. Spawning them on tab change would rebuild
                    // eighteen sliders per keystroke and drop any drag in
                    // progress.
                    display: Display::None,
                    ..default()
                },
                FieldRows(tab),
            ))
            .with_children(|list| {
                for (index, field) in fields_on(tab) {
                    spawn_row(list, FieldRef { tab, index }, field);
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
                    FieldConsequence(tab),
                ));
            });
    }
}

/// One label / track / value row.
fn spawn_row(list: &mut ChildSpawnerCommands, at: FieldRef, field: &'static TunableField) {
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
            FieldName(at),
        ));

        row.spawn((
            Node {
                width: Val::Px(TRACK_WIDTH),
                height: Val::Px(ROW_HEIGHT - 4.0),
                ..default()
            },
            // Track and thumb draw only from `Grid` and `Selected`. `Pinned`
            // and `Volatile` are spoken for: green and red mean "the solver
            // will not / will rewire this" in the viewport legend, and a red
            // slider track would be making a claim about the facility that it
            // is not making.
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
            FieldSlider(at),
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
                FieldThumb(at),
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
            FieldValue(at),
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
pub struct FieldRowQueries<'w, 's> {
    roots: Query<'w, 's, (&'static FieldRows, &'static mut Node), Without<FieldThumb>>,
    sliders: Query<'w, 's, (Entity, &'static FieldSlider, &'static SliderValue)>,
    thumbs: Query<
        'w,
        's,
        (
            &'static FieldThumb,
            &'static mut Node,
            &'static mut BackgroundColor,
        ),
        Without<FieldRows>,
    >,
    names: Query<'w, 's, (&'static FieldName, &'static mut TextColor), Without<FieldValue>>,
    values: Query<
        'w,
        's,
        (
            &'static FieldValue,
            &'static mut Text,
            &'static mut TextColor,
        ),
        Without<FieldName>,
    >,
    consequence: Query<
        'w,
        's,
        (&'static FieldConsequence, &'static mut Text),
        (Without<FieldValue>, Without<FieldName>),
    >,
}

/// Push the profile out to the controls. One way, every frame.
pub fn sync_field_rows(
    state: Res<StudioState>,
    menu_state: Res<LabMenuState>,
    mut rows: FieldRowQueries,
    mut commands: Commands,
) {
    let active = menu_state.tab();
    let showing = state.panel_open;
    for (group, mut node) in &mut rows.roots {
        node.display = if showing && group.0 == active {
            Display::Flex
        } else {
            Display::None
        };
    }
    if !showing {
        return;
    }

    let selected = menu_state.selected_item;
    // The selected row lights up and every other row stays recessive. The whole
    // row moves together - label, thumb, and value - because a highlight on one
    // part of it is a highlight the eye has to hunt for. Rows on inactive tabs
    // are never selected, so a hidden tab cannot hold the highlight.
    let role_for = |at: FieldRef| {
        if at.tab == active && at.index == selected {
            SchematicRole::Selected
        } else {
            SchematicRole::Grid
        }
    };

    for (entity, slider, current) in &rows.sliders {
        let Some(field) = slider.0.field() else {
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

    for (thumb, mut node, mut colour) in &mut rows.thumbs {
        let Some(field) = thumb.0.field() else {
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
        if let Some(field) = value.0.field() {
            **text = format!("{:.2}", (field.get)(&state.profile));
            colour.0 = schematic(role_for(value.0)).base_color;
        }
    }

    for (group, mut text) in &mut rows.consequence {
        **text = if group.0 == active {
            FieldRef {
                tab: active,
                index: selected,
            }
            .field()
            .map(|field| field.consequence.to_string())
            .unwrap_or_default()
        } else {
            String::new()
        };
    }
}

/// A drag reported a new value: write it into the profile and re-solve.
///
/// The same clamp and the same `touch_profile` debounce as the keyboard path,
/// so dragging and arrow keys cannot diverge in what they produce.
pub fn apply_slider_change(
    change: On<ValueChange<f32>>,
    sliders: Query<&FieldSlider>,
    time: Res<Time>,
    mut state: ResMut<StudioState>,
) {
    let Ok(slider) = sliders.get(change.source) else {
        return;
    };
    let Some(field) = slider.0.field() else {
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
        let indices: Vec<usize> = fields_on(StudioTab::Tuning)
            .map(|(index, _)| index)
            .collect();
        assert!(!indices.is_empty(), "the Tuning tab has no fields");
        assert_eq!(indices, (0..indices.len()).collect::<Vec<_>>());
        for (index, field) in fields_on(StudioTab::Tuning) {
            assert_eq!(
                field.tab,
                StudioTab::Tuning,
                "row {index} is on another tab"
            );
        }
    }
}
