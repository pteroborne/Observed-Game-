//! Supported, observation-validated major inspection fixtures.
use super::{Stage, VistaPose};
use crate::hex_wfc::sim::HexWfcRuntime;
use bevy::prelude::*;

pub(super) fn prepare(runtime: &HexWfcRuntime, found: &mut Vec<VistaPose>) {
    if std::env::var_os("OBSERVED2_CAPTURE_HEX_WFC_GUARDIAN").is_some()
        && let Some((&cell, spine)) = runtime.match_state.geometry.climbs.iter().find(|(_, s)| {
            s.nodes.len() > 1 && (s.nodes[1] - s.nodes[0]).with_y(0.0).length() > 6.0
        })
    {
        let start = spine.nodes[0];
        let offset = spine.nodes[1] - start;
        let distance = offset.with_y(0.0).length();
        let direction = offset.with_y(0.0).normalize();
        let metres = 5_u8;
        let rise = offset.y * f32::from(metres) / distance;
        found.insert(
            0,
            VistaPose {
                name: "guardian_ramp",
                cell,
                feet: start,
                yaw: direction.x.atan2(-direction.z),
                pitch: (rise + 0.6).atan2(f32::from(metres)),
                stage: Stage::Guardian {
                    metres,
                    rise_cm: (rise * 100.0).round() as i16,
                    at_mm: None,
                },
            },
        );
    }
}

pub(super) fn prepare_pose(runtime: &HexWfcRuntime, pose: &mut VistaPose) -> bool {
    let Stage::Guardian {
        metres,
        rise_cm,
        at_mm,
    } = &mut pose.stage
    else {
        return true;
    };
    let ahead = Vec3::new(pose.yaw.sin(), 0.0, -pose.yaw.cos());
    let preferred =
        pose.feet + ahead * f32::from(*metres) + Vec3::Y * (f32::from(*rise_cm) / 100.0);
    let game = &runtime.match_state;
    let Some(standing) = game.major_standing_point(pose.cell, preferred) else {
        return false;
    };
    let mut major = observed_match::hex_wfc::HexGuardianState::at(pose.cell);
    major.position = standing + Vec3::Y * 0.9;
    let view = std::iter::once(pose.feet)
        .chain(game.standing_points(pose.cell))
        .filter_map(|feet| {
            let distance = feet.distance(standing);
            if !(3.0..=9.0).contains(&distance) {
                return None;
            }
            let sight = standing + Vec3::Y * 2.2 - (feet + Vec3::Y * 1.6);
            let mut viewer = runtime.local().clone();
            viewer.cell = pose.cell;
            viewer.position = feet + Vec3::Y * 0.9;
            viewer.yaw = sight.x.atan2(-sight.z);
            viewer.pitch = sight.y.atan2(sight.with_y(0.0).length());
            game.major_visible_from(&viewer, &major)
                .then_some((feet, viewer))
        })
        .min_by(|a, b| {
            a.0.distance_squared(pose.feet)
                .total_cmp(&b.0.distance_squared(pose.feet))
        });
    let Some((feet, viewer)) = view else {
        return false;
    };
    *at_mm = Some(standing.to_array().map(|v| (v * 1000.0).round() as i32));
    pose.feet = feet;
    pose.yaw = viewer.yaw;
    pose.pitch = viewer.pitch;
    true
}

pub(super) fn validate(runtime: &HexWfcRuntime, pose: &VistaPose, path: &str) -> bool {
    let major = &runtime.match_state.guardian;
    let diagnostic = serde_json::json!({
        "seed":runtime.match_state.seed,"tick":runtime.match_state.tick,
        "input_version":observed_match::hex_wfc::HEX_INPUT_VERSION,
        "staged_fixture":pose.name,"rules":"facility_race_shared_major_physics",
        "guardian_cell":format!("{:?}",major.cell),"guardian_feet":major.feet().to_array(),
        "guardian_status":format!("{:?}",major.status),"physically_placed":major.physically_placed(),
        "visible_to_viewer": runtime.match_state.major_visible_from(runtime.local(),major),
        "viewer_position":runtime.local().position.to_array(),"viewer_yaw":runtime.local().yaw,"viewer_pitch":runtime.local().pitch,
    });
    std::fs::write(
        std::path::Path::new(path).join(format!("{}.json", pose.name)),
        serde_json::to_string_pretty(&diagnostic).expect("finite guardian diagnostics"),
    )
    .expect("capture diagnostics writable");
    major.physically_placed()
        && runtime
            .match_state
            .major_visible_from(runtime.local(), major)
        && major.status == observed_match::hex_wfc::HexGuardianStatus::FrozenByPlayer
}
