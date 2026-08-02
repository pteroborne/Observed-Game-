//! Tests for the Tuning tab's real slider controls.
//!
//! Split from `tests.rs` to keep both under the 600-line review budget.

use bevy::prelude::*;

use crate::StudioState;
use crate::tests::headless;

/// Dragging a slider must move the profile, not just the widget.
///
/// This drives the real seam a mouse drag uses - `ValueChange` triggered on the
/// slider entity - rather than calling the setter directly, so it fails if the
/// observer is never registered, if the row index stops matching the field, or
/// if the sliders are spawned without their marker. Every one of those leaves a
/// tool that looks correct and quietly edits nothing.
#[test]
fn dragging_a_slider_edits_the_profile_it_points_at() {
    use crate::tuning_widgets::{TuningSlider, tuning_fields};

    let mut app = headless();
    app.update();

    let (entity, index) = app
        .world_mut()
        .query::<(Entity, &TuningSlider)>()
        .iter(app.world())
        .map(|(entity, slider)| (entity, slider.0))
        .next()
        .expect("the Tuning tab spawned no sliders");

    let field = tuning_fields()
        .find(|(row, _)| *row == index)
        .map(|(_, field)| field)
        .expect("slider row has no matching field");

    let before = (field.get)(&app.world().resource::<StudioState>().profile);
    let wanted = if before > field.min + 0.5 {
        field.min
    } else {
        field.max
    };

    #[allow(clippy::cast_possible_truncation)]
    app.world_mut().trigger(bevy::ui_widgets::ValueChange {
        source: entity,
        value: wanted as f32,
    });
    app.update();

    let after = (field.get)(&app.world().resource::<StudioState>().profile);
    assert!(
        (after - wanted).abs() < 0.01,
        "{} went to {after}, wanted {wanted}",
        field.label
    );
    assert!(
        app.world().resource::<StudioState>().solve_dirty
            || app.world().resource::<StudioState>().last_edit.is_some(),
        "an edit that never re-solves leaves the viewport lying about the profile"
    );
}

/// The widget must never be able to author a value the validator would reject.
#[test]
fn a_slider_cannot_push_a_field_past_its_own_range() {
    use crate::tuning_widgets::{TuningSlider, tuning_fields};

    let mut app = headless();
    app.update();

    let (entity, index) = app
        .world_mut()
        .query::<(Entity, &TuningSlider)>()
        .iter(app.world())
        .map(|(entity, slider)| (entity, slider.0))
        .next()
        .expect("the Tuning tab spawned no sliders");
    let field = tuning_fields()
        .find(|(row, _)| *row == index)
        .map(|(_, field)| field)
        .expect("slider row has no matching field");

    app.world_mut().trigger(bevy::ui_widgets::ValueChange {
        source: entity,
        value: 9_000.0,
    });
    app.update();

    let after = (field.get)(&app.world().resource::<StudioState>().profile);
    assert!(
        after <= field.max,
        "{} reached {after}, above its own max {}",
        field.label,
        field.max
    );
}
