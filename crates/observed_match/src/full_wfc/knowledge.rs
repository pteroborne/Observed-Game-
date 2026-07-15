//! Team-scoped survivor-sketch knowledge for the tactical map.

use std::collections::BTreeMap;

use observed_core::TeamId;
use observed_facility::full_wfc::{CellCoord, FullWfcWorld, ModuleFace, ModuleSpace};

use super::{DeployableKind, EquipmentState, PlayerState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapDiscovery {
    Glimpsed,
    Traversed,
}

#[cfg(test)]
mod tests {
    use observed_core::{PlayerId, TeamId};
    use observed_facility::full_wfc::{FullWfcConfig, FullWfcWorld};

    use super::*;
    use crate::full_wfc::{EquipmentState, PlayerState, cell_origin};

    #[test]
    fn traversed_history_survives_and_becomes_stale_after_generation_changes() {
        let mut world = FullWfcWorld::new(61, FullWfcConfig::default()).expect("world");
        let cell = world.spawn();
        let player = PlayerState {
            id: PlayerId(0),
            team: TeamId(0),
            cell,
            position: cell_origin(cell),
            yaw: 0.0,
            pitch: 0.0,
            climb_target: None,
            escaped: false,
        };
        let equipment = EquipmentState::new([TeamId(0)]);
        let mut knowledge = TeamMapKnowledge::default();
        knowledge.observe(TeamId(0), &world, [player.clone()].into_iter(), &equipment);
        let known = knowledge.cells[&cell];
        assert_eq!(known.discovery, MapDiscovery::Traversed);
        world.generation += 1;
        assert!(knowledge.cells[&cell].is_stale(world.generation));
        knowledge.observe(TeamId(0), &world, [player].into_iter(), &equipment);
        assert!(!knowledge.cells[&cell].is_stale(world.generation));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MapCellKnowledge {
    pub discovery: MapDiscovery,
    pub last_confirmed_generation: u32,
    pub anchored: bool,
}

impl MapCellKnowledge {
    pub fn is_stale(self, live_generation: u32) -> bool {
        self.last_confirmed_generation < live_generation && !self.anchored
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TeamMapKnowledge {
    pub cells: BTreeMap<CellCoord, MapCellKnowledge>,
}

impl TeamMapKnowledge {
    pub(super) fn survey(&mut self, world: &FullWfcWorld) {
        for placement in world
            .placements
            .values()
            .filter(|placement| placement.space != ModuleSpace::Void)
        {
            self.record(
                placement.coord,
                MapDiscovery::Glimpsed,
                world.generation,
                false,
            );
        }
    }

    pub(super) fn observe(
        &mut self,
        team: TeamId,
        world: &FullWfcWorld,
        players: impl Iterator<Item = PlayerState>,
        equipment: &EquipmentState,
    ) {
        for known in self.cells.values_mut() {
            known.anchored = false;
        }
        for player in players.filter(|player| player.team == team && !player.escaped) {
            self.record(
                player.cell,
                MapDiscovery::Traversed,
                world.generation,
                false,
            );
            let placement = &world.placements[&player.cell];
            for face in ModuleFace::ALL {
                if placement.is_open(face)
                    && let Some(cell) = world.config.neighbor(player.cell, face)
                    && world.placements[&cell].space != ModuleSpace::Void
                {
                    self.record(cell, MapDiscovery::Glimpsed, world.generation, false);
                }
            }
        }
        for item in equipment.deployed.values().filter(|item| item.team == team) {
            self.record(
                item.cell,
                MapDiscovery::Traversed,
                world.generation,
                item.kind == DeployableKind::Anchor,
            );
        }
    }

    fn record(
        &mut self,
        cell: CellCoord,
        discovery: MapDiscovery,
        generation: u32,
        anchored: bool,
    ) {
        self.cells
            .entry(cell)
            .and_modify(|known| {
                if discovery == MapDiscovery::Traversed {
                    known.discovery = MapDiscovery::Traversed;
                }
                known.last_confirmed_generation = generation;
                known.anchored |= anchored;
            })
            .or_insert(MapCellKnowledge {
                discovery,
                last_confirmed_generation: generation,
                anchored,
            });
    }
}
