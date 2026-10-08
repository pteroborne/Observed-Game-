//! Body-height review of the production initial Library/Lumen hall compositions.
use super::{Stage, VistaPose, face_dir};
use crate::hex_wfc::sim::HexWfcRuntime;
use bevy::prelude::*;
use observed_authoring::{
    forge::initial_halls::InitialHallKind, initial_composition::seed_initial_halls,
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
    seed_initial_halls(&mut proposal, state.content().cells(), &profile)
        .into_iter()
        .map(|plan| {
            let cell = plan.cells[0];
            let variant = proposal.initial_modules[&cell];
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
                _ => unreachable!("planner targets two districts"),
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
        .collect()
}
