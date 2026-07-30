//! Deliberate room objectives for the canonical hex match.

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;
use observed_authoring::RoomSocketKind;
use observed_core::{PlayerId, TeamId};
use observed_hex::{HexCoord, hex_origin, travel_distance};

use super::{HexActionButtons, HexInputFrame, HexMatchEvent, HexMatchEventKind, HexWfcMatch};

pub const KEYSTONES_REQUIRED: u8 = 2;
pub const DUAL_STATION_HOLD_TICKS: u16 = 120;
const INTERACTION_RADIUS: f32 = 2.2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexObjectiveState {
    pub enabled: bool,
    pub keystones_required: u8,
    pub available_keystones: BTreeSet<u64>,
    pub surveyed_monitors: BTreeSet<(TeamId, u64)>,
}

impl HexObjectiveState {
    pub(super) fn new(game: &HexWfcMatch) -> Self {
        let available_keystones = game
            .geometry
            .sockets
            .iter()
            .filter(|socket| socket.kind == RoomSocketKind::Keystone)
            .map(|socket| socket.room_generation_key)
            .collect::<BTreeSet<_>>();
        let has_station_a = game
            .geometry
            .sockets
            .iter()
            .any(|socket| socket.kind == RoomSocketKind::StationA);
        let has_station_b = game
            .geometry
            .sockets
            .iter()
            .any(|socket| socket.kind == RoomSocketKind::StationB);
        Self {
            enabled: available_keystones.len() >= usize::from(KEYSTONES_REQUIRED)
                && has_station_a
                && has_station_b,
            keystones_required: KEYSTONES_REQUIRED,
            available_keystones,
            surveyed_monitors: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct HexObjectiveTarget {
    pub cell: HexCoord,
    pub position: Vec3,
    pub kind: RoomSocketKind,
}

impl HexWfcMatch {
    pub(super) fn step_objectives(&mut self, frame: &HexInputFrame) {
        if !self.objectives.enabled {
            return;
        }
        for (&player, command) in &frame.commands {
            if !command.actions.interact || self.players.get(&player).is_none_or(|p| p.escaped) {
                continue;
            }
            self.try_collect_keystone(player);
            self.try_survey_monitor(player);
        }
        self.resolve_dual_stations(frame);
    }

    fn try_collect_keystone(&mut self, player: PlayerId) {
        let state = &self.players[&player];
        let team = state.team;
        if self.teams[&team].objectives.keystones >= self.objectives.keystones_required {
            return;
        }
        let Some(socket) = self.geometry.sockets.iter().find(|socket| {
            socket.kind == RoomSocketKind::Keystone
                && socket.cell == state.cell
                && state.position.distance(socket.position) <= INTERACTION_RADIUS
                && self
                    .objectives
                    .available_keystones
                    .contains(&socket.room_generation_key)
        }) else {
            return;
        };
        let room = socket.room_generation_key;
        let cell = socket.cell;
        self.objectives.available_keystones.remove(&room);
        self.teams
            .get_mut(&team)
            .expect("player team")
            .objectives
            .keystones += 1;
        self.recent_events.push(HexMatchEvent {
            tick: self.tick,
            kind: HexMatchEventKind::KeystoneCollected,
            player: Some(player),
            cell: Some(cell),
        });
    }

    fn try_survey_monitor(&mut self, player: PlayerId) {
        let state = &self.players[&player];
        let Some(socket) = self.geometry.sockets.iter().find(|socket| {
            socket.kind == RoomSocketKind::Monitor
                && socket.cell == state.cell
                && state.position.distance(socket.position) <= INTERACTION_RADIUS
        }) else {
            return;
        };
        let key = (state.team, socket.room_generation_key);
        if !self.objectives.surveyed_monitors.insert(key) {
            return;
        }
        self.map_knowledge
            .get_mut(&state.team)
            .expect("team knowledge")
            .survey_local(&self.facility, socket.cell, 2);
        self.recent_events.push(HexMatchEvent {
            tick: self.tick,
            kind: HexMatchEventKind::MonitorSurveyed,
            player: Some(player),
            cell: Some(socket.cell),
        });
    }

    fn resolve_dual_stations(&mut self, frame: &HexInputFrame) {
        let mut stations =
            BTreeMap::<u64, (Option<(Vec3, HexCoord)>, Option<(Vec3, HexCoord)>)>::new();
        for socket in self.geometry.sockets.iter().filter(|socket| {
            matches!(
                socket.kind,
                RoomSocketKind::StationA | RoomSocketKind::StationB
            )
        }) {
            let entry = stations
                .entry(socket.room_generation_key)
                .or_insert((None, None));
            match socket.kind {
                RoomSocketKind::StationA => entry.0 = Some((socket.position, socket.cell)),
                RoomSocketKind::StationB => entry.1 = Some((socket.position, socket.cell)),
                _ => unreachable!(),
            }
        }

        for team in self.teams.keys().copied().collect::<Vec<_>>() {
            if self.teams[&team].objectives.dual_station_complete {
                continue;
            }
            let members = self.teams[&team].members.clone();
            let active = members
                .iter()
                .copied()
                .filter(|member| !self.players[member].escaped)
                .collect::<Vec<_>>();
            let synchronized = stations.iter().find_map(|(&room, &(a, b))| {
                let (Some((a, a_cell)), Some((b, b_cell))) = (a, b) else {
                    return None;
                };
                let holding = |member: PlayerId| {
                    frame
                        .commands
                        .get(&member)
                        .is_some_and(|command| command.actions.interact)
                };
                let near = |member: PlayerId, target: Vec3, cell: HexCoord| {
                    self.players[&member].cell == cell
                        && self.players[&member].position.distance(target) <= INTERACTION_RADIUS
                        && holding(member)
                };
                let ready = if active.len() <= 1 {
                    active
                        .first()
                        .is_some_and(|&member| near(member, a, a_cell) || near(member, b, b_cell))
                } else {
                    active.iter().any(|&a_member| {
                        near(a_member, a, a_cell)
                            && active
                                .iter()
                                .any(|&b_member| b_member != a_member && near(b_member, b, b_cell))
                    })
                };
                ready.then_some((room, a_cell, active.first().copied()))
            });

            let Some((room, cell, participant)) = synchronized else {
                let objectives = &mut self.teams.get_mut(&team).expect("team").objectives;
                objectives.dual_station_ticks = 0;
                objectives.dual_station_room = None;
                continue;
            };
            let objectives = &mut self.teams.get_mut(&team).expect("team").objectives;
            if objectives.dual_station_room != Some(room) {
                objectives.dual_station_room = Some(room);
                objectives.dual_station_ticks = 0;
            }
            objectives.dual_station_ticks = objectives.dual_station_ticks.saturating_add(1);
            let ticks = objectives.dual_station_ticks;
            if ticks >= DUAL_STATION_HOLD_TICKS {
                objectives.dual_station_complete = true;
                self.recent_events.push(HexMatchEvent {
                    tick: self.tick,
                    kind: HexMatchEventKind::DualStationCompleted,
                    player: participant,
                    cell: Some(cell),
                });
            } else if ticks.is_multiple_of(30) {
                self.recent_events.push(HexMatchEvent {
                    tick: self.tick,
                    kind: HexMatchEventKind::DualStationProgress,
                    player: participant,
                    cell: Some(cell),
                });
            }
        }
    }

    pub(super) fn objective_target(&self, player: PlayerId) -> Option<HexObjectiveTarget> {
        let state = self.players.get(&player)?;
        if state.escaped {
            return None;
        }
        if !self.objectives.enabled {
            let cell = self.facility.config.exit();
            return Some(HexObjectiveTarget {
                cell,
                position: Vec3::from_array(hex_origin(cell)),
                kind: RoomSocketKind::Exit,
            });
        }
        let team = &self.teams[&state.team];
        if team.objectives.keystones < self.objectives.keystones_required {
            return self.nearest_socket_target(player, |socket| {
                socket.kind == RoomSocketKind::Keystone
                    && self
                        .objectives
                        .available_keystones
                        .contains(&socket.room_generation_key)
            });
        }
        if !team.objectives.dual_station_complete {
            let member_index = team
                .members
                .iter()
                .position(|member| *member == player)
                .unwrap_or(0);
            let wanted = if team.members.len() == 1 || member_index.is_multiple_of(2) {
                RoomSocketKind::StationA
            } else {
                RoomSocketKind::StationB
            };
            return self.team_station_target(player, wanted);
        }
        let cell = self.facility.config.exit();
        Some(HexObjectiveTarget {
            cell,
            position: self
                .geometry
                .sockets
                .iter()
                .find(|socket| socket.kind == RoomSocketKind::Exit)
                .map_or_else(
                    || Vec3::from_array(hex_origin(cell)),
                    |socket| socket.position,
                ),
            kind: RoomSocketKind::Exit,
        })
    }

    fn nearest_socket_target(
        &self,
        player: PlayerId,
        predicate: impl Fn(&crate::hex_wfc::HexRoomSocket) -> bool,
    ) -> Option<HexObjectiveTarget> {
        let from = self.players[&player].cell;
        self.geometry
            .sockets
            .iter()
            .filter(|socket| predicate(socket))
            // Target selection runs for every objective bot on every fixed tick. A full
            // A* for every candidate made the production objective loop perform dozens
            // of whole-facility searches per tick. The hard room/open-volume gates keep
            // objective rooms connected, so the admissible lattice distance is enough
            // to choose a stable candidate; bot movement then performs exactly one A*
            // to the chosen socket.
            .min_by_key(|socket| {
                (
                    travel_distance(from, socket.cell),
                    socket.room_generation_key,
                    &socket.id,
                )
            })
            .map(|socket| HexObjectiveTarget {
                cell: socket.cell,
                position: socket.position,
                kind: socket.kind,
            })
    }

    /// Pick one synchronization room for the whole team, then assign its A/B
    /// sockets by stable member order. Using the nearest socket independently
    /// can split teammates across repeated station rooms after they collect
    /// keystones on opposite sides of the facility.
    fn team_station_target(
        &self,
        player: PlayerId,
        wanted: RoomSocketKind,
    ) -> Option<HexObjectiveTarget> {
        let team = &self.teams[&self.players[&player].team];
        let mut rooms = self
            .geometry
            .sockets
            .iter()
            .filter(|socket| {
                matches!(
                    socket.kind,
                    RoomSocketKind::StationA | RoomSocketKind::StationB
                )
            })
            .fold(BTreeMap::<u64, Vec<_>>::new(), |mut rooms, socket| {
                rooms
                    .entry(socket.room_generation_key)
                    .or_default()
                    .push(socket);
                rooms
            });
        rooms.retain(|_, sockets| {
            sockets
                .iter()
                .any(|socket| socket.kind == RoomSocketKind::StationA)
                && sockets
                    .iter()
                    .any(|socket| socket.kind == RoomSocketKind::StationB)
        });
        let room = rooms
            .iter()
            .filter_map(|(&room, sockets)| {
                let cell = sockets.first()?.cell;
                let total_cost = team
                    .members
                    .iter()
                    .filter(|member| !self.players[member].escaped)
                    .map(|member| u64::from(travel_distance(self.players[member].cell, cell)))
                    .sum::<u64>();
                Some((total_cost, room))
            })
            .min()
            .map(|(_, room)| room)?;
        let socket = rooms[&room].iter().find(|socket| socket.kind == wanted)?;
        Some(HexObjectiveTarget {
            cell: socket.cell,
            position: socket.position,
            kind: socket.kind,
        })
    }

    pub fn bot_action_buttons(&self, player: PlayerId) -> HexActionButtons {
        self.bot_action_buttons_for_target(player, self.objective_target(player))
    }

    pub(super) fn bot_action_buttons_for_target(
        &self,
        player: PlayerId,
        target: Option<HexObjectiveTarget>,
    ) -> HexActionButtons {
        let Some(state) = self.players.get(&player) else {
            return HexActionButtons::default();
        };
        let Some(target) = target else {
            return HexActionButtons::default();
        };
        let interactive = matches!(
            target.kind,
            RoomSocketKind::Keystone | RoomSocketKind::StationA | RoomSocketKind::StationB
        );
        HexActionButtons {
            interact: interactive
                && state.cell == target.cell
                && state.position.distance(target.position) <= INTERACTION_RADIUS,
            ..HexActionButtons::default()
        }
    }

    pub(super) fn team_authorized_for_exit(&self, team: TeamId) -> bool {
        !self.objectives.enabled
            || (self.teams[&team].objectives.keystones >= self.objectives.keystones_required
                && self.teams[&team].objectives.dual_station_complete)
    }

    pub(super) fn incomplete_objective_cells(&self) -> BTreeSet<HexCoord> {
        if !self.objectives.enabled {
            return BTreeSet::new();
        }
        self.geometry
            .sockets
            .iter()
            .filter(|socket| match socket.kind {
                RoomSocketKind::Keystone => self
                    .objectives
                    .available_keystones
                    .contains(&socket.room_generation_key),
                RoomSocketKind::StationA | RoomSocketKind::StationB => self
                    .teams
                    .values()
                    .any(|team| !team.objectives.dual_station_complete),
                _ => false,
            })
            .map(|socket| socket.cell)
            .collect()
    }
}
