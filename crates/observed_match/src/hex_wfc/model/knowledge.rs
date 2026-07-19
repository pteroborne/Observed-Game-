//! Player-local, mutation-aware survivor-sketch knowledge for the hex match.

use std::collections::BTreeMap;

use observed_facility::hex_wfc::{HexSpace, HexWfcWorld, PortSignature};
use observed_hex::{HexCoord, HexFace};

use super::{HexLanternState, HexPlayerState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexMapDiscovery {
    Glimpsed,
    Traversed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexMapCellKnowledge {
    pub discovery: HexMapDiscovery,
    pub last_confirmed_revision: u32,
    pub known_ports: PortSignature,
    pub anchored: bool,
}

impl HexMapCellKnowledge {
    #[must_use]
    pub fn is_stale(self, world: &HexWfcWorld, cell: HexCoord) -> bool {
        !self.anchored
            && world.cell_revisions.get(&cell).copied().unwrap_or(0) > self.last_confirmed_revision
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HexPlayerMapKnowledge {
    pub cells: BTreeMap<HexCoord, HexMapCellKnowledge>,
}

impl HexPlayerMapKnowledge {
    pub fn observe(
        &mut self,
        world: &HexWfcWorld,
        player: &HexPlayerState,
        lanterns: &HexLanternState,
    ) {
        for known in self.cells.values_mut() {
            known.anchored = false;
        }
        if !player.escaped {
            self.record(world, player.cell, HexMapDiscovery::Traversed, false);
            let placement = &world.placements[&player.cell];
            for face in HexFace::ALL {
                if placement.ports().port(face) != observed_hex::PortClass::Sealed
                    && let Some(cell) = world.config.grid().neighbor(player.cell, face)
                    && world.placements[&cell].space != HexSpace::Void
                {
                    self.record(world, cell, HexMapDiscovery::Glimpsed, false);
                }
            }
        }
        for lantern in lanterns
            .deployed
            .values()
            .filter(|item| item.owner == player.id)
        {
            self.record(world, lantern.cell, HexMapDiscovery::Traversed, true);
        }
    }

    fn record(
        &mut self,
        world: &HexWfcWorld,
        cell: HexCoord,
        discovery: HexMapDiscovery,
        anchored: bool,
    ) {
        let revision = world.cell_revisions.get(&cell).copied().unwrap_or(0);
        let known_ports = world.placements[&cell].ports();
        self.cells
            .entry(cell)
            .and_modify(|known| {
                if discovery == HexMapDiscovery::Traversed {
                    known.discovery = HexMapDiscovery::Traversed;
                }
                known.last_confirmed_revision = revision;
                known.known_ports = known_ports;
                known.anchored |= anchored;
            })
            .or_insert(HexMapCellKnowledge {
                discovery,
                last_confirmed_revision: revision,
                known_ports,
                anchored,
            });
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;
    use observed_core::PlayerId;
    use observed_facility::hex_wfc::HexWfcConfig;

    use super::*;

    #[test]
    fn unrelated_generation_change_does_not_stale_known_cell() {
        let mut world = HexWfcWorld::generate(91, HexWfcConfig::default()).expect("world");
        let cell = world.config.spawn();
        let player = HexPlayerState {
            id: PlayerId(0),
            cell,
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            climb_target: None,
            transit_target: None,
            escaped: false,
        };
        let lanterns = HexLanternState::new([player.id], &world);
        let mut map = HexPlayerMapKnowledge::default();
        map.observe(&world, &player, &lanterns);
        world.generation += 1;
        assert!(!map.cells[&cell].is_stale(&world, cell));
        *world.cell_revisions.entry(cell).or_default() += 1;
        assert!(map.cells[&cell].is_stale(&world, cell));
        map.observe(&world, &player, &lanterns);
        assert!(!map.cells[&cell].is_stale(&world, cell));
    }
}
