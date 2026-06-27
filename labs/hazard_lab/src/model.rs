//! Pure-logic feasibility model for Phase 15: a shared environmental hazard
//! that requires coordinated player roles, can be steered by the facility
//! director, and interferes by delaying route progress rather than damaging
//! players or removing progress they already earned.

use std::collections::BTreeMap;

use bevy::prelude::*;
use observed_core::{PlayerId, TeamId};

pub const PLAYER_COUNT: usize = 4;
pub const TEAM_COUNT: usize = 2;
pub const ZONE_COUNT: usize = 3;
pub const ROUTE_LENGTH: u8 = 12;
const STEPS_PER_ZONE: u8 = ROUTE_LENGTH / ZONE_COUNT as u8;
const MAX_PRESSURE: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HazardZoneId(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardZone {
    pub id: HazardZoneId,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardPlayer {
    pub id: PlayerId,
    pub team: TeamId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardTeam {
    pub id: TeamId,
    pub progress: u8,
    pub delay_rounds: u32,
    pub completed_round: Option<u32>,
}

impl HazardTeam {
    pub fn active(self) -> bool {
        self.completed_round.is_none()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlayerHazardIntent {
    Advance,
    VentA,
    VentB,
    #[default]
    Wait,
}

impl PlayerHazardIntent {
    pub fn label(self) -> &'static str {
        match self {
            Self::Advance => "ADVANCE",
            Self::VentA => "VENT A",
            Self::VentB => "VENT B",
            Self::Wait => "WAIT",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DirectorHazardAction {
    #[default]
    Hold,
    Steer(HazardZoneId),
}

impl DirectorHazardAction {
    pub fn label(self, zones: &[HazardZone]) -> String {
        match self {
            Self::Hold => "HOLD".to_string(),
            Self::Steer(id) => zones
                .iter()
                .find(|zone| zone.id == id)
                .map(|zone| format!("STEER {}", zone.name))
                .unwrap_or_else(|| format!("STEER Z{}", id.0 + 1)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PressureHazard {
    pub zone: HazardZoneId,
    pub pressure: u8,
    pub contained_last_round: bool,
    pub pulse_count: u32,
    pub steer_count: u32,
}

#[derive(Resource, Clone, Debug, Eq, PartialEq)]
pub struct HazardWorld {
    pub zones: Vec<HazardZone>,
    pub players: Vec<HazardPlayer>,
    pub teams: Vec<HazardTeam>,
    pub hazard: PressureHazard,
    pub round: u32,
    pub vent_cycles: u32,
    pub last_vent_a: Vec<PlayerId>,
    pub last_vent_b: Vec<PlayerId>,
    pub last_delayed_teams: Vec<TeamId>,
    pub last_director_action: DirectorHazardAction,
    pub last_event: String,
}

impl HazardWorld {
    pub fn authored() -> Self {
        let zones = vec![
            HazardZone {
                id: HazardZoneId(0),
                name: "INTAKE",
            },
            HazardZone {
                id: HazardZoneId(1),
                name: "CORE",
            },
            HazardZone {
                id: HazardZoneId(2),
                name: "SPINE",
            },
        ];
        let players = vec![
            HazardPlayer {
                id: PlayerId(0),
                team: TeamId(0),
            },
            HazardPlayer {
                id: PlayerId(1),
                team: TeamId(0),
            },
            HazardPlayer {
                id: PlayerId(2),
                team: TeamId(1),
            },
            HazardPlayer {
                id: PlayerId(3),
                team: TeamId(1),
            },
        ];
        let teams = vec![
            HazardTeam {
                id: TeamId(0),
                progress: 0,
                delay_rounds: 0,
                completed_round: None,
            },
            HazardTeam {
                id: TeamId(1),
                progress: 0,
                delay_rounds: 0,
                completed_round: None,
            },
        ];

        Self {
            zones,
            players,
            teams,
            hazard: PressureHazard {
                zone: HazardZoneId(0),
                pressure: 1,
                contained_last_round: false,
                pulse_count: 0,
                steer_count: 0,
            },
            round: 0,
            vent_cycles: 0,
            last_vent_a: Vec::new(),
            last_vent_b: Vec::new(),
            last_delayed_teams: Vec::new(),
            last_director_action: DirectorHazardAction::Hold,
            last_event: "Pressure rising in INTAKE. Staff both relief valves.".to_string(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    pub fn player(&self, id: PlayerId) -> Option<&HazardPlayer> {
        self.players.iter().find(|player| player.id == id)
    }

    pub fn team(&self, id: TeamId) -> Option<&HazardTeam> {
        self.teams.iter().find(|team| team.id == id)
    }

    pub fn zone(&self, id: HazardZoneId) -> Option<&HazardZone> {
        self.zones.iter().find(|zone| zone.id == id)
    }

    pub fn team_zone(&self, team: TeamId) -> Option<HazardZoneId> {
        self.team(team).map(|team| {
            HazardZoneId((team.progress / STEPS_PER_ZONE).min(ZONE_COUNT.saturating_sub(1) as u8))
        })
    }

    pub fn complete(&self) -> bool {
        self.teams.iter().all(|team| !team.active())
    }

    /// Resolve one deterministic hazard round.
    ///
    /// Both relief roles must be staffed by distinct players who are physically
    /// in the active hazard zone. The operators may belong to different teams:
    /// containment is shared by everyone in the zone. If containment fails,
    /// advancing teams in that zone are delayed for the round, but their route
    /// progress is never reduced.
    pub fn resolve_round(
        &mut self,
        intents: &[(PlayerId, PlayerHazardIntent)],
        director_action: DirectorHazardAction,
    ) {
        if self.complete() {
            return;
        }

        self.round += 1;
        self.last_director_action = director_action;
        self.apply_director_action(director_action);

        let intents: BTreeMap<PlayerId, PlayerHazardIntent> = intents.iter().copied().collect();
        self.last_vent_a.clear();
        self.last_vent_b.clear();
        self.last_delayed_teams.clear();

        for player in &self.players {
            let Some(team) = self.team(player.team) else {
                continue;
            };
            if !team.active() || self.team_zone(player.team) != Some(self.hazard.zone) {
                continue;
            }
            match intents.get(&player.id).copied().unwrap_or_default() {
                PlayerHazardIntent::VentA => self.last_vent_a.push(player.id),
                PlayerHazardIntent::VentB => self.last_vent_b.push(player.id),
                PlayerHazardIntent::Advance | PlayerHazardIntent::Wait => {}
            }
        }

        let contained = !self.last_vent_a.is_empty() && !self.last_vent_b.is_empty();
        self.hazard.contained_last_round = contained;
        if contained {
            self.hazard.pressure = self.hazard.pressure.saturating_sub(1).max(1);
            self.vent_cycles += 1;
        } else {
            self.hazard.pressure = (self.hazard.pressure + 1).min(MAX_PRESSURE);
            self.hazard.pulse_count += 1;
        }

        let team_zones: BTreeMap<TeamId, HazardZoneId> = self
            .teams
            .iter()
            .map(|team| {
                (
                    team.id,
                    HazardZoneId(
                        (team.progress / STEPS_PER_ZONE).min(ZONE_COUNT.saturating_sub(1) as u8),
                    ),
                )
            })
            .collect();

        for team in &mut self.teams {
            if !team.active() {
                continue;
            }
            let advancing = self
                .players
                .iter()
                .filter(|player| player.team == team.id)
                .filter(|player| {
                    intents.get(&player.id).copied() == Some(PlayerHazardIntent::Advance)
                })
                .count() as u8;

            let in_uncontained_hazard =
                team_zones.get(&team.id) == Some(&self.hazard.zone) && !contained;
            if in_uncontained_hazard && advancing > 0 {
                team.delay_rounds += 1;
                self.last_delayed_teams.push(team.id);
                continue;
            }

            team.progress = (team.progress + advancing).min(ROUTE_LENGTH);
            if team.progress == ROUTE_LENGTH {
                team.completed_round = Some(self.round);
            }
        }

        let hazard_name = self
            .zone(self.hazard.zone)
            .map(|zone| zone.name)
            .unwrap_or("UNKNOWN");
        self.last_event = if contained {
            format!(
                "{hazard_name} pressure vented by {} + {}; shared route remains open.",
                player_list(&self.last_vent_a),
                player_list(&self.last_vent_b)
            )
        } else if self.last_delayed_teams.is_empty() {
            format!(
                "{hazard_name} pulsed at pressure {}; nobody advancing there was caught.",
                self.hazard.pressure
            )
        } else {
            format!(
                "{hazard_name} pulsed at pressure {}; {} delayed, no progress removed.",
                self.hazard.pressure,
                team_list(&self.last_delayed_teams)
            )
        };
    }

    fn apply_director_action(&mut self, action: DirectorHazardAction) {
        let DirectorHazardAction::Steer(zone) = action else {
            return;
        };
        if self.zone(zone).is_some() && self.hazard.zone != zone {
            self.hazard.zone = zone;
            self.hazard.steer_count += 1;
            self.hazard.contained_last_round = false;
        }
    }
}

fn player_list(players: &[PlayerId]) -> String {
    players
        .iter()
        .map(|player| player.label())
        .collect::<Vec<_>>()
        .join(", ")
}

fn team_list(teams: &[TeamId]) -> String {
    teams
        .iter()
        .map(|team| team.label())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_advance() -> Vec<(PlayerId, PlayerHazardIntent)> {
        (0..PLAYER_COUNT)
            .map(|id| (PlayerId(id as u16), PlayerHazardIntent::Advance))
            .collect()
    }

    #[test]
    fn authored_world_has_four_players_two_teams_and_three_zones() {
        let world = HazardWorld::authored();
        assert_eq!(world.players.len(), PLAYER_COUNT);
        assert_eq!(world.teams.len(), TEAM_COUNT);
        assert_eq!(world.zones.len(), ZONE_COUNT);
        assert_eq!(world.hazard.zone, HazardZoneId(0));
        assert_eq!(world.hazard.pressure, 1);
    }

    #[test]
    fn one_operator_cannot_contain_the_hazard() {
        let mut world = HazardWorld::authored();
        world.resolve_round(
            &[
                (PlayerId(0), PlayerHazardIntent::VentA),
                (PlayerId(2), PlayerHazardIntent::Advance),
                (PlayerId(3), PlayerHazardIntent::Advance),
            ],
            DirectorHazardAction::Hold,
        );

        assert!(!world.hazard.contained_last_round);
        assert_eq!(world.vent_cycles, 0);
        assert_eq!(world.team(TeamId(1)).unwrap().progress, 0);
        assert_eq!(world.team(TeamId(1)).unwrap().delay_rounds, 1);
    }

    #[test]
    fn both_valves_contain_the_hazard_and_open_the_shared_route() {
        let mut world = HazardWorld::authored();
        world.resolve_round(
            &[
                (PlayerId(0), PlayerHazardIntent::VentA),
                (PlayerId(1), PlayerHazardIntent::VentB),
                (PlayerId(2), PlayerHazardIntent::Advance),
                (PlayerId(3), PlayerHazardIntent::Advance),
            ],
            DirectorHazardAction::Hold,
        );

        assert!(world.hazard.contained_last_round);
        assert_eq!(world.vent_cycles, 1);
        assert_eq!(world.team(TeamId(0)).unwrap().progress, 0);
        assert_eq!(world.team(TeamId(1)).unwrap().progress, 2);
        assert!(world.last_delayed_teams.is_empty());
    }

    #[test]
    fn players_from_different_teams_can_coordinate_the_two_valves() {
        let mut world = HazardWorld::authored();
        world.resolve_round(
            &[
                (PlayerId(0), PlayerHazardIntent::VentA),
                (PlayerId(2), PlayerHazardIntent::VentB),
                (PlayerId(1), PlayerHazardIntent::Advance),
                (PlayerId(3), PlayerHazardIntent::Advance),
            ],
            DirectorHazardAction::Hold,
        );

        assert!(world.hazard.contained_last_round);
        assert_eq!(world.team(TeamId(0)).unwrap().progress, 1);
        assert_eq!(world.team(TeamId(1)).unwrap().progress, 1);
    }

    #[test]
    fn the_director_can_steer_the_hazard_to_delay_a_different_team() {
        let mut world = HazardWorld::authored();
        world.teams[1].progress = STEPS_PER_ZONE;

        world.resolve_round(&all_advance(), DirectorHazardAction::Steer(HazardZoneId(1)));

        assert_eq!(world.hazard.zone, HazardZoneId(1));
        assert_eq!(world.hazard.steer_count, 1);
        assert_eq!(world.team(TeamId(0)).unwrap().progress, 2);
        assert_eq!(world.team(TeamId(1)).unwrap().progress, STEPS_PER_ZONE);
        assert_eq!(world.team(TeamId(1)).unwrap().delay_rounds, 1);
    }

    #[test]
    fn hazard_interference_never_removes_earned_progress() {
        let mut world = HazardWorld::authored();
        let mut previous = world
            .teams
            .iter()
            .map(|team| team.progress)
            .collect::<Vec<_>>();

        for round in 0..20 {
            let zone = HazardZoneId((round % ZONE_COUNT) as u8);
            world.resolve_round(&all_advance(), DirectorHazardAction::Steer(zone));
            for (team, prior) in world.teams.iter().zip(previous.iter()) {
                assert!(
                    team.progress >= *prior,
                    "hazards may stall progress but never remove it"
                );
            }
            previous = world.teams.iter().map(|team| team.progress).collect();
            if world.complete() {
                break;
            }
        }
    }

    #[test]
    fn resolution_is_deterministic_and_input_order_independent() {
        let ordered = vec![
            (PlayerId(0), PlayerHazardIntent::VentA),
            (PlayerId(1), PlayerHazardIntent::Advance),
            (PlayerId(2), PlayerHazardIntent::VentB),
            (PlayerId(3), PlayerHazardIntent::Advance),
        ];
        let mut reversed = ordered.clone();
        reversed.reverse();

        let mut a = HazardWorld::authored();
        let mut b = HazardWorld::authored();
        a.resolve_round(&ordered, DirectorHazardAction::Hold);
        b.resolve_round(&reversed, DirectorHazardAction::Hold);

        assert_eq!(a, b);
    }

    #[test]
    fn a_team_completes_without_being_modified_by_later_rounds() {
        let mut world = HazardWorld::authored();
        world.teams[0].progress = ROUTE_LENGTH - 2;
        world.hazard.zone = HazardZoneId(1);
        world.resolve_round(&all_advance(), DirectorHazardAction::Hold);
        let completed = *world.team(TeamId(0)).unwrap();
        assert_eq!(completed.progress, ROUTE_LENGTH);
        assert!(completed.completed_round.is_some());

        world.resolve_round(&all_advance(), DirectorHazardAction::Hold);
        assert_eq!(*world.team(TeamId(0)).unwrap(), completed);
    }

    #[test]
    fn reset_restores_the_authored_pressure_front() {
        let mut world = HazardWorld::authored();
        world.resolve_round(&all_advance(), DirectorHazardAction::Steer(HazardZoneId(2)));
        world.reset();
        assert_eq!(world, HazardWorld::authored());
    }
}
