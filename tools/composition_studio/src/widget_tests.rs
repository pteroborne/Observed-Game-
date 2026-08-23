//! Tests for the panel's real slider controls.
//!
//! Split from `tests.rs` to keep both under the 600-line review budget.

use bevy::prelude::*;

use crate::StudioState;
use crate::field_widgets::FieldSlider;
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
    let mut app = headless();
    app.update();

    let (entity, index) = app
        .world_mut()
        .query::<(Entity, &FieldSlider)>()
        .iter(app.world())
        .map(|(entity, slider)| (entity, slider.0))
        .next()
        .expect("the Tuning tab spawned no sliders");

    let field = index.field().expect("slider row has no matching field");

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
        // A committed edit, not a live drag: these tests assert the profile
        // moved, which is what a slider release means.
        is_final: true,
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
    let mut app = headless();
    app.update();

    let (entity, index) = app
        .world_mut()
        .query::<(Entity, &FieldSlider)>()
        .iter(app.world())
        .map(|(entity, slider)| (entity, slider.0))
        .next()
        .expect("the Tuning tab spawned no sliders");
    let field = index.field().expect("slider row has no matching field");

    app.world_mut().trigger(bevy::ui_widgets::ValueChange {
        source: entity,
        value: 9_000.0,
        // A committed edit, not a live drag: these tests assert the profile
        // moved, which is what a slider release means.
        is_final: true,
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

/// The corridor routing switch must produce a facility, not a dead viewport.
///
/// It ships a control the moment it appears in `TUNABLE_FIELDS`, and a control
/// that empties the viewport is worse than no control: an author flips it, sees
/// nothing, and has no way to tell a broken switch from a hard seed. The routing
/// stage is off by default precisely because it is still being sized, so this is
/// the guard that keeps the *switch* honest while the feature is unfinished.
#[test]
fn the_corridor_routing_switch_still_solves() {
    use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld};

    let mut profile = observed_facility::hex_wfc::profile::HexCompositionProfile::baseline();
    profile.route_corridors = true;
    assert_eq!(profile.validate(), Ok(()));

    // Several floors, because the router has to claim shafts to connect rooms
    // that are not on the same one, and a single-level facility never exercises
    // that at all.
    for levels in [1u8, 4] {
        let config = HexWfcConfig {
            levels,
            ..HexWfcConfig::default()
        };
        for seed in [1u64, 0x0000_0000_000c_0ffe, 0x5eed_0000_0000_0001] {
            assert!(
                HexWfcWorld::generate_with_profile(seed, config, None, &profile).is_ok(),
                "routing left seed {seed:#x} unsolved at {levels} level(s), so the \
                 switch would show an author an empty viewport"
            );
        }
    }
}
