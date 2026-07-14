//! The first-person place renderer: turn the current place (room or hallway) into
//! neon-noir geometry in its own local frame — floor/ceiling, walls straight from the
//! collision arena, neon doorway frames + animated leaves, a preview of the place beyond
//! each open doorway, and keystone items — plus the shared camera and the walking rival
//! avatars. Rebuilds only when the player teleports.

pub(crate) mod animate;
mod authored;
pub(crate) mod factory;
pub(crate) mod item_visuals;
pub(crate) mod lighting;
pub(crate) mod mesh;
pub(crate) mod modules;
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
    monitor_page_for, observation_bank_view, sync_observation_monitors, update_guardian_monitors,
    update_tether_monitors,
};
pub(crate) use shell::normalize_imported_threshold_materials;
// Test-only: `game::tests` reaches these through `crate::screens::place::...` rather than
// `crate::screens::place::monitors::...` to match the rest of that module's style; nothing
// in the production build needs them re-exported at this level.
#[cfg(test)]
pub(crate) use monitors::{
    MONITOR_PANEL_ENTITY_BUDGET, ObservationFeedElement, ObservationFeedPrimitive,
    ObservationPanel, observation_page_contents, observation_panel_content,
};

use crate::sim::director::MatchDirector;

/// Place the shared camera at the player's first-person eye in the current place's
/// local frame (each place is centred at the origin). No shake — a reroute is felt
/// through the world's flickering lights ([`super::flicker_lights`]), not the camera.
pub(crate) fn present_match_camera(
    time: Res<Time>,
    mut juice: ResMut<crate::view::components::CameraJuice>,
    runtime: Res<MatchDirector>,
    tp: Res<TeleportState>,
    mut camera: Query<&mut Transform, With<GameCam>>,
) {
    if let Ok(mut transform) = camera.single_mut() {
        camera::player_view(&tp.body, &tp.config).apply_to(&mut transform);

        let dt = time.delta_secs();
        juice.land_timer = (juice.land_timer - dt).max(0.0);
        juice.jump_timer = (juice.jump_timer - dt).max(0.0);
        juice.teleport_ease_timer = (juice.teleport_ease_timer - dt).max(0.0);

        let mut offset = Vec3::ZERO;

        if juice.land_timer > 0.0 {
            let t = juice.land_timer / 0.25;
            offset.y -= (t * std::f32::consts::PI).sin() * 0.18;
        }

        if juice.jump_timer > 0.0 {
            let t = juice.jump_timer / 0.20;
            offset.y += (t * std::f32::consts::PI).sin() * 0.08;
        }

        if juice.teleport_ease_timer > 0.0 {
            const TELEPORT_CAMERA_EASE_SECS: f32 = 0.45;
            let t = (juice.teleport_ease_timer / TELEPORT_CAMERA_EASE_SECS).clamp(0.0, 1.0);
            let pulse = (t * std::f32::consts::PI).sin();
            offset.y += pulse * 0.10;
            offset.z -= pulse * 0.04;
        }

        let game = runtime.live.host_match();
        let collapse_state =
            crate::screens::match_runtime::ambience::collapse_state_for_place(game, tp.place);

        if collapse_state == observed_match::facility::CollapseState::Dying {
            offset.y -= 0.025;
        }

        let local_offset = transform.rotation * offset;
        transform.translation += local_offset;
    }
}
