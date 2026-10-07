//! Temporal filtering stabilizes subpixel ceiling construction. Cuts and
//! geometry changes discard history instead of dragging old rooms into view.
use crate::{hex_wfc::sim::HexWfcRuntime, view::components::GameCam};
use bevy::{anti_alias::taa::TemporalAntiAliasing, prelude::*};

pub(in crate::hex_wfc) fn stabilize_surfaces(
    runtime: Res<HexWfcRuntime>,
    mut previous: Local<Option<(u32, Transform)>>,
    mut camera: Query<(&Transform, &mut TemporalAntiAliasing), With<GameCam>>,
) {
    let Ok((pose, mut temporal)) = camera.single_mut() else {
        return;
    };
    let generation = runtime.match_state.geometry.generation;
    if previous.as_ref().is_none_or(|(old, before)| {
        *old != generation
            || before.translation.distance(pose.translation) > 4.0
            || before.rotation.angle_between(pose.rotation) > 1.0
    }) {
        temporal.reset = true;
    }
    *previous = Some((generation, *pose));
}
