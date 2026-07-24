//! Physical observation-driven Guardian for the hex facility.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::PlayerId;
use observed_facility::{hex_wfc::HexWfcWorld, map_spec::RoomRole};
use observed_hex::{HexCoord, hex_origin};

use super::{HexLanternState, HexMatchEvent, HexMatchEventKind, HexPlayerState};

const MOVE_PERIOD_TICKS: u64 = 120;
const GUARDIAN_SPEED: f32 = 2.5;
const CATCH_DISTANCE: f32 = 1.1;
const FIXED_DT: f32 = 1.0 / 60.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexGuardianStatus {
    Active,
    FrozenByPlayer,
    FrozenByAnchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HexGuardianState {
    pub cell: HexCoord,
    pub position: Vec3,
    pub status: HexGuardianStatus,
    pub target: Option<PlayerId>,
}

impl HexGuardianState {
    #[must_use]
    pub fn new(world: &HexWfcWorld) -> Self {
        let cell = guardian_home(world);
        Self {
            cell,
            position: Vec3::from_array(hex_origin(cell)) + Vec3::Y * 0.9,
            status: HexGuardianStatus::Active,
            target: None,
        }
    }

    #[must_use]
    pub fn pressure_for(&self, world: &HexWfcWorld, player: &HexPlayerState) -> f32 {
        if player.cell == self.cell {
            return (1.0 - player.position.distance(self.position) / 12.0).clamp(0.55, 1.0);
        }
        world
            .route_between_cells(player.cell, self.cell)
            .map_or(0.0, |route| {
                (1.0 - route.cost_millis as f32 / 12_000.0).clamp(0.0, 0.8)
            })
    }

    pub fn step(
        &mut self,
        tick: u64,
        world: &HexWfcWorld,
        lanterns: &HexLanternState,
        players: &mut BTreeMap<PlayerId, HexPlayerState>,
        events: &mut Vec<HexMatchEvent>,
    ) {
        let observed = players
            .values()
            .any(|player| player_sees_guardian(world, player, self));
        let anchored = lanterns.anchors_blueprint_cell(world, self.cell);
        self.status = if observed {
            HexGuardianStatus::FrozenByPlayer
        } else if anchored {
            HexGuardianStatus::FrozenByAnchor
        } else {
            HexGuardianStatus::Active
        };
        if self.status != HexGuardianStatus::Active {
            return;
        }

        let Some(target_id) = leading_player(world, players) else {
            self.target = None;
            return;
        };
        self.target = Some(target_id);
        let target_cell = players[&target_id].cell;
        if target_cell == self.cell {
            let target_position = players[&target_id].position;
            self.position +=
                (target_position - self.position).normalize_or_zero() * GUARDIAN_SPEED * FIXED_DT;
            if self.position.distance(target_position) <= CATCH_DISTANCE
                && let Some(destination) = recovery_destination(world, self.cell)
            {
                let player = players.get_mut(&target_id).expect("target exists");
                let from = player.cell;
                player.cell = destination;
                player.position = Vec3::from_array(hex_origin(destination)) + Vec3::Y * 0.9;
                events.push(HexMatchEvent {
                    tick,
                    kind: HexMatchEventKind::GuardianCatch,
                    player: Some(target_id),
                    cell: Some(from),
                });
                // A catch is one complete pressure cycle. Returning home gives
                // the recovered runner a readable new attempt instead of
                // allowing the Guardian to camp the last survivor forever.
                self.cell = guardian_home(world);
                self.position = Vec3::from_array(hex_origin(self.cell)) + Vec3::Y * 0.9;
                self.target = None;
            }
        } else if tick.is_multiple_of(MOVE_PERIOD_TICKS)
            && let Some(route) = world.route_between_cells(self.cell, target_cell)
            && let Some(&next) = route.cells.get(1)
        {
            self.cell = next;
            self.position = Vec3::from_array(hex_origin(next)) + Vec3::Y * 0.9;
        }
    }
}

fn guardian_home(world: &HexWfcWorld) -> HexCoord {
    world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.role == RoomRole::GuardianControl)
        .or_else(|| world.blueprints.first())
        .map_or_else(|| world.config.spawn(), |blueprint| blueprint.anchor)
}

fn leading_player(
    world: &HexWfcWorld,
    players: &BTreeMap<PlayerId, HexPlayerState>,
) -> Option<PlayerId> {
    let active = players.values().filter(|player| !player.escaped).count();
    // The Guardian is competitive pressure, not a single-player traversal
    // blocker. Once only one runner remains (including one-player labs), the
    // route itself is the remaining challenge.
    if active <= 1 {
        return None;
    }
    players
        .values()
        .filter(|player| !player.escaped)
        .min_by_key(|player| {
            (
                world
                    .route_between_cells(player.cell, world.config.exit())
                    .map_or(u32::MAX, |route| route.cost_millis),
                player.id,
            )
        })
        .map(|player| player.id)
}

fn recovery_destination(world: &HexWfcWorld, guardian_cell: HexCoord) -> Option<HexCoord> {
    world
        .blueprints
        .iter()
        .filter(|blueprint| blueprint.anchor != guardian_cell)
        .max_by_key(|blueprint| {
            (
                u8::from(blueprint.role == RoomRole::Recovery),
                world
                    .route_between_cells(blueprint.anchor, world.config.exit())
                    .map_or(0, |route| route.cost_millis),
                std::cmp::Reverse(blueprint.anchor),
            )
        })
        .map(|blueprint| blueprint.anchor)
}

fn player_sees_guardian(
    world: &HexWfcWorld,
    player: &HexPlayerState,
    guardian: &HexGuardianState,
) -> bool {
    if player.escaped {
        return false;
    }
    let visible_route = world
        .route_between(player.cell, guardian.cell)
        .is_some_and(|route| route.len() <= 2);
    if !visible_route {
        return false;
    }
    let offset = guardian.position - player.position;
    let distance = offset.length();
    if !(0.15..=14.0).contains(&distance) {
        return false;
    }
    let forward = Vec3::new(player.yaw.sin(), 0.0, -player.yaw.cos());
    forward.dot(offset.normalize_or_zero()) > 0.42
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_core::PlayerId;
    use observed_facility::hex_wfc::HexWfcConfig;

    fn guardian_world() -> (HexWfcWorld, HexCoord) {
        let config = HexWfcConfig {
            levels: 3,
            ..HexWfcConfig::default()
        };
        for seed in 0..2_000 {
            let Ok(world) = HexWfcWorld::generate(seed, config) else {
                continue;
            };
            if let Some(anchor) = world
                .blueprints
                .iter()
                .find(|blueprint| blueprint.role == RoomRole::GuardianControl)
                .map(|room| room.anchor)
            {
                return (world, anchor);
            }
        }
        panic!("seed corpus did not stamp GuardianControl");
    }

    fn player(id: PlayerId, cell: HexCoord, position: Vec3, yaw: f32) -> HexPlayerState {
        HexPlayerState {
            id,
            team: observed_core::TeamId((id.0 / 2) as u8),
            cell,
            position,
            yaw,
            pitch: 0.0,
            escaped: false,
        }
    }

    #[test]
    fn guardian_spawns_in_control_room_when_present() {
        let (world, anchor) = guardian_world();
        assert_eq!(HexGuardianState::new(&world).cell, anchor);
    }

    #[test]
    fn direct_observation_freezes_the_physical_guardian() {
        let (world, cell) = guardian_world();
        let mut guardian = HexGuardianState::new(&world);
        let before = guardian.position;
        let id = PlayerId(0);
        let mut players =
            BTreeMap::from([(id, player(id, cell, guardian.position + Vec3::Z * 5.0, 0.0))]);
        let lanterns = HexLanternState::new([id], &world);
        guardian.step(120, &world, &lanterns, &mut players, &mut Vec::new());
        assert_eq!(guardian.status, HexGuardianStatus::FrozenByPlayer);
        assert_eq!(guardian.position, before);
    }

    #[test]
    fn a_lantern_anchoring_the_control_room_freezes_the_guardian() {
        let (world, cell) = guardian_world();
        let mut guardian = HexGuardianState::new(&world);
        let id = PlayerId(0);
        let mut lanterns = HexLanternState::new([id], &world);
        let room = world
            .blueprints
            .iter()
            .find(|blueprint| blueprint.cells.contains(&cell))
            .expect("guardian room");
        lanterns
            .deploy(
                id,
                observed_facility::hex_wfc::HexThresholdKey {
                    room_generation_key: room.generation_key(),
                    port: "lower_port",
                },
                cell,
                guardian.position,
            )
            .expect("deploy anchor");
        let far = world.config.exit();
        let mut players = BTreeMap::from([(
            id,
            player(id, far, Vec3::from_array(hex_origin(far)) + Vec3::Y, 0.0),
        )]);
        guardian.step(120, &world, &lanterns, &mut players, &mut Vec::new());
        assert_eq!(guardian.status, HexGuardianStatus::FrozenByAnchor);
    }

    #[test]
    fn an_unobserved_catch_sends_the_leader_to_recovery() {
        let (world, _) = guardian_world();
        let mut guardian = HexGuardianState::new(&world);
        let cell = world.config.exit();
        guardian.cell = cell;
        guardian.position = Vec3::from_array(hex_origin(cell)) + Vec3::Y * 0.9;
        let id = PlayerId(0);
        let rival_id = PlayerId(1);
        let start = guardian.position + Vec3::Z;
        let rival_cell = world.config.spawn();
        let mut players = BTreeMap::from([
            (
                id,
                // Facing +Z while the Guardian is behind them along -Z.
                player(id, cell, start, std::f32::consts::PI),
            ),
            (
                rival_id,
                player(
                    rival_id,
                    rival_cell,
                    Vec3::from_array(hex_origin(rival_cell)) + Vec3::Y,
                    0.0,
                ),
            ),
        ]);
        let lanterns = HexLanternState::new([id, rival_id], &world);
        let mut events = Vec::new();
        guardian.step(1, &world, &lanterns, &mut players, &mut events);
        assert_ne!(players[&id].cell, cell);
        assert_eq!(guardian.cell, guardian_home(&world));
        assert_eq!(guardian.target, None);
        assert!(events.iter().any(|event| {
            event.kind == HexMatchEventKind::GuardianCatch && event.player == Some(id)
        }));
    }
}
