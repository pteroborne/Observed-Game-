//! The first-person place renderer: turn the current place (room or hallway) into
//! neon-noir geometry in its own local frame — floor/ceiling, walls straight from the
//! collision arena, neon doorway frames + animated leaves, a preview of the place beyond
//! each open doorway, and keystone items — plus the shared camera and the walking rival
//! avatars. Rebuilds only when the player teleports.

pub(crate) mod animate;
pub(crate) mod factory;
pub(crate) mod item_visuals;
pub(crate) mod lighting;
pub(crate) mod mesh;
pub(crate) mod monitors;
pub(crate) mod preview;
pub(crate) mod shell;

use bevy::prelude::*;

use crate::camera;
use crate::sim::state::TeleportState;
use crate::view::components::GameCam;

pub(crate) use animate::{
    animate_doors, animate_teleport_pad_glow, sync_rival_avatars, update_carried_torch_light,
};
pub(crate) use factory::{LastRenderedSignature, rebuild_place};
pub(crate) use monitors::{
    GuardianConsole, GuardianObservationMonitor, ObservationMonitorKind,
    ObservationMonitorLabelSegment, TetherCameraMonitor, interact_guardian_console, monitor_label,
    update_guardian_monitors, update_tether_monitors,
};
// Test-only: `game::tests` reaches these through `crate::screens::place::...` rather than
// `crate::screens::place::monitors::...` to match the rest of that module's style; nothing
// in the production build needs them re-exported at this level.
#[cfg(test)]
pub(crate) use monitors::{MONITOR_PANEL_ENTITY_BUDGET, MonitorMiniature, monitor_page_for};

/// Place the shared camera at the player's first-person eye in the current place's
/// local frame (each place is centred at the origin). No shake — a reroute is felt
/// through the world's flickering lights ([`super::flicker_lights`]), not the camera.
pub(crate) fn present_match_camera(
    tp: Res<TeleportState>,
    mut camera: Query<&mut Transform, With<GameCam>>,
) {
    if let Ok(mut transform) = camera.single_mut() {
        camera::player_view(&tp.body, &tp.config).apply_to(&mut transform);
    }
}
