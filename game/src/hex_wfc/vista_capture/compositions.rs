//! Body-height review of production initial district hall compositions.
use super::{Stage, VistaPose, face_dir};
use crate::hex_wfc::sim::HexWfcRuntime;
use bevy::prelude::*;
use observed_authoring::{
    forge::initial_halls::{InitialHallKind, is_terrace_key},
    initial_composition::seed_initial_halls,
};
use observed_hex::{HexFace, hex_origin};

pub(super) fn stand(runtime: &mut HexWfcRuntime, pose: &VistaPose) -> bool {
    let look = Vec3::new(
        pose.yaw.sin() * pose.pitch.cos(),
        pose.pitch.sin(),
        -pose.yaw.cos() * pose.pitch.cos(),
    );
    let at = pose.feet + Vec3::Y * 1.6 + look * 1.5;
    runtime
        .match_state
        .stage_body_facing(runtime.local_player, pose.cell, pose.feet, at)
}

pub(super) fn poses(runtime: &HexWfcRuntime) -> Vec<VistaPose> {
    let state = &runtime.match_state;
    // Plan the same cameras for the authored on/off control. This clone is
    // only photographic planning; the loaded simulation content is unchanged.
    let mut proposal = state.facility.clone();
    proposal.initial_modules.clear();
    let mut profile = state.content().composition().clone();
    profile.initial_hall_compositions = true;
    let wanted = std::env::var("OBSERVED2_REFERENCE_POSES").ok();
    seed_initial_halls(&mut proposal, state.content().cells(), &profile)
        .into_iter()
        .map(|plan| {
            let cell = plan.cells[0];
            let variant = proposal.initial_modules[&cell];
            let terrace = state.content().cells().iter().any(|tile| {
                tile.key.variant == variant
                    && tile.key.register == plan.register
                    && is_terrace_key(&tile.key)
            });
            let entry = HexFace::LATERAL[usize::from(variant % 6)];
            let dir = face_dir(entry);
            let origin = Vec3::from_array(hex_origin(cell));
            let feet = origin + Vec3::new(dir.x * 5.4, 0.5, dir.y * 5.4);
            let target = origin
                + Vec3::Y
                    * if plan.kind == InitialHallKind::Gallery {
                        3.0
                    } else {
                        3.5
                    };
            let ahead = target - feet - Vec3::Y * 1.6;
            let name = match (plan.register, plan.kind) {
                ("infinite_gallery", InitialHallKind::Gallery) => "library_gallery",
                ("infinite_gallery", InitialHallKind::Court) => "library_court",
                ("overlit_grid", InitialHallKind::Gallery) => "lumen_gallery",
                ("overlit_grid", InitialHallKind::Court) => "lumen_court",
                ("shadow_screen", InitialHallKind::Gallery) => "zen_gallery",
                ("shadow_screen", InitialHallKind::Court) => "zen_court",
                ("facet_monument", InitialHallKind::Gallery) if terrace => {
                    "monument_terrace_gallery"
                }
                ("facet_monument", InitialHallKind::Court) if terrace => "monument_terrace_court",
                ("facet_monument", InitialHallKind::Gallery) => "monument_gallery",
                ("facet_monument", InitialHallKind::Court) => "monument_court",
                _ => unreachable!("planner targets authored districts"),
            };
            VistaPose {
                name,
                cell,
                feet,
                yaw: ahead.x.atan2(-ahead.z),
                pitch: ahead.y.atan2(Vec2::new(ahead.x, ahead.z).length()),
                stage: Stage::Nothing,
            }
        })
        .filter(|pose| {
            wanted
                .as_ref()
                .is_none_or(|names| names.split(',').any(|name| name == pose.name))
        })
        .collect()
}
