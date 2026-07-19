//! Player-owned caged anchor lantern inventory for the hex match.
//!
//! The same logical item is carried guidance light, Guardian warning, and a
//! durable threshold lock. Ownership is player-shaped in Arc M; a future team
//! roster can migrate the owner field without changing stable equipment IDs.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::{EquipmentId, PlayerId};
use observed_facility::{
    hex_wfc::{HexObservationFrame, HexThresholdKey, HexWfcWorld},
    map_spec::RoomRole,
};
use observed_hex::{HexCoord, face_edge, hex_origin};

use super::{HexActionButtons, HexMatchEvent, HexMatchEventKind, HexWfcMatch};

#[derive(Clone, Debug, PartialEq)]
pub struct HexDeployedLantern {
    pub id: EquipmentId,
    pub owner: PlayerId,
    pub threshold: HexThresholdKey,
    pub cell: HexCoord,
    pub position: Vec3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexLanternCache {
    pub id: EquipmentId,
    pub cell: HexCoord,
    pub amount: u16,
    pub collected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HexLanternState {
    pub carried: BTreeMap<PlayerId, u16>,
    pub deployed: BTreeMap<EquipmentId, HexDeployedLantern>,
    pub caches: BTreeMap<EquipmentId, HexLanternCache>,
    next_id: u32,
}

impl HexLanternState {
    #[must_use]
    pub fn new(players: impl IntoIterator<Item = PlayerId>, world: &HexWfcWorld) -> Self {
        let carried = players.into_iter().map(|player| (player, 1)).collect();
        let mut state = Self {
            carried,
            deployed: BTreeMap::new(),
            caches: BTreeMap::new(),
            next_id: 0,
        };
        for blueprint in &world.blueprints {
            let amount = match blueprint.role {
                RoomRole::AnchorCheckpoint => 2,
                RoomRole::Recovery | RoomRole::GuardianControl => 1,
                _ => 0,
            };
            if amount == 0 {
                continue;
            }
            let id = state.allocate_id();
            state.caches.insert(
                id,
                HexLanternCache {
                    id,
                    cell: blueprint.anchor,
                    amount,
                    collected: false,
                },
            );
        }
        state
    }

    #[must_use]
    pub fn inventory(&self, player: PlayerId) -> u16 {
        self.carried.get(&player).copied().unwrap_or(0)
    }

    pub fn collect_cache(
        &mut self,
        player: PlayerId,
        cell: HexCoord,
    ) -> Option<(EquipmentId, u16)> {
        let cache = self
            .caches
            .values_mut()
            .filter(|cache| !cache.collected && cache.cell == cell)
            .min_by_key(|cache| cache.id)?;
        cache.collected = true;
        let amount = cache.amount;
        let id = cache.id;
        let carried = self.carried.entry(player).or_default();
        *carried = carried.saturating_add(amount);
        Some((id, amount))
    }

    pub fn deploy(
        &mut self,
        owner: PlayerId,
        threshold: HexThresholdKey,
        cell: HexCoord,
        position: Vec3,
    ) -> Option<EquipmentId> {
        if self
            .deployed
            .values()
            .any(|lantern| lantern.threshold == threshold)
        {
            return None;
        }
        let carried = self.carried.get_mut(&owner)?;
        if *carried == 0 {
            return None;
        }
        *carried -= 1;
        let id = self.allocate_id();
        self.deployed.insert(
            id,
            HexDeployedLantern {
                id,
                owner,
                threshold,
                cell,
                position,
            },
        );
        Some(id)
    }

    pub fn recover_nearest(
        &mut self,
        owner: PlayerId,
        position: Vec3,
        radius: f32,
    ) -> Option<EquipmentId> {
        let id = self
            .deployed
            .values()
            .filter(|lantern| lantern.owner == owner)
            .filter(|lantern| lantern.position.distance(position) <= radius)
            .min_by(|a, b| {
                a.position
                    .distance_squared(position)
                    .total_cmp(&b.position.distance_squared(position))
                    .then_with(|| a.id.cmp(&b.id))
            })?
            .id;
        self.deployed.remove(&id)?;
        let carried = self.carried.entry(owner).or_default();
        *carried = carried.saturating_add(1);
        Some(id)
    }

    pub fn apply_mutation_pins(&self, frame: &mut HexObservationFrame) {
        for cache in self.caches.values().filter(|cache| !cache.collected) {
            frame.visible_cells.insert(cache.cell);
        }
        for lantern in self.deployed.values() {
            frame.visible_cells.insert(lantern.cell);
            frame.visible_thresholds.insert(lantern.threshold);
        }
    }

    #[must_use]
    pub fn anchors_blueprint_cell(&self, world: &HexWfcWorld, cell: HexCoord) -> bool {
        self.deployed.values().any(|lantern| {
            world.blueprints.iter().any(|blueprint| {
                blueprint.generation_key() == lantern.threshold.room_generation_key
                    && blueprint.cells.contains(&cell)
            })
        })
    }

    fn allocate_id(&mut self) -> EquipmentId {
        let id = EquipmentId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        id
    }
}

impl HexWfcMatch {
    pub(super) fn step_lantern_actions(&mut self, player: PlayerId, actions: HexActionButtons) {
        if self.players[&player].escaped {
            return;
        }
        let cell = self.players[&player].cell;
        let position = self.players[&player].position;
        if actions.deploy_lantern {
            let threshold = self.looked_at_threshold(&self.players[&player]);
            if let Some(threshold) = threshold
                && let Some(door) = self
                    .door_states()
                    .into_iter()
                    .find(|door| door.key == threshold && door.open)
            {
                let world_origin = Vec3::from_array(hex_origin(door.room_cell));
                let [a, b] = face_edge(door.face);
                let edge_mid = Vec3::new((a.0 + b.0) as f32 * 0.5, 1.1, (a.1 + b.1) as f32 * 0.5);
                if self
                    .lanterns
                    .deploy(player, threshold, door.room_cell, world_origin + edge_mid)
                    .is_some()
                {
                    self.recent_events.push(HexMatchEvent {
                        tick: self.tick,
                        kind: HexMatchEventKind::AnchorDeployed,
                        player: Some(player),
                        cell: Some(door.room_cell),
                    });
                }
            }
        } else if actions.recover_lantern {
            if self
                .lanterns
                .recover_nearest(player, position, 1.8)
                .is_some()
            {
                self.recent_events.push(HexMatchEvent {
                    tick: self.tick,
                    kind: HexMatchEventKind::AnchorRecovered,
                    player: Some(player),
                    cell: Some(cell),
                });
            }
        } else if actions.interact && self.lanterns.collect_cache(player, cell).is_some() {
            self.recent_events.push(HexMatchEvent {
                tick: self.tick,
                kind: HexMatchEventKind::LanternCacheCollected,
                player: Some(player),
                cell: Some(cell),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_facility::hex_wfc::HexWfcConfig;

    fn world_with_role(role: RoomRole) -> HexWfcWorld {
        let config = HexWfcConfig::arc_default();
        (0..64u64)
            .find_map(|seed| {
                let world = HexWfcWorld::generate(seed, config).ok()?;
                world
                    .blueprints
                    .iter()
                    .any(|blueprint| blueprint.role == role)
                    .then_some(world)
            })
            .expect("seed corpus contains requested room role")
    }

    #[test]
    fn inventory_has_no_cap_and_zero_prevents_deployment() {
        let world = world_with_role(RoomRole::AnchorCheckpoint);
        let player = PlayerId(0);
        let mut state = HexLanternState::new([player], &world);
        let cache = state
            .caches
            .values()
            .find(|cache| cache.amount == 2)
            .expect("anchor cache")
            .clone();
        assert_eq!(state.collect_cache(player, cache.cell), Some((cache.id, 2)));
        assert_eq!(state.inventory(player), 3);

        let blueprint = &world.blueprints[0];
        let threshold = HexThresholdKey {
            room_generation_key: blueprint.generation_key(),
            port: "test",
        };
        state.carried.insert(player, 0);
        assert!(
            state
                .deploy(player, threshold, blueprint.anchor, Vec3::ZERO)
                .is_none()
        );
    }

    #[test]
    fn deployed_lantern_pins_cell_and_named_threshold_until_recovered() {
        let world = world_with_role(RoomRole::GuardianControl);
        let player = PlayerId(0);
        let blueprint = &world.blueprints[0];
        let threshold = HexThresholdKey {
            room_generation_key: blueprint.generation_key(),
            port: "north",
        };
        let mut state = HexLanternState::new([player], &world);
        state
            .deploy(player, threshold, blueprint.anchor, Vec3::ZERO)
            .expect("deploy");
        let mut frame = HexObservationFrame::default();
        state.apply_mutation_pins(&mut frame);
        assert!(frame.visible_cells.contains(&blueprint.anchor));
        assert!(frame.visible_thresholds.contains(&threshold));
        assert!(state.anchors_blueprint_cell(&world, blueprint.anchor));
        state
            .recover_nearest(player, Vec3::ZERO, 1.5)
            .expect("recover");
        assert_eq!(state.inventory(player), 1);
    }
}
