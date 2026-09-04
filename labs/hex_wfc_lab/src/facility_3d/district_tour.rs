//! A flythrough that visits every district in a solved facility, ending at the
//! exit.
//!
//! Enabled with `OBSERVED2_HEX_DISTRICT_TOUR=<dir>`.
//!
//! # Why a tour rather than a walk
//!
//! The bot-POV capture follows the objective bot, which is the right instrument
//! for "is the route walkable" and the wrong one for "do the districts look like
//! different places". A bot goes where the objective is; it may cross four
//! registers and never enter the other six, and nothing in the resulting frames
//! tells you which register you were in when.
//!
//! This flies the camera to one cell of *every* register the solve actually
//! used, in a sensible order, and finishes at the exit. Each frame is named for
//! the district it was taken in, so the output is a labelled set you can lay
//! side by side — which is the only way to answer the question by eye.
//!
//! # The representative cell is a medoid, not a centroid
//!
//! A district is a scattered set of cells, and its average position is often not
//! one of them — frequently it lands in a neighbouring district. The cell with
//! the smallest total distance to the rest of its own district is guaranteed to
//! *be* in the district, and sits well inside it rather than on a border, which
//! is what you want when the question is what this place looks like.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexWfcConfig, HexWfcWorld};
use observed_hex::{HexCoord, hex_origin, travel_distance};
use observed_match::hex_wfc::{HexMatchConfig, HexWfcMatch};

use super::{CameraMode, FacilityCamera, FacilityState, FacilityStatus, LabViewMode};

/// The same pinned seed the bot-POV capture uses, so the two are comparable.
const TOUR_SEED: u64 = 0xa11c_0000_0000_0000;
/// Frames spent travelling between two neighbouring cells.
const FRAMES_PER_STEP: u32 = 7;
/// Frames held on arrival at a district, panning, before moving on.
///
/// The pan is deliberately less than a full turn. Sweeping a whole circle over
/// a short hold spins fast enough to be unreadable, and the hold exists to let
/// somebody *look* at the district rather than to prove the camera can rotate.
const HOLD_FRAMES: u32 = 22;
/// Camera height above the cell floor. Eye level: the question is what a player
/// sees, not what a map sees.
const EYE: f32 = 2.1;
/// Hard cap so an unexpected path still terminates.
const MAX_FRAMES: u32 = 1200;

pub(crate) fn tour_config() -> HexWfcConfig {
    HexWfcConfig {
        cols: 12,
        rows: 9,
        levels: 5,
        min_rooms: 4,
        max_rooms: 8,
        retry_budget: 100,
        min_room_distance: 2,
    }
}

/// One leg of the tour: the cells to fly through, and what waits at the end.
struct Leg {
    cells: Vec<HexCoord>,
    arrival: Option<ArchitectureRegister>,
}

#[derive(Resource)]
pub(crate) struct DistrictTour {
    dir: String,
    legs: Vec<Leg>,
    leg: usize,
    step: usize,
    frame_in_step: u32,
    hold: u32,
    shot: u32,
    started: bool,
    visited: Vec<String>,
}

/// The cell of `register` with the smallest total travel distance to the rest of
/// its own district.
fn medoid(world: &HexWfcWorld, register: ArchitectureRegister) -> Option<HexCoord> {
    let cells: Vec<HexCoord> = world
        .architecture
        .iter()
        .filter(|(_, r)| **r == register)
        .map(|(coord, _)| *coord)
        .collect();
    cells.iter().copied().min_by_key(|candidate| {
        cells
            .iter()
            .map(|other| u64::from(travel_distance(*candidate, *other)))
            .sum::<u64>()
    })
}

/// A lattice walk from `from` to `to`, one cell per entry.
///
/// Not a traversable route — it steps through the lattice rather than through
/// doorways, because the camera is not a body and the tour is about looking
/// rather than reaching. Following cells rather than a straight line keeps the
/// flight mostly inside the facility's own volume instead of cutting through
/// its mass.
fn walk(from: HexCoord, to: HexCoord) -> Vec<HexCoord> {
    let mut cells = Vec::new();
    let (mut q, mut r, mut level) = (i32::from(from.q), i32::from(from.r), i32::from(from.level));
    let (tq, tr, tl) = (i32::from(to.q), i32::from(to.r), i32::from(to.level));
    let mut guard = 0;
    while (q, r, level) != (tq, tr, tl) && guard < 512 {
        guard += 1;
        if level != tl {
            level += (tl - level).signum();
        } else if q != tq {
            q += (tq - q).signum();
        } else {
            r += (tr - r).signum();
        }
        cells.push(HexCoord {
            q: u16::try_from(q).unwrap_or(0),
            r: u16::try_from(r).unwrap_or(0),
            level: u8::try_from(level).unwrap_or(0),
        });
    }
    cells
}

pub(crate) fn setup(mut commands: Commands, mut facility: ResMut<FacilityState>) {
    let Ok(dir) = std::env::var("OBSERVED2_HEX_DISTRICT_TOUR") else {
        return;
    };
    let config = tour_config();
    let game = HexWfcMatch::new_with_content(
        TOUR_SEED,
        HexMatchConfig {
            guardian: false,
            teams: 1,
            members_per_team: 1,
            wfc: config,
        },
        std::sync::Arc::clone(&facility.content),
    )
    .expect("district tour match must build");

    // Every register the solve actually used, with a cell well inside each.
    let mut stops: Vec<(ArchitectureRegister, HexCoord)> = ArchitectureRegister::ALL
        .iter()
        .filter_map(|register| medoid(&game.facility, *register).map(|cell| (*register, cell)))
        .collect();

    // Nearest-neighbour order from the spawn, so the tour does not criss-cross
    // the facility. Greedy is enough: this is a camera path, not a salesman.
    let mut here = config.spawn();
    let mut ordered: Vec<(ArchitectureRegister, HexCoord)> = Vec::new();
    while !stops.is_empty() {
        let (index, _) = stops
            .iter()
            .enumerate()
            .min_by_key(|(_, (_, cell))| travel_distance(here, *cell))
            .expect("stops is non-empty");
        let stop = stops.remove(index);
        here = stop.1;
        ordered.push(stop);
    }

    let mut legs = Vec::new();
    let mut from = config.spawn();
    for (register, cell) in ordered {
        legs.push(Leg {
            cells: walk(from, cell),
            arrival: Some(register),
        });
        from = cell;
    }
    legs.push(Leg {
        cells: walk(from, config.exit()),
        arrival: None,
    });

    facility.rebuild(&game.facility);
    facility.camera_mode = CameraMode::FreeFly;
    facility.overlay = false;
    commands.insert_resource(DistrictTour {
        dir,
        legs,
        leg: 0,
        step: 0,
        frame_in_step: 0,
        hold: 0,
        shot: 0,
        started: false,
        visited: Vec::new(),
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    tour: Option<ResMut<DistrictTour>>,
    mut facility: ResMut<FacilityState>,
    mut mode: ResMut<LabViewMode>,
    mut plan_cameras: Query<&mut Camera, (With<Camera2d>, Without<FacilityCamera>)>,
    mut facility_cameras: Query<&mut Camera, With<FacilityCamera>>,
    mut status: Query<&mut Visibility, With<FacilityStatus>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut tour) = tour else {
        return;
    };
    if !tour.started {
        tour.started = true;
        std::fs::create_dir_all(&tour.dir).expect("district tour dir must be creatable");
        *mode = LabViewMode::Facility3d;
        for mut camera in &mut plan_cameras {
            camera.is_active = false;
        }
        for mut camera in &mut facility_cameras {
            camera.is_active = true;
        }
        for mut visibility in &mut status {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    if tour.leg >= tour.legs.len() || tour.shot >= MAX_FRAMES {
        write_manifest(&tour);
        exit.write(AppExit::Success);
        return;
    }

    // Where the camera is now, and what it is looking at.
    let (position, look_at, label) = {
        let leg = &tour.legs[tour.leg];
        let cells = &leg.cells;
        if cells.is_empty() {
            (facility.fly_position, facility.fly_position, None)
        } else {
            let index = tour.step.min(cells.len() - 1);
            let here = Vec3::from_array(hex_origin(cells[index])) + Vec3::Y * EYE;
            let next = cells
                .get(index + 1)
                .map(|cell| Vec3::from_array(hex_origin(*cell)) + Vec3::Y * EYE);
            #[allow(clippy::cast_precision_loss)]
            let t = tour.frame_in_step as f32 / FRAMES_PER_STEP as f32;
            let position = next.map_or(here, |next| here.lerp(next, t));
            let ahead = next.unwrap_or(here + Vec3::X);
            (position, ahead, leg.arrival)
        }
    };

    facility.fly_position = position;
    let delta = look_at - position;
    if delta.length_squared() > 0.001 {
        facility.fly_yaw = delta.x.atan2(-delta.z);
        facility.fly_pitch = -0.08;
    }
    // On the hold, sweep the view so the district is seen rather than passed.
    if tour.hold > 0 {
        #[allow(clippy::cast_precision_loss)]
        let sweep = (tour.hold as f32 / HOLD_FRAMES as f32) * std::f32::consts::TAU * 0.5;
        facility.fly_yaw += sweep;
    }

    let name = label.map_or_else(
        || "exit".to_string(),
        |register| register.slug().to_string(),
    );
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!(
            "{}/tour_{:03}_{}.png",
            tour.dir, tour.shot, name
        )));
    tour.shot += 1;

    // Advance: hold at the end of a leg, then start the next one.
    let at_end = {
        let leg = &tour.legs[tour.leg];
        leg.cells.is_empty() || tour.step + 1 >= leg.cells.len()
    };
    if at_end {
        if tour.hold == 0 {
            let entry = name.clone();
            if !tour.visited.contains(&entry) {
                tour.visited.push(entry);
            }
        }
        tour.hold += 1;
        if tour.hold > HOLD_FRAMES {
            tour.hold = 0;
            tour.leg += 1;
            tour.step = 0;
            tour.frame_in_step = 0;
        }
        return;
    }
    tour.frame_in_step += 1;
    if tour.frame_in_step >= FRAMES_PER_STEP {
        tour.frame_in_step = 0;
        tour.step += 1;
    }
}

fn write_manifest(tour: &DistrictTour) {
    let manifest = serde_json::json!({
        "lab": "hex_wfc_lab",
        "capture": "district_tour",
        "seed": format!("{:#018x}", TOUR_SEED),
        "frames": tour.shot,
        "districts_visited": tour.visited,
    });
    let path = std::path::Path::new(&tour.dir).join("manifest.json");
    let _ = std::fs::write(
        path,
        serde_json::to_string_pretty(&manifest).unwrap_or_default(),
    );
}
