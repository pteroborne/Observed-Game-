use std::cmp::Reverse;

use glam::Vec3;
use observed_core::PlayerId;
use observed_facility::{full_wfc::CellCoord, map_spec::RoomRole};

use super::{
    FIRST_ESCAPE_COUNTDOWN_TICKS, FullWfcMatch, GameplayEvent, GameplayEventKind,
    INTERACTION_RADIUS, InputFrame, MatchStatus,
};
use crate::full_wfc::cell_origin;

impl FullWfcMatch {
    pub(super) fn try_collect_keystone(&mut self, id: PlayerId) {
        let (team, cell) = {
            let player = &self.players[&id];
            (player.team, player.cell)
        };
        let Some(room) = self.facility.room_at(cell) else {
            return;
        };
        let pickup = cell_origin(cell) + Vec3::Y * 1.15;
        if self.players[&id].position.distance(pickup) > INTERACTION_RADIUS {
            return;
        }
        if self.teams[&team].keystones >= self.config.keystones_required
            || !self.available_keystones.remove(&room)
        {
            return;
        }
        self.teams.get_mut(&team).expect("team").keystones += 1;
        self.push_player_event(GameplayEventKind::KeystoneCollected, id, cell, None);
    }

    pub(super) fn resolve_dual_stations(&mut self, frame: &InputFrame) {
        for team in self.teams.keys().copied().collect::<Vec<_>>() {
            let members = self.teams[&team].members;
            let cell = self.players[&members[0]].cell;
            let station = self
                .facility
                .room_at(cell)
                .is_some_and(|room| self.facility.rooms[&room].role == RoomRole::DualStation);
            let synchronized = station
                && members.iter().all(|member| {
                    self.players[member].cell == cell
                        && self.players[member].position.distance(cell_origin(cell)) <= 2.8
                        && frame
                            .commands
                            .get(member)
                            .is_some_and(|command| command.actions.interact)
                });
            if !synchronized {
                self.teams.get_mut(&team).expect("team").dual_station_ticks = 0;
                continue;
            }
            let state = self.teams.get_mut(&team).expect("team");
            if state.dual_station_complete {
                continue;
            }
            state.dual_station_ticks = state.dual_station_ticks.saturating_add(1);
            let ticks = state.dual_station_ticks;
            if ticks >= 120 {
                state.dual_station_complete = true;
                self.push_player_event(
                    GameplayEventKind::DualStationCompleted,
                    members[0],
                    cell,
                    None,
                );
            } else if ticks.is_multiple_of(30) {
                self.push_player_event(
                    GameplayEventKind::DualStationProgress,
                    members[0],
                    cell,
                    None,
                );
            }
        }
    }

    pub(super) fn try_survey_monitor(&mut self, id: PlayerId) {
        let player = &self.players[&id];
        let Some(room) = self.facility.room_at(player.cell) else {
            return;
        };
        if self.facility.rooms[&room].role != RoomRole::Monitor
            || !self.surveyed_monitors.insert((player.team, room))
        {
            return;
        }
        let team = player.team;
        let cell = player.cell;
        self.map_knowledge
            .get_mut(&team)
            .expect("team knowledge")
            .survey(&self.facility);
        self.push_player_event(GameplayEventKind::MonitorSurveyed, id, cell, None);
    }

    pub(super) fn try_redirect_guardian(&mut self, id: PlayerId) {
        let player = &self.players[&id];
        let Some(room) = self.facility.room_at(player.cell) else {
            return;
        };
        if self.facility.rooms[&room].role != RoomRole::GuardianControl {
            return;
        }
        let team = player.team;
        let target = self
            .teams
            .values()
            .filter(|candidate| candidate.id != team && !candidate.escaped && !candidate.eliminated)
            .max_by_key(|candidate| {
                (
                    candidate.keystones,
                    candidate.dual_station_complete,
                    Reverse(candidate.id),
                )
            })
            .map(|candidate| candidate.id);
        if target.is_none() || self.guardian.target_team == target {
            return;
        }
        let cell = player.cell;
        self.guardian.target_team = target;
        self.push_player_event(GameplayEventKind::GuardianRedirected, id, cell, None);
    }

    pub(super) fn resolve_team_escape(&mut self) {
        let exit = self.facility.exit();
        let mut escaped = Vec::new();
        for team in self.teams.values() {
            if team.escaped
                || team.eliminated
                || team.keystones < self.config.keystones_required
                || !team.dual_station_complete
            {
                continue;
            }
            if team.members.iter().all(|id| self.players[id].cell == exit) {
                escaped.push(team.id);
            }
        }
        for team in escaped {
            self.teams.get_mut(&team).expect("team").escaped = true;
            for id in self.teams[&team].members {
                self.players.get_mut(&id).expect("player").escaped = true;
            }
            self.escape_order.push(team);
            self.recent_events.push(GameplayEvent::new(
                self.tick,
                GameplayEventKind::TeamEscaped,
                None,
                Some(team),
                Some(exit),
                None,
            ));
            if self.status == MatchStatus::Running {
                self.status = MatchStatus::Countdown {
                    remaining_ticks: FIRST_ESCAPE_COUNTDOWN_TICKS,
                };
            }
        }
    }

    pub(super) fn step_countdown(&mut self) {
        let MatchStatus::Countdown { remaining_ticks } = self.status else {
            return;
        };
        if remaining_ticks > 1 {
            self.status = MatchStatus::Countdown {
                remaining_ticks: remaining_ticks - 1,
            };
            return;
        }
        for team in self.teams.values_mut().filter(|team| !team.escaped) {
            team.eliminated = true;
        }
        self.status = MatchStatus::Finished;
        self.recent_events.push(GameplayEvent::new(
            self.tick,
            GameplayEventKind::MatchFinished,
            None,
            None,
            None,
            None,
        ));
    }

    pub(super) fn push_player_event(
        &mut self,
        kind: GameplayEventKind,
        player: PlayerId,
        source: CellCoord,
        destination: Option<CellCoord>,
    ) {
        self.recent_events.push(GameplayEvent::new(
            self.tick,
            kind,
            Some(player),
            Some(self.players[&player].team),
            Some(source),
            destination,
        ));
    }

    pub(super) fn update_map_knowledge(&mut self) {
        for (team, knowledge) in &mut self.map_knowledge {
            knowledge.observe(
                *team,
                &self.facility,
                self.players.values().cloned(),
                &self.equipment,
            );
        }
    }
}
