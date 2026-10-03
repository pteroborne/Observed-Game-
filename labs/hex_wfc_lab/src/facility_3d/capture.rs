//! Curated Phase 92 evidence: hall, a climb from its foot and from its landing, and
//! retained 2D mode.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_facility::hex_wfc::{HexArchetype, HexWfcConfig, HexWfcWorld};
use observed_hex::hex_origin;

use super::{
    CameraMode, FacilityCamera, FacilityState, FacilityStatus, LabViewMode, climb_view,
    face_plan_dir,
};
use crate::LabState;

const SHOTS: [&str; 5] = [
    "hall_chain.png",
    "climb_up.png",
    "climb_down.png",
    "collider_debug.png",
    "plan_2d.png",
];

#[derive(Resource)]
pub(crate) struct CaptureRun {
    dir: String,
    showcase: HexWfcWorld,
    frame: u32,
    stage: usize,
    armed: bool,
}

impl CaptureRun {
    pub(crate) fn new(dir: String) -> Self {
        Self {
            dir,
            showcase: capture_world(),
            frame: 0,
            stage: 0,
            armed: false,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn capture_progress(
    mut run: ResMut<CaptureRun>,
    mut mode: ResMut<LabViewMode>,
    mut facility: ResMut<FacilityState>,
    mut plan: ResMut<LabState>,
    mut plan_cameras: Query<&mut Camera, (With<Camera2d>, Without<FacilityCamera>)>,
    mut facility_cameras: Query<&mut Camera, With<FacilityCamera>>,
    mut status: Query<&mut Visibility, With<FacilityStatus>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    run.frame += 1;
    if run.frame == 1 {
        std::fs::create_dir_all(&run.dir).expect("capture dir must be creatable");
    }
    if run.stage >= SHOTS.len() {
        if run.frame >= 24 {
            write_manifest(&run.dir, &run.showcase, &plan, &facility);
            exit.write(AppExit::Success);
        }
        return;
    }
    if !run.armed {
        if run.frame < 18 {
            return;
        }
        stage(
            run.stage,
            &run.showcase,
            &mut mode,
            &mut facility,
            &mut plan,
        );
        apply_camera_visibility(*mode, &mut plan_cameras, &mut facility_cameras, &mut status);
        run.armed = true;
        run.frame = 0;
        return;
    }
    // Defer until the camera, visibility, and any 2D rebuild reached a frame.
    if run.frame >= 12 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(format!("{}/{}", run.dir, SHOTS[run.stage])));
        run.stage += 1;
        run.armed = false;
        run.frame = 0;
    }
}

fn stage(
    stage: usize,
    showcase: &HexWfcWorld,
    mode: &mut LabViewMode,
    facility: &mut FacilityState,
    plan: &mut LabState,
) {
    match stage {
        0 => {
            *mode = LabViewMode::Facility3d;
            facility.rebuild(showcase);
            set_collider_view(facility, false);
            let (position, target) = hall_vantage(showcase);
            set_walk_vantage(facility, position, target);
        }
        1 => {
            *mode = LabViewMode::Facility3d;
            set_collider_view(facility, false);
            let (position, target) = climb_up_vantage(showcase);
            set_walk_vantage(facility, position, target);
        }
        2 => {
            *mode = LabViewMode::Facility3d;
            set_collider_view(facility, false);
            let (position, target) = climb_down_vantage(showcase);
            set_vantage(facility, position, target);
        }
        3 => {
            *mode = LabViewMode::Facility3d;
            set_collider_view(facility, true);
            let (position, target) = hall_vantage(showcase);
            set_walk_vantage(facility, position, target);
        }
        4 => {
            *mode = LabViewMode::Plan2d;
            plan.playing = false;
            plan.cursor = plan.trace.len();
            plan.current_level = 0;
            plan.dirty = true;
        }
        _ => unreachable!(),
    }
}

fn set_collider_view(state: &mut FacilityState, enabled: bool) {
    if state.collider_view != enabled {
        state.collider_view = enabled;
        state.dirty = true;
    }
}

fn set_vantage(state: &mut FacilityState, position: Vec3, target: Vec3) {
    let direction = (target - position).normalize_or_zero();
    state.camera_mode = CameraMode::FreeFly;
    state.fly_position = position;
    state.fly_yaw = direction.x.atan2(-direction.z);
    state.fly_pitch = direction.y.asin();
}

fn set_walk_vantage(state: &mut FacilityState, eye: Vec3, target: Vec3) {
    let direction = (target - eye).normalize_or_zero();
    state.camera_mode = CameraMode::Walk;
    state.body.position = eye - Vec3::Y * (state.config.eye_height - state.config.half_height);
    state.body.velocity = Vec3::ZERO;
    state.body.yaw = direction.x.atan2(-direction.z);
    state.body.pitch = direction.y.asin();
    state.body.grounded = false;
}

fn capture_world() -> HexWfcWorld {
    let config = HexWfcConfig {
        cols: 12,
        rows: 9,
        levels: 5,
        min_rooms: 4,
        max_rooms: 8,
        retry_budget: 100,
        min_room_distance: 2,
    };
    (0..64)
        .filter_map(|offset| {
            HexWfcWorld::generate(crate::PRESET_SEEDS[0].wrapping_add(offset), config).ok()
        })
        .find(|world| {
            // Every vantage the showcase shoots. The search once asked for one and
            // let the others `expect` downstream, which held only while a world with
            // the first happened to have the rest.
            try_hall_vantage(world).is_some() && climb_view(world).is_some()
        })
        .expect("Phase 92 evidence search must find a flat hall and a climb")
}

fn hall_vantage(world: &HexWfcWorld) -> (Vec3, Vec3) {
    try_hall_vantage(world).expect("showcase route has no flat hall")
}

/// Two consecutive hall cells on one level, somewhere along the showcase route.
///
/// Fallible so [`capture_world`] can *search* for a world that has one rather
/// than assert that the world it already picked does.
fn try_hall_vantage(world: &HexWfcWorld) -> Option<(Vec3, Vec3)> {
    let route = world.route_between(world.config.spawn(), world.config.exit())?;
    for chain in route.windows(3) {
        let placement = world.placements[&chain[0]];
        let next = world.placements[&chain[1]];
        if matches!(
            placement.archetype,
            HexArchetype::Straight | HexArchetype::Corner | HexArchetype::Junction
        ) && matches!(
            next.archetype,
            HexArchetype::Straight | HexArchetype::Corner | HexArchetype::Junction
        ) && chain[0].level == chain[1].level
        {
            let origin = Vec3::from_array(hex_origin(chain[0]));
            return Some((
                origin + Vec3::Y * 2.2,
                Vec3::from_array(hex_origin(chain[1])) + Vec3::Y * 2.2,
            ));
        }
    }
    None
}

/// At a climb's foot, looking up the flight to where it reaches the next storey.
fn climb_up_vantage(world: &HexWfcWorld) -> (Vec3, Vec3) {
    let climb = climb_view(world).expect("showcase climb");
    let origin = Vec3::from_array(hex_origin(climb.foot));
    let direction = face_plan_dir(climb.heading);
    let along = Vec3::new(direction.x, 0.0, direction.y);
    (
        origin - along * 5.0 + Vec3::Y * 2.2,
        origin + along * 28.0 + Vec3::Y * 8.0,
    )
}

/// On a climb's landing, looking back down the whole flight to its foot.
fn climb_down_vantage(world: &HexWfcWorld) -> (Vec3, Vec3) {
    let climb = climb_view(world).expect("showcase climb");
    let direction = face_plan_dir(climb.heading);
    let along = Vec3::new(direction.x, 0.0, direction.y);
    (
        Vec3::from_array(hex_origin(climb.landing)) + along * 5.5 + Vec3::Y * 2.2,
        Vec3::from_array(hex_origin(climb.foot)) + Vec3::Y * 1.0,
    )
}

fn apply_camera_visibility(
    mode: LabViewMode,
    plan_cameras: &mut Query<&mut Camera, (With<Camera2d>, Without<FacilityCamera>)>,
    facility_cameras: &mut Query<&mut Camera, With<FacilityCamera>>,
    status: &mut Query<&mut Visibility, With<FacilityStatus>>,
) {
    for mut camera in plan_cameras {
        camera.is_active = mode != LabViewMode::Facility3d;
    }
    for mut camera in facility_cameras {
        camera.is_active = mode == LabViewMode::Facility3d;
    }
    for mut visibility in status {
        *visibility = if mode == LabViewMode::Facility3d {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn write_manifest(dir: &str, showcase: &HexWfcWorld, plan: &LabState, facility: &FacilityState) {
    let climb = climb_view(showcase).expect("showcase climb");
    let manifest = serde_json::json!({
        "lab": "hex_wfc_lab",
        "phase": 92,
        "seed": format!("{:#018x}", showcase.seed),
        "grid_3d": [showcase.config.cols, showcase.config.rows, showcase.config.levels],
        "grid_2d": [plan.world.config.cols, plan.world.config.rows, plan.world.config.levels],
        "colliders": facility.snapshot.pieces.len(),
        "room_blueprints": facility.snapshot.blueprint_instances,
        "climb_foot": [climb.foot.q, climb.foot.r, climb.foot.level],
        "climb_landing": [climb.landing.q, climb.landing.r, climb.landing.level],
        "shots": SHOTS,
        "legend": {
            "gold": "room / decision place",
            "blue": "hall / traversal",
            "bright_cyan": "climb composition",
            "dark": "rhombus boundary shell"
        }
    });
    std::fs::write(
        format!("{dir}/manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest serializes"),
    )
    .expect("manifest writes");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_vantages_are_finite_and_look_down_a_whole_climb() {
        let world = capture_world();
        for vantage in [
            hall_vantage(&world),
            climb_up_vantage(&world),
            climb_down_vantage(&world),
        ] {
            assert!(vantage.0.is_finite() && vantage.1.is_finite());
        }
        let (landing, foot) = climb_down_vantage(&world);
        assert!(landing.y - foot.y >= 8.0, "the camera looks down a storey");
        assert!(
            landing.with_y(0.0).distance(foot.with_y(0.0)) >= 28.0,
            "the camera looks down the whole flight"
        );
    }

    #[test]
    fn face_vantage_uses_all_six_directions_without_local_metrics() {
        for face in observed_hex::HexFace::LATERAL {
            assert!(face_plan_dir(face).is_normalized());
        }
    }
}
