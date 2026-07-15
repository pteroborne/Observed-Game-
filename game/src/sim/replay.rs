//! Passive tactical replay recording for the assembled game.
//!
//! The recorder samples already-owned simulation state during Match and stores a compact
//! room/place trace. Replay presentation reads this tape after the match; it never drives
//! or mutates gameplay.

use std::collections::BTreeSet;

use bevy::prelude::*;
use observed_core::{RoomId, TeamId};
use observed_facility::map_spec::{MapSpec, RoomRole};
use observed_match::facility::{TEAM_COUNT, TeamRun};
use observed_match::teamplay::{TeamMemberState, TeamTask};

use crate::flow::{ActiveMatchSeed, LOCAL_TEAM, MatchResult};
use crate::items::{ItemKind, ItemsState};
use crate::keystones::KeystoneState;
use crate::sim::director::MatchDirector;
use crate::sim::state::{SpectatorBot, TeleportState};
use crate::teleport::Place;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReplayActorId {
    LocalPlayer,
    Team(TeamId),
    Member { team: TeamId, member: u8 },
}

impl ReplayActorId {
    pub fn label(self) -> String {
        match self {
            Self::LocalPlayer => "You".to_string(),
            Self::Team(team) => team.label(),
            Self::Member { team, member } => format!("{} P{}", team.label(), member + 1),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayActor {
    pub id: ReplayActorId,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayRoom {
    pub id: RoomId,
    pub schematic: Vec2,
    pub role: RoomRole,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayActorPose {
    pub actor: ReplayActorId,
    pub room: Option<RoomId>,
    pub place: Option<Place>,
    pub status: String,
    pub task: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplaySample {
    pub index: usize,
    pub live_round: u32,
    pub series_round: u32,
    pub actors: Vec<ReplayActorPose>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayMarker {
    pub sample: usize,
    pub live_round: u32,
    pub series_round: u32,
    pub label: String,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct ReplayTape {
    pub seed: u64,
    pub input_version: u16,
    pub map_name: String,
    pub simulation_content_hash: [u8; 32],
    pub presentation_content_hash: [u8; 32],
    pub rooms: Vec<ReplayRoom>,
    pub actors: Vec<ReplayActor>,
    pub samples: Vec<ReplaySample>,
    pub markers: Vec<ReplayMarker>,
    pub result: Option<MatchResult>,
    /// Presentation-side story facts retained after Match-scoped resources are
    /// cleaned up. These are sampled from existing state and never drive gameplay.
    pub visited_rooms: Vec<RoomId>,
    pub collapsed_rooms: Vec<RoomId>,
    pub escape_order: Vec<TeamId>,
    pub keystones_collected: u32,
    pub keystones_required: u32,
    pub anchor_uses: usize,
    seen_actors: BTreeSet<ReplayActorId>,
    seen_markers: BTreeSet<String>,
    anchor_was_placed: bool,
}

impl ReplayTape {
    pub fn new(seed: u64, map_spec: &MapSpec) -> Self {
        Self::new_with_content(seed, map_spec, [0; 32], [0; 32])
    }

    pub fn new_with_content(
        seed: u64,
        map_spec: &MapSpec,
        simulation_content_hash: [u8; 32],
        presentation_content_hash: [u8; 32],
    ) -> Self {
        let mut tape = Self {
            seed,
            input_version: 0,
            map_name: map_spec.name.to_string(),
            simulation_content_hash,
            presentation_content_hash,
            rooms: map_spec
                .rooms
                .iter()
                .map(|room| ReplayRoom {
                    id: room.id,
                    schematic: room.schematic,
                    role: room.role,
                })
                .collect(),
            actors: Vec::new(),
            samples: Vec::new(),
            markers: Vec::new(),
            result: None,
            visited_rooms: Vec::new(),
            collapsed_rooms: Vec::new(),
            escape_order: Vec::new(),
            keystones_collected: 0,
            keystones_required: 0,
            anchor_uses: 0,
            seen_actors: BTreeSet::new(),
            seen_markers: BTreeSet::new(),
            anchor_was_placed: false,
        };
        tape.ensure_actor(ReplayActorId::LocalPlayer);
        for team in 0..TEAM_COUNT as u8 {
            tape.ensure_actor(ReplayActorId::Team(TeamId(team)));
        }
        tape
    }

    pub fn new_full_wfc(game: &observed_match::full_wfc::FullWfcMatch) -> Self {
        let mut tape = Self {
            seed: game.seed,
            input_version: observed_match::full_wfc::FULL_WFC_INPUT_VERSION,
            map_name: "full_wfc_v1".to_string(),
            simulation_content_hash: [0; 32],
            presentation_content_hash: [0; 32],
            rooms: Vec::new(),
            actors: Vec::new(),
            samples: Vec::new(),
            markers: Vec::new(),
            result: None,
            visited_rooms: Vec::new(),
            collapsed_rooms: Vec::new(),
            escape_order: Vec::new(),
            keystones_collected: 0,
            keystones_required: 2,
            anchor_uses: 0,
            seen_actors: BTreeSet::new(),
            seen_markers: BTreeSet::new(),
            anchor_was_placed: false,
        };
        tape.ensure_actor(ReplayActorId::LocalPlayer);
        for player in game.players.values().filter(|player| player.id.0 != 0) {
            tape.ensure_actor(ReplayActorId::Member {
                team: player.team,
                member: (player.id.0 % 2) as u8,
            });
        }
        tape.sync_full_wfc_rooms(game);
        tape
    }

    pub fn record_full_wfc(&mut self, game: &observed_match::full_wfc::FullWfcMatch) {
        if self.seed != game.seed {
            return;
        }
        self.sync_full_wfc_rooms(game);
        let local = &game.players[&observed_core::PlayerId(0)];
        if let Some(room) = game.facility.room_at(local.cell)
            && !self.visited_rooms.contains(&room)
        {
            self.visited_rooms.push(room);
        }
        let local_team = &game.teams[&local.team];
        self.keystones_collected = self
            .keystones_collected
            .max(u32::from(local_team.keystones));
        self.escape_order.clone_from(&game.escape_order);
        let anchor_is_placed = game.equipment.deployed.values().any(|item| {
            item.team == local.team && item.kind == observed_match::full_wfc::DeployableKind::Anchor
        });
        if anchor_is_placed && !self.anchor_was_placed {
            self.anchor_uses += 1;
        }
        self.anchor_was_placed = anchor_is_placed;

        if game.tick.is_multiple_of(6) || self.samples.is_empty() {
            let actors = game
                .players
                .values()
                .map(|player| {
                    let team = &game.teams[&player.team];
                    ReplayActorPose {
                        actor: if player.id.0 == 0 {
                            ReplayActorId::LocalPlayer
                        } else {
                            ReplayActorId::Member {
                                team: player.team,
                                member: (player.id.0 % 2) as u8,
                            }
                        },
                        room: game.facility.room_at(player.cell),
                        place: None,
                        status: if player.escaped {
                            "escaped".to_string()
                        } else if team.eliminated {
                            "eliminated".to_string()
                        } else {
                            "active".to_string()
                        },
                        task: if team.keystones < 2 {
                            format!("keystones {}/2", team.keystones)
                        } else if !team.dual_station_complete {
                            "dual station".to_string()
                        } else {
                            "reach exit".to_string()
                        },
                    }
                })
                .collect();
            self.push_sample(
                game.tick.min(u64::from(u32::MAX)) as u32,
                game.facility.generation,
                actors,
            );
        }
        for event in &game.recent_events {
            self.push_marker(
                game.tick.min(u64::from(u32::MAX)) as u32,
                game.facility.generation,
                format!("t{} {:?}", game.tick, event.kind),
            );
        }
    }

    fn sync_full_wfc_rooms(&mut self, game: &observed_match::full_wfc::FullWfcMatch) {
        let current = game.facility.rooms.keys().copied().collect::<BTreeSet<_>>();
        self.collapsed_rooms = self
            .rooms
            .iter()
            .filter_map(|room| (!current.contains(&room.id)).then_some(room.id))
            .collect();
        for room in game.facility.rooms.values() {
            if self.rooms.iter().any(|existing| existing.id == room.id) {
                continue;
            }
            self.rooms.push(ReplayRoom {
                id: room.id,
                schematic: Vec2::new(
                    f32::from(room.coord.x)
                        + f32::from(room.coord.level)
                            * (f32::from(game.facility.config.cols) + 1.0),
                    f32::from(room.coord.z),
                ),
                role: room.role,
            });
        }
    }

    pub fn ensure_actor(&mut self, id: ReplayActorId) {
        if self.seen_actors.insert(id) {
            self.actors.push(ReplayActor {
                id,
                label: id.label(),
            });
        }
    }

    pub fn push_sample(
        &mut self,
        live_round: u32,
        series_round: u32,
        actors: Vec<ReplayActorPose>,
    ) {
        for pose in &actors {
            self.ensure_actor(pose.actor);
        }
        self.samples.push(ReplaySample {
            index: self.samples.len(),
            live_round,
            series_round,
            actors,
        });
    }

    pub fn push_marker(&mut self, live_round: u32, series_round: u32, label: impl Into<String>) {
        let label = label.into();
        if label.trim().is_empty() || !self.seen_markers.insert(label.clone()) {
            return;
        }
        self.markers.push(ReplayMarker {
            sample: self.samples.len().saturating_sub(1),
            live_round,
            series_round,
            label,
        });
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn sample_at(&self, cursor: usize) -> Option<&ReplaySample> {
        if self.samples.is_empty() {
            None
        } else {
            self.samples.get(cursor.min(self.samples.len() - 1))
        }
    }

    pub fn default_focus(&self) -> ReplayActorId {
        self.result
            .as_ref()
            .and_then(|result| result.winner)
            .map(ReplayActorId::Team)
            .unwrap_or(ReplayActorId::LocalPlayer)
    }

    pub fn focus_index(&self, focus: ReplayActorId) -> usize {
        self.actors
            .iter()
            .position(|actor| actor.id == focus)
            .unwrap_or(0)
    }

    fn record_story_snapshot(
        &mut self,
        game: &observed_match::hybrid::HybridMatch,
        place: Place,
        keys: &KeystoneState,
        items: &ItemsState,
    ) {
        if let Place::Room(room) = place
            && !self.visited_rooms.contains(&room)
        {
            self.visited_rooms.push(room);
        }

        self.collapsed_rooms = game.competitive.collapse_rooms();
        self.keystones_collected = self.keystones_collected.max(keys.held);
        self.keystones_required = keys.required;

        let anchor_is_placed = items
            .placed
            .iter()
            .any(|item| item.kind == ItemKind::AnchorTorch && item.team == LOCAL_TEAM);
        if anchor_is_placed && !self.anchor_was_placed {
            self.anchor_uses += 1;
        }
        self.anchor_was_placed = anchor_is_placed;

        let mut placed: Vec<(u8, TeamId)> = game
            .competitive
            .teams
            .iter()
            .filter_map(|team| team.placement.map(|placement| (placement, team.id)))
            .collect();
        placed.sort_by_key(|(placement, team)| (*placement, team.0));
        for (_, team) in placed {
            if !self.escape_order.contains(&team) {
                self.escape_order.push(team);
            }
        }
    }
}

pub(crate) fn record_match_replay_sample(
    mut tape: ResMut<ReplayTape>,
    director: Res<MatchDirector>,
    tp: Res<TeleportState>,
    keys: Res<KeystoneState>,
    items: Res<ItemsState>,
    spectator: Option<Res<SpectatorBot>>,
    seed: Option<Res<ActiveMatchSeed>>,
) {
    if let Some(seed) = seed
        && tape.seed != seed.0
    {
        return;
    }

    let game = director.live.host_match();
    tape.record_story_snapshot(game, tp.place, &keys, &items);
    let live_round = game.competitive.round;
    let series_round = director.series.current.index;
    let mut actors = Vec::new();

    actors.push(ReplayActorPose {
        actor: ReplayActorId::LocalPlayer,
        room: match tp.place {
            Place::Room(room) => Some(room),
            Place::Hallway { .. } => None,
        },
        place: Some(tp.place),
        status: local_status(game.competitive.team(LOCAL_TEAM)),
        task: if keys.gate_open() {
            "reach exit".to_string()
        } else {
            format!("keystones {}/{}", keys.held, keys.required)
        },
    });

    for (index, team) in game.competitive.teams.iter().enumerate() {
        actors.push(ReplayActorPose {
            actor: ReplayActorId::Team(team.id),
            room: Some(game.competitive.team_room(index)),
            place: None,
            status: team_status(team, game.competitive.team_progress(index)),
            task: if team.active_runner() {
                "race route".to_string()
            } else {
                team.status()
            },
        });
    }

    if let Some(spectator) = spectator.as_ref() {
        for team in &spectator.teamplay.teams {
            for member in &team.members {
                actors.push(member_pose(member, team.id));
            }
        }
    }

    tape.push_sample(live_round, series_round, actors);
    tape.push_marker(
        live_round,
        series_round,
        format!("live: {}", game.last_event),
    );
    tape.push_marker(
        live_round,
        series_round,
        format!("series: {}", director.series.last_event),
    );
    if let Some(spectator) = spectator.as_ref() {
        tape.push_marker(
            live_round,
            series_round,
            format!("bot: {}", spectator.last_teamplay_event),
        );
    }
    if director.done || director.finished() {
        tape.result = Some(director.outcome());
    }
}

fn local_status(team: Option<&TeamRun>) -> String {
    team.map(|team| team_status(team, 0.0))
        .unwrap_or_else(|| "unknown".to_string())
}

fn team_status(team: &TeamRun, progress: f32) -> String {
    match (team.placement, team.active_runner()) {
        (Some(place), _) => format!("escaped {}", ordinal(place)),
        (None, true) => format!("{:.0}% route", progress * 100.0),
        (None, false) => team.status(),
    }
}

fn member_pose(member: &TeamMemberState, team: TeamId) -> ReplayActorPose {
    ReplayActorPose {
        actor: ReplayActorId::Member {
            team,
            member: member.id.member,
        },
        room: Some(member.room),
        place: None,
        status: if member.escaped {
            "escaped".to_string()
        } else if member.delayed_ticks > 0 {
            "delayed".to_string()
        } else {
            "active".to_string()
        },
        task: task_label(member.task).to_string(),
    }
}

fn task_label(task: TeamTask) -> &'static str {
    match task {
        TeamTask::Regroup(_) => "regroup",
        TeamTask::OperateStationA(_) => "station A",
        TeamTask::OperateStationB(_) => "station B",
        TeamTask::PlaceAnchor(_) => "anchor",
        TeamTask::PlaceTeleportPad(_) => "place pad",
        TeamTask::CollectKeystone(_) => "keystone",
        TeamTask::Escape(_) => "escape",
        TeamTask::EvadeGuardian(_) => "evade guardian",
    }
}

fn ordinal(place: u8) -> String {
    match place {
        1 => "1st".to_string(),
        2 => "2nd".to_string(),
        3 => "3rd".to_string(),
        n => format!("{n}th"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_catalog::default_map_spec;

    #[test]
    fn tape_clamps_samples_and_tracks_actors_once() {
        let spec = default_map_spec(1);
        let mut tape = ReplayTape::new(1, &spec);
        tape.push_sample(
            0,
            1,
            vec![
                ReplayActorPose {
                    actor: ReplayActorId::LocalPlayer,
                    room: Some(RoomId(0)),
                    place: Some(Place::Room(RoomId(0))),
                    status: "active".to_string(),
                    task: "test".to_string(),
                },
                ReplayActorPose {
                    actor: ReplayActorId::LocalPlayer,
                    room: Some(RoomId(0)),
                    place: Some(Place::Room(RoomId(0))),
                    status: "active".to_string(),
                    task: "test".to_string(),
                },
            ],
        );

        assert_eq!(tape.sample_at(99).unwrap().index, 0);
        assert_eq!(
            tape.actors
                .iter()
                .filter(|actor| actor.id == ReplayActorId::LocalPlayer)
                .count(),
            1
        );
    }

    #[test]
    fn marker_labels_are_deduplicated_in_order() {
        let spec = default_map_spec(1);
        let mut tape = ReplayTape::new(1, &spec);
        tape.push_sample(0, 1, Vec::new());
        tape.push_marker(0, 1, "same");
        tape.push_marker(1, 1, "same");
        tape.push_marker(2, 1, "next");

        assert_eq!(tape.markers.len(), 2);
        assert_eq!(tape.markers[0].label, "same");
        assert_eq!(tape.markers[1].label, "next");
        assert!(tape.markers[0].sample <= tape.markers[1].sample);
    }
}
