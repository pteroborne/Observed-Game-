use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::{EquipmentId, PlayerId, TeamId};
use observed_facility::full_wfc::{
    CellCoord, FullWfcWorld, ModuleFace, ObservationFrame, ThresholdKey,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeployableKind {
    Anchor,
    TeleportPad,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Deployable {
    pub id: EquipmentId,
    pub team: TeamId,
    pub kind: DeployableKind,
    pub cell: CellCoord,
    pub position: Vec3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeamLoadout {
    pub carried_anchors: u8,
    pub carried_pads: u8,
}

impl Default for TeamLoadout {
    fn default() -> Self {
        Self {
            carried_anchors: 1,
            carried_pads: 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentState {
    pub loadouts: BTreeMap<TeamId, TeamLoadout>,
    pub deployed: BTreeMap<EquipmentId, Deployable>,
    pub pad_latches: BTreeMap<PlayerId, EquipmentId>,
    next_id: u32,
}

impl EquipmentState {
    pub fn new(teams: impl IntoIterator<Item = TeamId>) -> Self {
        Self {
            loadouts: teams
                .into_iter()
                .map(|team| (team, TeamLoadout::default()))
                .collect(),
            deployed: BTreeMap::new(),
            pad_latches: BTreeMap::new(),
            next_id: 0,
        }
    }

    pub fn deploy(
        &mut self,
        team: TeamId,
        kind: DeployableKind,
        cell: CellCoord,
        position: Vec3,
    ) -> Option<EquipmentId> {
        let loadout = self.loadouts.get_mut(&team)?;
        let carried = match kind {
            DeployableKind::Anchor => &mut loadout.carried_anchors,
            DeployableKind::TeleportPad => &mut loadout.carried_pads,
        };
        if *carried == 0 {
            return None;
        }
        *carried -= 1;
        let id = EquipmentId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.deployed.insert(
            id,
            Deployable {
                id,
                team,
                kind,
                cell,
                position,
            },
        );
        Some(id)
    }

    pub fn recover_nearest(
        &mut self,
        team: TeamId,
        cell: CellCoord,
        position: Vec3,
        radius: f32,
    ) -> Option<(EquipmentId, DeployableKind)> {
        let id = self
            .deployed
            .values()
            .filter(|item| item.team == team && item.cell == cell)
            .filter(|item| item.position.distance(position) <= radius)
            .min_by(|a, b| {
                a.position
                    .distance_squared(position)
                    .total_cmp(&b.position.distance_squared(position))
                    .then_with(|| a.id.cmp(&b.id))
            })?
            .id;
        let item = self.deployed.remove(&id)?;
        let loadout = self.loadouts.get_mut(&team)?;
        match item.kind {
            DeployableKind::Anchor => loadout.carried_anchors += 1,
            DeployableKind::TeleportPad => loadout.carried_pads += 1,
        }
        self.pad_latches.retain(|_, latched| *latched != id);
        Some((id, item.kind))
    }

    pub fn pad_target(
        &mut self,
        player: PlayerId,
        team: TeamId,
        cell: CellCoord,
        position: Vec3,
        radius: f32,
    ) -> Option<(EquipmentId, CellCoord, Vec3)> {
        let pads = self
            .deployed
            .values()
            .filter(|item| item.team == team && item.kind == DeployableKind::TeleportPad)
            .collect::<Vec<_>>();
        if pads.len() != 2 {
            return None;
        }
        let source = pads
            .iter()
            .copied()
            .find(|pad| pad.cell == cell && pad.position.distance(position) <= radius)?;
        if self.pad_latches.get(&player) == Some(&source.id) {
            return None;
        }
        let target = pads.into_iter().find(|pad| pad.id != source.id)?;
        self.pad_latches.insert(player, target.id);
        Some((target.id, target.cell, target.position))
    }

    pub fn clear_pad_latch_when_away(
        &mut self,
        player: PlayerId,
        cell: CellCoord,
        position: Vec3,
        radius: f32,
    ) {
        let still_on_latched = self
            .pad_latches
            .get(&player)
            .and_then(|id| self.deployed.get(id))
            .is_some_and(|pad| pad.cell == cell && pad.position.distance(position) <= radius);
        if !still_on_latched {
            self.pad_latches.remove(&player);
        }
    }

    pub fn apply_relayout_pins(&self, world: &FullWfcWorld, frame: &mut ObservationFrame) {
        for item in self.deployed.values() {
            frame.visible_cells.insert(item.cell);
            if item.kind == DeployableKind::Anchor
                && let Some(room) = world.room_at(item.cell)
                && let Some(room) = world.room(room)
            {
                for face in ModuleFace::ALL {
                    frame.visible_thresholds.insert(ThresholdKey {
                        room: room.coord,
                        face,
                    });
                }
            }
        }
    }

    pub fn anchors_in(&self, cell: CellCoord) -> impl Iterator<Item = &Deployable> {
        self.deployed
            .values()
            .filter(move |item| item.kind == DeployableKind::Anchor && item.cell == cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_facility::full_wfc::FullWfcConfig;

    #[test]
    fn pads_are_team_keyed_bidirectional_and_latched() {
        let mut equipment = EquipmentState::new([TeamId(0), TeamId(1)]);
        let a = CellCoord::new(1, 1, 0);
        let b = CellCoord::new(4, 2, 1);
        equipment.deploy(TeamId(0), DeployableKind::TeleportPad, a, Vec3::ZERO);
        equipment.deploy(TeamId(0), DeployableKind::TeleportPad, b, Vec3::X);
        assert!(
            equipment
                .pad_target(PlayerId(2), TeamId(1), a, Vec3::ZERO, 1.2)
                .is_none()
        );
        let target = equipment
            .pad_target(PlayerId(0), TeamId(0), a, Vec3::ZERO, 1.2)
            .expect("link");
        assert_eq!((target.1, target.2), (b, Vec3::X));
        assert!(
            equipment
                .pad_target(PlayerId(0), TeamId(0), b, Vec3::X, 1.2)
                .is_none()
        );
        equipment.clear_pad_latch_when_away(PlayerId(0), b, Vec3::splat(9.0), 1.2);
        assert!(
            equipment
                .pad_target(PlayerId(0), TeamId(0), b, Vec3::X, 1.2)
                .is_some()
        );
    }

    #[test]
    fn pads_pin_only_cells_while_anchors_pin_room_thresholds() {
        let world = FullWfcWorld::new(53, FullWfcConfig::default()).expect("world");
        let anchor_cell = world.spawn();
        let pad_cell = world
            .rooms
            .values()
            .find(|room| room.coord != anchor_cell)
            .expect("other room")
            .coord;
        let mut equipment = EquipmentState::new([TeamId(0)]);
        equipment.deploy(TeamId(0), DeployableKind::Anchor, anchor_cell, Vec3::ZERO);
        equipment.deploy(TeamId(0), DeployableKind::TeleportPad, pad_cell, Vec3::ZERO);
        let mut frame = ObservationFrame::default();
        equipment.apply_relayout_pins(&world, &mut frame);
        assert!(frame.visible_cells.contains(&anchor_cell));
        assert!(frame.visible_cells.contains(&pad_cell));
        assert_eq!(
            frame
                .visible_thresholds
                .iter()
                .filter(|threshold| threshold.room == anchor_cell)
                .count(),
            ModuleFace::ALL.len()
        );
        assert!(
            !frame
                .visible_thresholds
                .iter()
                .any(|threshold| threshold.room == pad_cell)
        );
    }
}
