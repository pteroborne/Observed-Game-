//! Compact trace and production-corpus world construction for the lab.

use std::collections::BTreeSet;

use observed_facility::hex_wfc::{
    HexRoomQuotas, HexWfcConfig, HexWfcWorld, placement_tile_archetype,
};
use observed_facility::map_spec::RoomRole;

use crate::LabState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LabGenerationMode {
    CompactTrace,
    ProductionCorpus,
}

impl LabGenerationMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::CompactTrace => "compact trace",
            Self::ProductionCorpus => "production corpus",
        }
    }

    pub(crate) fn toggled(self) -> Self {
        match self {
            Self::CompactTrace => Self::ProductionCorpus,
            Self::ProductionCorpus => Self::CompactTrace,
        }
    }
}

const PRODUCTION_TEAM_COUNT: u8 = 2;
const CONCEPT_SEARCH_LIMIT: u64 = 32;

const PLAYED_ROOM_ROLES: [RoomRole; 10] = [
    RoomRole::Start,
    RoomRole::Exit,
    RoomRole::Decision,
    RoomRole::DecoherenceFork,
    RoomRole::AnchorCheckpoint,
    RoomRole::Keystone,
    RoomRole::DualStation,
    RoomRole::GuardianControl,
    RoomRole::Monitor,
    RoomRole::Recovery,
];

const PLAYED_HALL_ARCHETYPES: [&str; 8] = [
    "hall_straight",
    "hall_turn_60",
    "hall_turn_120",
    "hall_junction_3way",
    "hall_junction_4way",
    "hall_ramp",
    "stair_tower",
    "expanse",
];

pub(crate) fn build(seed: u64, mode: LabGenerationMode) -> LabState {
    let (world, trace, status) = match mode {
        LabGenerationMode::CompactTrace => {
            let config = HexWfcConfig {
                cols: 12,
                rows: 9,
                levels: 4,
                min_rooms: 4,
                max_rooms: 8,
                retry_budget: 100,
                min_room_distance: 2,
            };
            let (world, trace) =
                HexWfcWorld::generate_traced(seed, config).expect("default hex config must solve");
            (
                world,
                trace,
                "compact traced solve — Space pauses".to_string(),
            )
        }
        LabGenerationMode::ProductionCorpus => {
            let world = production_corpus(seed);
            let status = format!(
                "production corpus — every played room/hall concept present (requested {seed:#018x})"
            );
            (world, Vec::new(), status)
        }
    };
    LabState {
        world,
        trace,
        cursor: 0,
        playing: mode == LabGenerationMode::CompactTrace,
        steps_per_tick: 4,
        overlay: true,
        status,
        dirty: true,
        current_level: 0,
        generation_mode: mode,
    }
}

fn production_corpus(requested_seed: u64) -> HexWfcWorld {
    let config = HexWfcConfig::arc_default();
    let quotas = HexRoomQuotas::for_team_count(PRODUCTION_TEAM_COUNT);
    let mut fallback = None;
    for offset in 0..CONCEPT_SEARCH_LIMIT {
        let seed = requested_seed.wrapping_add(offset);
        let Ok(world) = HexWfcWorld::generate_with_room_quotas(seed, config, quotas) else {
            continue;
        };
        if concept_gaps(&world).is_empty() {
            return world;
        }
        fallback = Some(world);
    }
    let fallback = fallback.expect("production corpus search must yield at least one solve");
    panic!(
        "no concept-complete production solve in {CONCEPT_SEARCH_LIMIT} seeds; last gaps: {:?}",
        concept_gaps(&fallback)
    );
}

pub(crate) fn concept_gaps(world: &HexWfcWorld) -> Vec<String> {
    let room_roles = world
        .blueprints
        .iter()
        .map(|blueprint| blueprint.role)
        .collect::<Vec<_>>();
    let hall_archetypes = world
        .placements
        .values()
        .filter_map(placement_tile_archetype)
        .collect::<BTreeSet<_>>();
    let mut gaps = PLAYED_ROOM_ROLES
        .into_iter()
        .filter(|role| !room_roles.contains(role))
        .map(|role| format!("room: {}", role.label()))
        .collect::<Vec<_>>();
    gaps.extend(
        PLAYED_HALL_ARCHETYPES
            .into_iter()
            .filter(|archetype| !hall_archetypes.contains(archetype))
            .map(|archetype| format!("hall: {archetype}")),
    );
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_mode_finds_a_full_played_concept_corpus() {
        let state = build(0xA11C_E3D0_0000_0008, LabGenerationMode::ProductionCorpus);
        assert_eq!(state.world.config, HexWfcConfig::arc_default());
        assert!(concept_gaps(&state.world).is_empty());
        assert!(state.trace.is_empty(), "production mode is a solved atlas");
    }
}
