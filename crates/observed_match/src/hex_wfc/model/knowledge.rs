//! Player-local, mutation-aware survivor-sketch knowledge for the hex match.

use std::collections::BTreeMap;

use observed_core::TeamId;
use observed_facility::hex_wfc::{HexSpace, HexWfcWorld, PortSignature};
use observed_facility::map_spec::RoomRole;
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
    /// Known only after entering the room or using a local survey monitor.
    pub room_role: Option<RoomRole>,
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
    pub fn observe_team<'a>(
        &mut self,
        team: TeamId,
        world: &HexWfcWorld,
        players: impl Iterator<Item = &'a HexPlayerState>,
        lanterns: &HexLanternState,
    ) {
        let players = players.collect::<Vec<_>>();
        let member_ids = players.iter().map(|player| player.id).collect::<Vec<_>>();
        for known in self.cells.values_mut() {
            known.anchored = false;
        }
        for player in players
            .into_iter()
            .filter(|player| player.team == team && !player.escaped)
        {
            self.record(world, player.cell, HexMapDiscovery::Traversed, false, true);
            let placement = &world.placements[&player.cell];
            for face in HexFace::ALL {
                if placement.ports().port(face) != observed_hex::PortClass::Sealed
                    && let Some(cell) = world.config.grid().neighbor(player.cell, face)
                    && world.placements[&cell].space != HexSpace::Void
                {
                    self.record(world, cell, HexMapDiscovery::Glimpsed, false, false);
                }
            }
        }
        for lantern in lanterns
            .deployed
            .values()
            .filter(|item| member_ids.contains(&item.owner))
        {
            self.record(world, lantern.cell, HexMapDiscovery::Traversed, true, true);
        }
    }

    /// Reveal a bounded, team-local two-hop schematic around a monitor.
    pub fn survey_local(&mut self, world: &HexWfcWorld, origin: HexCoord, hops: usize) {
        for (&cell, placement) in &world.placements {
            if placement.space == HexSpace::Void {
                continue;
            }
            let within = world
                .route_between(origin, cell)
                .is_some_and(|route| route.len().saturating_sub(1) <= hops);
            if within {
                self.record(world, cell, HexMapDiscovery::Glimpsed, false, true);
            }
        }
    }

    fn record(
        &mut self,
        world: &HexWfcWorld,
        cell: HexCoord,
        discovery: HexMapDiscovery,
        anchored: bool,
        role_known: bool,
    ) {
        let revision = world.cell_revisions.get(&cell).copied().unwrap_or(0);
        let known_ports = world.placements[&cell].ports();
        let room_role = role_known
            .then(|| {
                world
                    .blueprints
                    .iter()
                    .find(|blueprint| blueprint.cells.contains(&cell))
                    .map(|blueprint| blueprint.role)
            })
            .flatten();
        self.cells
            .entry(cell)
            .and_modify(|known| {
                if discovery == HexMapDiscovery::Traversed {
                    known.discovery = HexMapDiscovery::Traversed;
                }
                known.last_confirmed_revision = revision;
                known.known_ports = known_ports;
                known.anchored |= anchored;
                if room_role.is_some() {
                    known.room_role = room_role;
                }
            })
            .or_insert(HexMapCellKnowledge {
                discovery,
                last_confirmed_revision: revision,
                known_ports,
                anchored,
                room_role,
            });
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;
    use observed_core::{PlayerId, TeamId};
    use observed_facility::hex_wfc::HexWfcConfig;

    use super::*;

    #[test]
    fn unrelated_generation_change_does_not_stale_known_cell() {
        let mut world = HexWfcWorld::generate(91, HexWfcConfig::default()).expect("world");
        let cell = world.config.spawn();
        let player = HexPlayerState {
            id: PlayerId(0),
            team: TeamId(0),
            cell,
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            escaped: false,
        };
        let lanterns = HexLanternState::new([player.id], &world);
        let mut map = HexPlayerMapKnowledge::default();
        map.observe_team(TeamId(0), &world, [&player].into_iter(), &lanterns);
        assert_eq!(
            map.cells[&cell].room_role,
            Some(observed_facility::map_spec::RoomRole::Start)
        );
        assert!(
            map.cells
                .values()
                .filter(|known| known.discovery == HexMapDiscovery::Glimpsed)
                .all(|known| known.room_role.is_none()),
            "glimpsing a threshold must not reveal the room function"
        );
        world.generation += 1;
        assert!(!map.cells[&cell].is_stale(&world, cell));
        *world.cell_revisions.entry(cell).or_default() += 1;
        assert!(map.cells[&cell].is_stale(&world, cell));
        map.observe_team(TeamId(0), &world, [&player].into_iter(), &lanterns);
        assert!(!map.cells[&cell].is_stale(&world, cell));

        let survey = world
            .blueprints
            .iter()
            .find(|blueprint| blueprint.anchor != cell)
            .expect("another room");
        map.survey_local(&world, survey.anchor, 2);
        assert_eq!(map.cells[&survey.anchor].room_role, Some(survey.role));
    }
}
