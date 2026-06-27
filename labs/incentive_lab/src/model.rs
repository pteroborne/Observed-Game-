//! Pure-logic feasibility model for **splitting & backtracking incentives** —
//! a scoring scheme whose dominant strategy is to *divide* the team across rooms
//! and *revisit* rooms, never to funnel everyone down one path.
//!
//! Each room holds a `charge` that a present team harvests for score and that
//! regenerates only while the room is empty. Two rules make spreading and
//! backtracking pay:
//!   * **Dispersion** — a team's harvest is multiplied by how many distinct rooms
//!     its members occupy, so splitting up both covers more rooms *and* scores
//!     each one harder; clumping wastes members.
//!   * **Regrowth** — a camped room drains to nothing, while one you left behind
//!     regrows, so cycling back to a regrown room out-scores standing still.
//!
//! There is no exit to rush — score is coverage over time — so no single path
//! dominates.

use bevy::prelude::*;
use observed_core::{PlayerId, RoomId, TeamId};

pub const ROOM_COUNT: usize = 6;
pub const TEAM_COUNT: usize = 2;
pub const MEMBERS_PER_TEAM: usize = 3;
pub const TOTAL_MEMBERS: usize = TEAM_COUNT * MEMBERS_PER_TEAM;

const HARVEST_RATE: f32 = 0.25;
const REGEN_RATE: f32 = 0.12;
const SPREAD_BONUS: f32 = 0.5;
const MAX_CHARGE: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub id: RoomId,
    pub charge: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Member {
    pub id: PlayerId,
    pub team: TeamId,
    pub room: RoomId,
    pub spawn_room: RoomId,
}

#[derive(Resource, Clone, Debug)]
pub struct IncentiveWorld {
    pub rooms: Vec<Room>,
    pub members: Vec<Member>,
    pub teams: Vec<TeamId>,
    pub scores: Vec<f32>,
    pub tick_count: u32,
    pub last_event: String,
}

impl IncentiveWorld {
    pub fn authored() -> Self {
        let rooms = (0..ROOM_COUNT)
            .map(|i| Room {
                id: RoomId(i as u32),
                charge: MAX_CHARGE,
            })
            .collect();
        // Team A starts spread across rooms 0-2; team B starts clumped in room 5.
        let starts = [0u32, 1, 2, 5, 5, 5];
        let members = (0..TOTAL_MEMBERS)
            .map(|i| Member {
                id: PlayerId(i as u16),
                team: TeamId((i / MEMBERS_PER_TEAM) as u8),
                room: RoomId(starts[i]),
                spawn_room: RoomId(starts[i]),
            })
            .collect();
        Self {
            rooms,
            members,
            teams: (0..TEAM_COUNT).map(|i| TeamId(i as u8)).collect(),
            scores: vec![0.0; TEAM_COUNT],
            tick_count: 0,
            last_event: "Spread out and revisit regrown rooms — clumping wastes members."
                .to_string(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    fn team_index(&self, team: TeamId) -> Option<usize> {
        self.teams.iter().position(|t| *t == team)
    }

    pub fn score_of(&self, team: TeamId) -> f32 {
        self.team_index(team).map(|i| self.scores[i]).unwrap_or(0.0)
    }

    pub fn member(&self, player: PlayerId) -> Option<&Member> {
        self.members.iter().find(|m| m.id == player)
    }

    pub fn room(&self, id: RoomId) -> Option<&Room> {
        self.rooms.iter().find(|r| r.id == id)
    }

    /// How many distinct rooms a team currently occupies.
    pub fn coverage(&self, team: TeamId) -> usize {
        let mut rooms: Vec<RoomId> = self
            .members
            .iter()
            .filter(|m| m.team == team)
            .map(|m| m.room)
            .collect();
        rooms.sort_by_key(|r| r.0);
        rooms.dedup();
        rooms.len()
    }

    /// Harvest multiplier from spreading: 1 + (distinct rooms − 1) × bonus.
    pub fn dispersion(&self, team: TeamId) -> f32 {
        1.0 + (self.coverage(team).max(1) as f32 - 1.0) * SPREAD_BONUS
    }

    pub fn move_member(&mut self, player: PlayerId, room: RoomId) -> bool {
        if room.0 as usize >= self.rooms.len() {
            return false;
        }
        if let Some(member) = self.members.iter_mut().find(|m| m.id == player) {
            member.room = room;
            true
        } else {
            false
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.tick_count += 1;
        let dispersion: Vec<f32> = self.teams.iter().map(|t| self.dispersion(*t)).collect();

        for index in 0..self.rooms.len() {
            let room_id = self.rooms[index].id;
            let mut teams_here: Vec<usize> = Vec::new();
            for member in &self.members {
                if member.room == room_id
                    && let Some(ti) = self.team_index(member.team)
                    && !teams_here.contains(&ti)
                {
                    teams_here.push(ti);
                }
            }

            if teams_here.is_empty() {
                // Empty rooms regrow — the reason backtracking pays.
                self.rooms[index].charge =
                    (self.rooms[index].charge + REGEN_RATE * dt).min(MAX_CHARGE);
            } else if self.rooms[index].charge > 0.0 {
                let demand = HARVEST_RATE * dt * teams_here.len() as f32;
                let available = self.rooms[index].charge.min(demand);
                let per_team = available / teams_here.len() as f32;
                for &ti in &teams_here {
                    self.scores[ti] += per_team * dispersion[ti];
                }
                self.rooms[index].charge -= available;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn single_member_world() -> IncentiveWorld {
        IncentiveWorld {
            rooms: vec![
                Room {
                    id: RoomId(0),
                    charge: MAX_CHARGE,
                },
                Room {
                    id: RoomId(1),
                    charge: MAX_CHARGE,
                },
            ],
            members: vec![Member {
                id: PlayerId(0),
                team: TeamId(0),
                room: RoomId(0),
                spawn_room: RoomId(0),
            }],
            teams: vec![TeamId(0)],
            scores: vec![0.0],
            tick_count: 0,
            last_event: String::new(),
        }
    }

    #[test]
    fn authored_world_has_two_teams_and_six_rooms() {
        let world = IncentiveWorld::authored();
        assert_eq!(world.rooms.len(), ROOM_COUNT);
        assert_eq!(world.members.len(), TOTAL_MEMBERS);
        // Team A spread across three rooms, team B clumped in one.
        assert_eq!(world.coverage(TeamId(0)), 3);
        assert_eq!(world.coverage(TeamId(1)), 1);
    }

    #[test]
    fn dispersion_scales_with_distinct_rooms() {
        let world = IncentiveWorld::authored();
        assert!((world.dispersion(TeamId(1)) - 1.0).abs() < 1e-6); // clumped
        assert!((world.dispersion(TeamId(0)) - (1.0 + 2.0 * SPREAD_BONUS)).abs() < 1e-6); // spread×3
    }

    #[test]
    fn a_split_team_outscores_a_clumped_team() {
        let mut world = IncentiveWorld::authored();
        for _ in 0..300 {
            world.tick(DT);
        }
        let spread = world.score_of(TeamId(0));
        let clumped = world.score_of(TeamId(1));
        assert!(
            spread > clumped * 2.0,
            "splitting must clearly out-score clumping"
        );
    }

    #[test]
    fn a_room_left_behind_regrows() {
        let mut world = single_member_world();
        // Camp room 0 until it is drained.
        for _ in 0..600 {
            world.tick(DT);
        }
        assert!(world.room(RoomId(0)).unwrap().charge < 0.05);
        // Leave; the room regrows while empty.
        world.move_member(PlayerId(0), RoomId(1));
        for _ in 0..300 {
            world.tick(DT);
        }
        assert!(
            world.room(RoomId(0)).unwrap().charge > 0.4,
            "an abandoned room regrows"
        );
    }

    #[test]
    fn cycling_rooms_outscores_camping_one() {
        // Camp run.
        let mut camp = single_member_world();
        for _ in 0..1200 {
            camp.tick(DT);
        }
        // Cycle run: alternate room 0 / room 1 so each regrows while away.
        let mut cycle = single_member_world();
        for step in 0..1200 {
            if step % 120 == 0 {
                let target = if (step / 120) % 2 == 0 {
                    RoomId(1)
                } else {
                    RoomId(0)
                };
                cycle.move_member(PlayerId(0), target);
            }
            cycle.tick(DT);
        }
        assert!(
            cycle.score_of(TeamId(0)) > camp.score_of(TeamId(0)) * 1.5,
            "revisiting regrown rooms beats camping a drained one"
        );
    }

    #[test]
    fn the_model_is_deterministic() {
        let mut a = IncentiveWorld::authored();
        let mut b = IncentiveWorld::authored();
        for _ in 0..200 {
            a.tick(DT);
            b.tick(DT);
        }
        assert_eq!(a.scores, b.scores);
    }

    #[test]
    fn reset_restores_the_authored_state() {
        let mut world = IncentiveWorld::authored();
        for _ in 0..200 {
            world.tick(DT);
        }
        world.move_member(PlayerId(0), RoomId(4));
        world.reset();
        assert_eq!(world.scores, vec![0.0; TEAM_COUNT]);
        assert_eq!(world.tick_count, 0);
        assert_eq!(world.coverage(TeamId(0)), 3);
    }
}
