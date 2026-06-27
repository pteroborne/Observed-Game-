//! Pure-logic feasibility model for the **facility director** — the adversarial
//! "house" that controls the megastructure, and which *eliminated teams join*
//! (per `AGENTS.md`).
//!
//! Teams race to escape through capacity-limited exits. A **collapse line** chases
//! the leading team; any team that falls more than a gap behind is **absorbed**
//! into the facility director. Each absorbed team strengthens the director, so the
//! collapse accelerates — a feedback loop where falling behind makes the hunt
//! worse for everyone still running. The director never touches a player directly:
//! its only effect is the environmental collapse and (for joined teams) a
//! `scramble` that advances it. Progress is monotonic — interference is purely
//! indirect.

use observed_core::TeamId;

pub const RUNNER_COUNT: usize = 4;
pub const EXIT_CAPACITY: u8 = 2;
const BASE_RUN_RATE: f32 = 0.22;
const SPRINT_BONUS: f32 = 0.12;
/// How far behind the leader a team may fall before the collapse absorbs it.
const PURGE_GAP: f32 = 0.30;
/// Extra collapse speed contributed by each absorbed team.
const PURGE_PER_MEMBER: f32 = 0.05;
/// A joined team's manual intervention.
const SCRAMBLE_BUMP: f32 = 0.06;
/// Authored per-team speed factors (team 0 is fastest).
const SPEEDS: [f32; RUNNER_COUNT] = [1.0, 0.86, 0.70, 0.58];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Runner,
    Director,
}

#[derive(Clone, Copy, Debug)]
pub struct Team {
    pub id: TeamId,
    pub progress: f32,
    pub speed: f32,
    pub role: Role,
    pub placement: Option<u8>,
}

impl Team {
    /// Still in the race: a runner that has neither escaped nor been absorbed.
    pub fn active_runner(&self) -> bool {
        self.role == Role::Runner && self.placement.is_none()
    }

    pub fn status(&self) -> String {
        match (self.placement, self.role) {
            (Some(1), _) => "ESCAPED 1st".to_string(),
            (Some(2), _) => "ESCAPED 2nd".to_string(),
            (Some(n), _) => format!("ESCAPED {n}th"),
            (None, Role::Director) => "ABSORBED".to_string(),
            (None, Role::Runner) => format!("{:.0}%", self.progress * 100.0),
        }
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct DirectorWorld {
    pub teams: Vec<Team>,
    pub purge_line: f32,
    pub exit_capacity: u8,
    pub slots_remaining: u8,
    pub next_placement: u8,
    pub director_actions: u32,
    pub finished: bool,
    pub tick_count: u32,
    pub last_event: String,
}

impl DirectorWorld {
    pub fn authored() -> Self {
        let teams = (0..RUNNER_COUNT)
            .map(|i| Team {
                id: TeamId(i as u8),
                progress: 0.0,
                speed: SPEEDS[i],
                role: Role::Runner,
                placement: None,
            })
            .collect();
        Self {
            teams,
            purge_line: -PURGE_GAP,
            exit_capacity: EXIT_CAPACITY,
            slots_remaining: EXIT_CAPACITY,
            next_placement: 1,
            director_actions: 0,
            finished: false,
            tick_count: 0,
            last_event: format!(
                "{RUNNER_COUNT} teams flee; {EXIT_CAPACITY} exits. Fall behind and the facility takes you."
            ),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    pub fn team(&self, id: TeamId) -> Option<&Team> {
        self.teams.iter().find(|team| team.id == id)
    }

    /// Number of teams absorbed into the director.
    pub fn director_strength(&self) -> usize {
        self.teams
            .iter()
            .filter(|t| t.role == Role::Director)
            .count()
    }

    pub fn escaped_count(&self) -> usize {
        self.teams.iter().filter(|t| t.placement.is_some()).count()
    }

    /// How fast the collapse advances, given how many teams the director has taken.
    pub fn purge_rate(&self) -> f32 {
        self.director_strength() as f32 * PURGE_PER_MEMBER
    }

    pub fn leader_progress(&self) -> f32 {
        self.teams.iter().map(|t| t.progress).fold(0.0, f32::max)
    }

    /// Standings: escaped teams (by rank) first, then running teams by progress,
    /// then absorbed teams.
    pub fn standings(&self) -> Vec<TeamId> {
        let mut order: Vec<&Team> = self.teams.iter().collect();
        order.sort_by(|a, b| {
            let rank = |t: &Team| match (t.placement, t.role) {
                (Some(p), _) => (0u8, p, 0.0f32),
                (None, Role::Runner) => (1, 0, -t.progress),
                (None, Role::Director) => (2, 0, 0.0),
            };
            rank(a)
                .partial_cmp(&rank(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        order.into_iter().map(|t| t.id).collect()
    }

    /// Advance one tick. `boosts` are teams sprinting this tick (the human).
    pub fn tick(&mut self, boosts: &[TeamId], dt: f32) {
        if self.finished {
            return;
        }
        self.tick_count += 1;

        // 1. Active runners advance (only their own progress, only upward).
        for team in &mut self.teams {
            if team.active_runner() {
                let rate = BASE_RUN_RATE * team.speed
                    + if boosts.contains(&team.id) {
                        SPRINT_BONUS
                    } else {
                        0.0
                    };
                team.progress = (team.progress + rate * dt).min(1.0);
            }
        }

        // 2. The collapse rises (director-driven) and chases the leader.
        let target = self.leader_progress() - PURGE_GAP;
        self.purge_line = (self.purge_line + self.purge_rate() * dt)
            .max(target)
            .min(1.0);

        // 3. Escapes claim capacity-limited exits, deterministically.
        let mut finishers: Vec<TeamId> = self
            .teams
            .iter()
            .filter(|t| t.active_runner() && t.progress >= 1.0)
            .map(|t| t.id)
            .collect();
        finishers.sort_by_key(|t| t.0);
        for id in finishers {
            if self.slots_remaining > 0 {
                let place = self.next_placement;
                self.next_placement += 1;
                self.slots_remaining -= 1;
                if let Some(team) = self.teams.iter_mut().find(|t| t.id == id) {
                    team.placement = Some(place);
                }
                self.last_event = format!("{} escaped — placement {place}.", id.label());
            }
        }

        // 4. Teams behind the collapse line are absorbed into the director.
        let line = self.purge_line;
        let mut absorbed = Vec::new();
        for team in &mut self.teams {
            if team.active_runner() && team.progress < line {
                team.role = Role::Director;
                absorbed.push(team.id);
            }
        }
        for id in absorbed {
            self.last_event = format!("{} fell behind — absorbed into the facility.", id.label());
        }

        // 5. Once the exits are full, anyone still running is locked out and taken.
        if self.slots_remaining == 0 {
            for team in &mut self.teams {
                if team.active_runner() {
                    team.role = Role::Director;
                }
            }
        }

        self.finished = !self.teams.iter().any(Team::active_runner);
        if self.finished {
            self.last_event = format!(
                "Run over — {} escaped, {} absorbed by the facility.",
                self.escaped_count(),
                self.director_strength()
            );
        }
    }

    /// A joined team accelerates the collapse. Only director members may act.
    pub fn scramble(&mut self, by: TeamId) -> bool {
        if self.finished || self.team(by).map(|t| t.role) != Some(Role::Director) {
            return false;
        }
        self.purge_line = (self.purge_line + SCRAMBLE_BUMP).min(1.0);
        self.director_actions += 1;
        self.last_event = format!(
            "{} (facility) scrambled — the collapse lurches forward.",
            by.label()
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn run_to_finish(world: &mut DirectorWorld, boosts: &[TeamId]) {
        for _ in 0..100_000 {
            if world.finished {
                break;
            }
            world.tick(boosts, DT);
        }
        assert!(world.finished, "the run must terminate");
    }

    #[test]
    fn authored_world_has_runners_and_open_exits() {
        let world = DirectorWorld::authored();
        assert_eq!(world.teams.len(), RUNNER_COUNT);
        assert!(world.teams.iter().all(|t| t.role == Role::Runner));
        assert_eq!(world.director_strength(), 0);
        assert_eq!(world.slots_remaining, EXIT_CAPACITY);
    }

    #[test]
    fn some_escape_and_the_rest_are_absorbed() {
        let mut world = DirectorWorld::authored();
        run_to_finish(&mut world, &[]);
        let escaped = world.escaped_count();
        let absorbed = world.director_strength();
        assert!(escaped >= 1, "the leader always escapes");
        assert!(
            escaped <= EXIT_CAPACITY as usize,
            "exits are capacity-limited"
        );
        assert_eq!(escaped + absorbed, RUNNER_COUNT, "every team is resolved");
    }

    #[test]
    fn the_leader_is_never_absorbed() {
        let mut world = DirectorWorld::authored();
        run_to_finish(&mut world, &[]);
        // Team 0 is fastest; it should escape, never be absorbed.
        assert!(world.team(TeamId(0)).unwrap().placement.is_some());
    }

    #[test]
    fn absorbing_a_team_strengthens_and_accelerates_the_director() {
        let mut world = DirectorWorld::authored();
        assert_eq!(world.purge_rate(), 0.0);
        // Run until at least one team is absorbed.
        for _ in 0..100_000 {
            world.tick(&[], DT);
            if world.director_strength() > 0 {
                break;
            }
        }
        assert!(world.director_strength() > 0);
        assert!(
            world.purge_rate() > 0.0,
            "each absorbed team speeds the collapse"
        );
    }

    #[test]
    fn capacity_limits_the_escapers() {
        let mut world = DirectorWorld::authored();
        run_to_finish(&mut world, &[]);
        assert_eq!(
            world.slots_remaining + world.escaped_count() as u8,
            EXIT_CAPACITY
        );
        assert!(world.escaped_count() <= EXIT_CAPACITY as usize);
    }

    #[test]
    fn scramble_is_director_only_and_advances_the_collapse() {
        let mut world = DirectorWorld::authored();
        // A runner cannot scramble.
        assert!(!world.scramble(TeamId(0)));
        // Force a team into the director, then it can.
        if let Some(team) = world.teams.iter_mut().find(|t| t.id == TeamId(3)) {
            team.role = Role::Director;
        }
        let before = world.purge_line;
        assert!(world.scramble(TeamId(3)));
        assert!(world.purge_line > before);
        assert_eq!(world.director_actions, 1);
    }

    #[test]
    fn interference_is_indirect_progress_never_drops() {
        let mut world = DirectorWorld::authored();
        let mut previous: Vec<f32> = world.teams.iter().map(|t| t.progress).collect();
        for _ in 0..600 {
            world.tick(&[], DT);
            for (team, prev) in world.teams.iter().zip(previous.iter()) {
                assert!(
                    team.progress + 1e-6 >= *prev,
                    "no team's progress may decrease"
                );
            }
            previous = world.teams.iter().map(|t| t.progress).collect();
            if world.finished {
                break;
            }
        }
    }

    #[test]
    fn the_run_is_deterministic() {
        let mut a = DirectorWorld::authored();
        let mut b = DirectorWorld::authored();
        run_to_finish(&mut a, &[]);
        run_to_finish(&mut b, &[]);
        let progress_a: Vec<f32> = a.teams.iter().map(|t| t.progress).collect();
        let progress_b: Vec<f32> = b.teams.iter().map(|t| t.progress).collect();
        assert_eq!(progress_a, progress_b);
        assert_eq!(a.standings(), b.standings());
    }

    #[test]
    fn reset_restores_a_fresh_run() {
        let mut world = DirectorWorld::authored();
        run_to_finish(&mut world, &[]);
        world.reset();
        assert!(!world.finished);
        assert_eq!(world.director_strength(), 0);
        assert_eq!(world.slots_remaining, EXIT_CAPACITY);
        assert!(
            world
                .teams
                .iter()
                .all(|t| t.progress == 0.0 && t.active_runner())
        );
    }
}
