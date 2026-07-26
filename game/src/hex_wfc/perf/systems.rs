//! Per-system CPU timing for the hex state's `Update` set.
//!
//! The set is `.chain()`ed in [`crate::hex_wfc`], so inserting a mark between every
//! adjacent pair is both legal and exact: each mark closes the span opened by the one
//! before it. That yields the per-system breakdown a sampling profiler would give,
//! without one attached, at the cost of one `Instant::now()` per system inside a chain
//! that was already sequential.

use std::time::Instant;

use bevy::camera::visibility::VisibilitySystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

use super::{HexPerfMetrics, current_register, micros};
use crate::GameState;
use crate::hex_wfc::sim::HexWfcRuntime;

/// Time one `route_between_cells` call per frame, at the runner's current position.
///
/// `bot_command` performs this A* **twice per bot per tick** — once in `bot_behaviour` to
/// choose Seek vs Explore, then again for the route itself — with no caching. A* expands a
/// frontier that grows with distance-to-goal, so the per-call cost is a function of where
/// in the facility you stand, which is the shape of the whole problem. Measured here
/// against the live facility but discarded, so the simulation is untouched.
pub(super) fn probe_route_cost(
    runtime: Res<HexWfcRuntime>,
    mut metrics: ResMut<super::HexPerfMetrics>,
) {
    let player = runtime.local();
    let exit = runtime.match_state.facility.config.exit();
    let started = Instant::now();
    let route = runtime
        .match_state
        .facility
        .route_between_cells(player.cell, exit);
    let elapsed = micros(started.elapsed());
    let Some(register) = current_register(&runtime) else {
        return;
    };
    let length = route.map_or(0, |route| route.cells.len()) as u64;
    metrics
        .route_probe_by_register
        .entry(register)
        .or_default()
        .push(elapsed);
    metrics
        .route_length_by_register
        .entry(register)
        .or_default()
        .push(length);
}

/// Register a mark between each adjacent pair of the chained hex `Update` systems.
pub(super) fn configure(app: &mut App) {
    use crate::hex_wfc::{audio, entities, feedback, hud, input, lantern, sim, tacmap, view};
    app.add_systems(
        Update,
        (
            mark("input::map_input").before(input::map_input),
            mark("input::mode_hotkeys").after(input::map_input),
            mark("view::sync_changed_geometry").after(input::mode_hotkeys),
            mark("view::sync_streamed_cells").after(view::sync_changed_geometry),
            mark("view::sync_practical_shadow_budget").after(view::sync_streamed_cells),
            mark("view::sync_camera").after(view::sync_practical_shadow_budget),
            mark("view::sync_lighting_and_atmosphere").after(view::sync_camera),
            mark("hud::sync").after(view::sync_lighting_and_atmosphere),
            mark("tacmap::sync").after(hud::sync),
            mark("feedback::sync").after(tacmap::sync),
            mark("audio::sync").after(feedback::sync),
            mark("entities::sync").after(audio::sync),
            mark("lantern::sync_projection").after(entities::sync),
            mark("lantern::sync_dynamic").after(lantern::sync_projection),
            mark("sim::finish_runtime").after(lantern::sync_dynamic),
            mark("after_hex_update").after(sim::finish_runtime),
        )
            .run_if(in_state(GameState::HexWfc)),
    )
    .add_systems(
        Update,
        probe_route_cost
            .after(sim::finish_runtime)
            .run_if(in_state(GameState::HexWfc)),
    )
    // Split `PostUpdate` around the engine passes that scale with entity count. The
    // `after_hex_update` span lumps together everything from the end of our chain to
    // `Last` — the rest of `Update` plus all of `PostUpdate`. These marks separate the two
    // classic hierarchy-wide costs (transform propagation and visibility) from the
    // remainder, which says whether the residual is engine work over ~109 k entities or
    // our own.
    .add_systems(
        PostUpdate,
        // A mark opens the span that runs *after* it, so each is named for the work it
        // brackets, not for where it sits.
        (
            mark("post_update:transform_propagate")
                .before(TransformSystems::Propagate)
                .before(VisibilitySystems::CalculateBounds),
            mark("post_update:visibility")
                .after(TransformSystems::Propagate)
                .before(VisibilitySystems::VisibilityPropagate),
            mark("post_update:tail").after(VisibilitySystems::MarkNewlyHiddenEntitiesInvisible),
        )
            .run_if(in_state(GameState::HexWfc)),
    );
}

/// Close the previous span and open `label`'s.
fn mark(label: &'static str) -> impl FnMut(Res<HexWfcRuntime>, ResMut<HexPerfMetrics>) + Clone {
    move |runtime: Res<HexWfcRuntime>, mut metrics: ResMut<HexPerfMetrics>| {
        let now = Instant::now();
        close_span(&runtime, &mut metrics, now);
        metrics.phase_mark = Some((label, now));
    }
}

/// Attribute the open span to the register the runner is standing in. Shared with
/// [`super::end_main_schedule`], which closes the last span of the frame.
pub(super) fn close_span(runtime: &HexWfcRuntime, metrics: &mut HexPerfMetrics, now: Instant) {
    if let Some((previous, started)) = metrics.phase_mark.take()
        && let Some(register) = current_register(runtime)
    {
        let elapsed = micros(now.duration_since(started));
        metrics
            .phase_by_register
            .entry((register, previous))
            .or_default()
            .push(elapsed);
    }
}
