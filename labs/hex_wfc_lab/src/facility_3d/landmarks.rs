//! Stable room/hall concept index and free-fly vantages for a solved facility.

use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_facility::hex_wfc::{
    HexWfcWorld, StampedBlueprint, blueprint_for_role, placement_tile_archetype,
};
use observed_hex::{HexCoord, HexFace, hex_origin, lateral_distance};

use super::{CameraMode, FacilityState};

const STREAM_RADIUS_CELLS: u32 = 5;
const STREAM_LEVEL_RADIUS: u8 = 1;

#[derive(Clone, Debug)]
pub(super) struct FacilityLandmark {
    pub(super) label: String,
    pub(super) coord: HexCoord,
    eye: Vec3,
    target: Vec3,
}

pub(super) fn build(world: &HexWfcWorld) -> Vec<FacilityLandmark> {
    let mut indexed = BTreeMap::new();
    for blueprint in &world.blueprints {
        let label = format!("ROOM / {}", blueprint.role.label().to_uppercase());
        indexed
            .entry(label.clone())
            .or_insert_with(|| room_landmark(blueprint, label));
    }
    for (&coord, placement) in &world.placements {
        let Some(archetype) = placement_tile_archetype(placement) else {
            continue;
        };
        let label = format!("HALL / {}", archetype.to_uppercase());
        indexed
            .entry(label.clone())
            .or_insert_with(|| hall_landmark(world, coord, label));
    }
    indexed.into_values().collect()
}

fn room_landmark(stamped: &StampedBlueprint, label: String) -> FacilityLandmark {
    let blueprint = blueprint_for_role(stamped.role);
    let centroid = stamped
        .cells
        .iter()
        .map(|&coord| Vec3::from_array(hex_origin(coord)))
        .sum::<Vec3>()
        / stamped.cells.len() as f32;
    let (_, offset, face) = blueprint.named_ports[0];
    let index = blueprint
        .cells
        .iter()
        .position(|&cell| cell == offset)
        .expect("named room port belongs to its footprint");
    let coord = stamped.cells[index];
    let origin = Vec3::from_array(hex_origin(coord));
    let outward = face_direction(face);
    FacilityLandmark {
        label,
        coord,
        eye: origin + outward * 9.0 + Vec3::Y * 2.2,
        target: centroid + Vec3::Y * 2.2,
    }
}

fn hall_landmark(world: &HexWfcWorld, coord: HexCoord, label: String) -> FacilityLandmark {
    let placement = world.placements[&coord];
    let face = HexFace::LATERAL
        .into_iter()
        .find(|&face| placement.is_open(face))
        .unwrap_or(HexFace::East);
    let origin = Vec3::from_array(hex_origin(coord));
    FacilityLandmark {
        label,
        coord,
        eye: origin + Vec3::Y * 2.2,
        target: origin + face_direction(face) * 9.0 + Vec3::Y * 2.2,
    }
}

pub(super) fn cycle(world: &HexWfcWorld, state: &mut FacilityState, forward: bool) {
    if state.landmarks.is_empty() {
        return;
    }
    let next = match state.landmark_index {
        Some(index) if forward => (index + 1) % state.landmarks.len(),
        Some(index) => (index + state.landmarks.len() - 1) % state.landmarks.len(),
        None if forward => 0,
        None => state.landmarks.len() - 1,
    };
    let landmark = state.landmarks[next].clone();
    state.landmark_index = Some(next);
    state.camera_mode = CameraMode::FreeFly;
    state.fly_position = landmark.eye;
    look_at(state, landmark.target);
    update_stream_focus(world, state, landmark.coord);
}

pub(super) fn overview(world: &HexWfcWorld, state: &mut FacilityState) {
    let (min, max) = world.placements.keys().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(min, max), &coord| {
            let origin = Vec3::from_array(hex_origin(coord));
            (min.min(origin), max.max(origin))
        },
    );
    let center = (min + max) * 0.5;
    let plan_span = (max.x - min.x).max(max.z - min.z);
    state.camera_mode = CameraMode::FreeFly;
    state.fly_position = center + Vec3::new(0.0, plan_span * 0.78 + 32.0, plan_span * 0.7);
    look_at(state, center);
    state.landmark_index = None;
    if let Some(coord) = nearest_coord(world, center) {
        update_stream_focus(world, state, coord);
    }
}

fn look_at(state: &mut FacilityState, target: Vec3) {
    let direction = (target - state.fly_position).normalize_or_zero();
    state.fly_yaw = direction.x.atan2(-direction.z);
    state.fly_pitch = direction.y.asin();
}

pub(super) fn track_position(world: &HexWfcWorld, state: &mut FacilityState, position: Vec3) {
    let Some(coord) = nearest_coord(world, position) else {
        return;
    };
    if coord.level.abs_diff(state.stream_focus.level) > 0
        || lateral_distance(coord, state.stream_focus) >= 2
    {
        update_stream_focus(world, state, coord);
    }
}

fn update_stream_focus(world: &HexWfcWorld, state: &mut FacilityState, coord: HexCoord) {
    if world.placements.contains_key(&coord) && state.stream_focus != coord {
        state.stream_focus = coord;
        state.dirty = true;
    }
}

fn nearest_coord(world: &HexWfcWorld, position: Vec3) -> Option<HexCoord> {
    world.placements.keys().copied().min_by(|a, b| {
        Vec3::from_array(hex_origin(*a))
            .distance_squared(position)
            .total_cmp(&Vec3::from_array(hex_origin(*b)).distance_squared(position))
    })
}

pub(super) fn piece_visible(state: &FacilityState, source: HexCoord) -> bool {
    if !is_production(state) {
        return true;
    }
    source.level.abs_diff(state.stream_focus.level) <= STREAM_LEVEL_RADIUS
        && lateral_distance(source, state.stream_focus) <= STREAM_RADIUS_CELLS
}

pub(super) fn current_label(state: &FacilityState) -> &str {
    state
        .landmark_index
        .and_then(|index| state.landmarks.get(index))
        .map_or("free exploration", |landmark| landmark.label.as_str())
}

pub(super) fn rendered_piece_count(state: &FacilityState) -> usize {
    state
        .snapshot
        .pieces
        .iter()
        .filter(|piece| piece_visible(state, piece.source_cell))
        .count()
}

pub(super) fn is_production(state: &FacilityState) -> bool {
    state.source_config.cols >= 28
        && state.source_config.rows >= 20
        && state.source_config.levels >= 10
}

fn face_direction(face: HexFace) -> Vec3 {
    let [a, b] = observed_hex::face_edge(face);
    Vec3::new((a.0 + b.0) as f32 * 0.5, 0.0, (a.1 + b.1) as f32 * 0.5).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LabGenerationMode, generation};

    #[test]
    fn production_landmarks_cover_every_played_room_and_hall_concept() {
        let state = generation::build(0xA11C_E3D0_0000_0008, LabGenerationMode::ProductionCorpus);
        let labels = build(&state.world)
            .into_iter()
            .map(|landmark| landmark.label)
            .collect::<Vec<_>>();
        assert_eq!(
            labels
                .iter()
                .filter(|label| label.starts_with("ROOM / "))
                .count(),
            10
        );
        assert_eq!(
            labels
                .iter()
                .filter(|label| label.starts_with("HALL / "))
                .count(),
            8
        );
    }
}
