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
        juice.teleport_shake = (juice.teleport_shake - dt).max(0.0);

        let mut offset = Vec3::ZERO;

        // 1. Landing camera dip
        if juice.land_timer > 0.0 {
            let t = juice.land_timer / 0.25;
            offset.y -= (t * std::f32::consts::PI).sin() * 0.18;
        }

        // 2. Jumping camera lift
        if juice.jump_timer > 0.0 {
            let t = juice.jump_timer / 0.20;
            offset.y += (t * std::f32::consts::PI).sin() * 0.08;
        }

        // 3. Teleport transition shake
        let mut shake_rot = Quat::IDENTITY;
        let elapsed = time.elapsed_secs();
        if juice.teleport_shake > 0.0 {
            let t = juice.teleport_shake;
            offset.x += (elapsed * 55.0).sin() * t * 0.15;
            offset.y += (elapsed * 47.0).cos() * t * 0.15;

            let rot_t = t * 0.02;
            shake_rot = Quat::from_euler(
                EulerRot::YXZ,
                (elapsed * 49.0).cos() * rot_t,
                (elapsed * 61.0).sin() * rot_t,
                0.0,
            );
        }

        // 4. Collapse camera shake
        let game = runtime.live.host_match();
        let collapse_state =
            crate::screens::match_runtime::ambience::collapse_state_for_place(game, tp.place);

        if collapse_state == observed_match::facility::CollapseState::Dying {
            let intensity = 0.04;
            offset.x += (elapsed * 43.0).sin() * intensity * 0.5;
            offset.y += (elapsed * 37.0).cos() * intensity * 0.5;
            offset.z += (elapsed * 51.0).sin() * intensity * 0.5;

            let rot_intensity = 0.003;
            let pitch = (elapsed * 47.0).sin() * rot_intensity;
            let yaw = (elapsed * 39.0).cos() * rot_intensity;
            let roll = (elapsed * 53.0).sin() * rot_intensity;
            shake_rot *= Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);
        }

        let klaxon_active =
            crate::screens::match_runtime::ambience::countdown_klaxon_active(&runtime);
        if klaxon_active {
            let pulse = ((elapsed * std::f32::consts::PI).sin() * 0.5 + 0.5).powf(3.0);
            let intensity = pulse * 0.015;
            offset.y += (elapsed * 23.0).sin() * intensity;
        }

        // Apply offsets in camera space
        let local_offset = transform.rotation * offset;
        transform.translation += local_offset;
        transform.rotation *= shake_rot;
    }
}
