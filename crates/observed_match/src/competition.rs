//! Pure-logic feasibility model for **competition**: several teams race through a
//! shared structure toward **capacity-limited exits**, and the match resolves to
//! a deterministic placement (winner, runners-up, locked-out).
//!
//! The design rule from `AGENTS.md` is that teams *cannot directly harm* each
//! other — they only manipulate shared systems. Here that is a single contested
//! **control**: a team can spend a tick seizing it instead of racing, and while
//! it holds the control it advances faster. A team's progress is only ever raised
//! by its own racing, never lowered by an opponent — interference is purely
//! indirect (winning the shared advantage), and exits run out, so finishing
//! order matters. The contested control stands in for the shared machinery /
//! routes / observation proven in the earlier labs.

use observed_core::TeamId;

pub const TEAM_COUNT: usize = 3;
pub const EXIT_CAPACITY: u8 = 2;
const BASE_RATE: f32 = 0.16;
const BOOST_RATE: f32 = 0.14;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RaceAction {
    /// Advance toward the exit (boosted while holding the control).
    Advance,
    /// Spend the tick seizing the shared control instead of advancing.
    Seize,
}

#[derive(Clone, Copy, Debug)]
pub struct TeamState {
    pub id: TeamId,
    pub progress: f32,
    pub placement: Option<u8>,
    pub eliminated: bool,
    pub members: u8,
}

impl TeamState {
    pub fn racing(&self) -> bool {
        self.placement.is_none() && !self.eliminated
    }

    pub fn status(&self) -> String {
        match (self.placement, self.eliminated) {
            (Some(1), _) => "1st".to_string(),
            (Some(2), _) => "2nd".to_string(),
            (Some(n), _) => format!("{n}th"),
            (None, true) => "LOCKED OUT".to_string(),
            (None, false) => format!("{:.0}%", self.progress * 100.0),
        }
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct CompetitionWorld {
    pub teams: Vec<TeamState>,
    pub exit_capacity: u8,
    pub slots_remaining: u8,
    pub control_holder: Option<TeamId>,
    pub next_placement: u8,
    pub winner: Option<TeamId>,
    pub finished: bool,
    pub tick_count: u32,
    pub last_event: String,
}

impl CompetitionWorld {
    pub fn authored() -> Self {
        let teams = (0..TEAM_COUNT)
            .map(|i| TeamState {
                id: TeamId(i as u8),
                progress: 0.0,
                placement: None,
                eliminated: false,
                members: 2,
            })
            .collect();
        Self {
            teams,
            exit_capacity: EXIT_CAPACITY,
            slots_remaining: EXIT_CAPACITY,
            control_holder: None,
            next_placement: 1,
            winner: None,
            finished: false,
            tick_count: 0,
            last_event: format!(
                "{} teams race; only {EXIT_CAPACITY} exits — seize the control to pull ahead.",
                TEAM_COUNT
            ),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::authored();
    }

    pub fn team(&self, id: TeamId) -> Option<&TeamState> {
        self.teams.iter().find(|team| team.id == id)
    }

    fn team_index(&self, id: TeamId) -> Option<usize> {
        self.teams.iter().position(|team| team.id == id)
    }

    pub fn is_racing(&self, id: TeamId) -> bool {
        self.team(id).is_some_and(TeamState::racing)
    }

    /// Final standings, best placement first, then locked-out teams.
    pub fn standings(&self) -> Vec<TeamId> {
        let mut order: Vec<&TeamState> = self.teams.iter().collect();
        order.sort_by(|a, b| match (a.placement, b.placement) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.progress.total_cmp(&a.progress),
        });
        order.into_iter().map(|team| team.id).collect()
    }

    /// Advance the match by one tick from per-team race actions. Teams not listed
    /// default to `Advance`. Resolution is deterministic (ascending `TeamId`).
    pub fn tick(&mut self, intents: &[(TeamId, RaceAction)], dt: f32) {
        if self.finished {
            return;
        }
        self.tick_count += 1;

        let action_for = |id: TeamId| {
            intents
                .iter()
                .find(|(team, _)| *team == id)
                .map(|(_, action)| *action)
                .unwrap_or(RaceAction::Advance)
        };

        // 1. Contest the shared control — lowest TeamId wins a tie. Seizing teams
        //    forgo advancing this tick (the opportunity cost of interfering).
        let mut seizers: Vec<TeamId> = self
            .teams
            .iter()
            .filter(|team| team.racing() && action_for(team.id) == RaceAction::Seize)
            .map(|team| team.id)
            .collect();
        seizers.sort_by_key(|team| team.0);
        if let Some(&winner) = seizers.first()
            && self.control_holder != Some(winner)
        {
            self.control_holder = Some(winner);
            self.last_event = format!("{} seized the shared control.", winner.label());
        }

        // 2. Advance every racing team that chose to race.
        let holder = self.control_holder;
        for team in &mut self.teams {
            if team.racing() && action_for(team.id) == RaceAction::Advance {
                let rate = BASE_RATE
                    + if holder == Some(team.id) {
                        BOOST_RATE
                    } else {
                        0.0
                    };
                team.progress = (team.progress + rate * dt).min(1.0);
            }
        }

        // 3. Claim exits in deterministic order; capacity is limited.
        let mut finishers: Vec<TeamId> = self
            .teams
            .iter()
            .filter(|team| team.racing() && team.progress >= 1.0)
            .map(|team| team.id)
            .collect();
        finishers.sort_by_key(|team| team.0);
        for id in finishers {
            if self.slots_remaining > 0 {
                let place = self.next_placement;
                self.next_placement += 1;
                self.slots_remaining -= 1;
                let index = self.team_index(id).expect("team exists");
                self.teams[index].placement = Some(place);
                if place == 1 {
                    self.winner = Some(id);
                }
                self.last_event = format!("{} reached an exit — placement {place}.", id.label());
            }
        }

        // 4. When the exits are full, everyone still racing is locked out.
        if self.slots_remaining == 0 {
            for team in &mut self.teams {
                if team.racing() {
                    team.eliminated = true;
                }
            }
        }

        self.finished = self
            .teams
            .iter()
            .all(|team| team.placement.is_some() || team.eliminated);
        if self.finished {
            self.last_event = match self.winner {
                Some(team) => format!("Match over — {} wins.", team.label()),
                None => "Match over.".to_string(),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn run_until_finished(world: &mut CompetitionWorld, intents: &[(TeamId, RaceAction)]) {
        for _ in 0..100_000 {
            if world.finished {
                break;
            }
            world.tick(intents, DT);
        }
        assert!(world.finished, "match should finish");
    }

    #[test]
    fn authored_match_has_three_teams_and_limited_exits() {
        let world = CompetitionWorld::authored();
        assert_eq!(world.teams.len(), 3);
        assert_eq!(world.slots_remaining, EXIT_CAPACITY);
        assert!(world.team(TeamId(0)).unwrap().racing());
    }

    #[test]
    fn equal_racing_fills_exits_by_team_id_and_locks_out_the_rest() {
        let mut world = CompetitionWorld::authored();
        run_until_finished(&mut world, &[]); // all default to Advance
        assert_eq!(world.team(TeamId(0)).unwrap().placement, Some(1));
        assert_eq!(world.team(TeamId(1)).unwrap().placement, Some(2));
        assert!(world.team(TeamId(2)).unwrap().eliminated);
        assert_eq!(world.winner, Some(TeamId(0)));
    }

    #[test]
    fn capacity_limits_the_number_of_escapers() {
        let mut world = CompetitionWorld::authored();
        run_until_finished(&mut world, &[]);
        let placed = world.teams.iter().filter(|t| t.placement.is_some()).count();
        let locked = world.teams.iter().filter(|t| t.eliminated).count();
        assert_eq!(placed, EXIT_CAPACITY as usize);
        assert_eq!(locked, TEAM_COUNT - EXIT_CAPACITY as usize);
    }

    #[test]
    fn holding_the_control_flips_the_result() {
        // Team 2 (highest id, last by default tie-break) seizes once then races
        // with the boost — it should now win.
        let mut world = CompetitionWorld::authored();
        world.tick(&[(TeamId(2), RaceAction::Seize)], DT);
        run_until_finished(&mut world, &[(TeamId(2), RaceAction::Advance)]);
        assert_eq!(world.winner, Some(TeamId(2)));
        assert_eq!(world.team(TeamId(2)).unwrap().placement, Some(1));
    }

    #[test]
    fn seizing_forgoes_advancing_that_tick() {
        let mut world = CompetitionWorld::authored();
        world.tick(&[(TeamId(0), RaceAction::Seize)], DT);
        assert_eq!(world.team(TeamId(0)).unwrap().progress, 0.0);
        assert_eq!(world.control_holder, Some(TeamId(0)));
        // Team 1 raced and made progress.
        assert!(world.team(TeamId(1)).unwrap().progress > 0.0);
    }

    #[test]
    fn control_contention_is_deterministic() {
        let mut world = CompetitionWorld::authored();
        world.tick(
            &[
                (TeamId(2), RaceAction::Seize),
                (TeamId(1), RaceAction::Seize),
            ],
            DT,
        );
        assert_eq!(world.control_holder, Some(TeamId(1)));
    }

    #[test]
    fn interference_never_directly_harms_an_opponent() {
        // Across an adversarial run, no team's progress ever decreases.
        let mut world = CompetitionWorld::authored();
        let mut previous: Vec<f32> = world.teams.iter().map(|t| t.progress).collect();
        for tick in 0..400 {
            // Team 0 keeps seizing; others race.
            world.tick(&[(TeamId(0), RaceAction::Seize)], DT);
            for (team, prev) in world.teams.iter().zip(previous.iter()) {
                assert!(
                    team.progress + 1e-6 >= *prev,
                    "progress must never drop (tick {tick})"
                );
            }
            previous = world.teams.iter().map(|t| t.progress).collect();
            if world.finished {
                break;
            }
        }
    }

    #[test]
    fn the_match_is_deterministic() {
        let mut a = CompetitionWorld::authored();
        let mut b = CompetitionWorld::authored();
        let script = [(TeamId(2), RaceAction::Seize)];
        for _ in 0..120 {
            a.tick(&script, DT);
            b.tick(&script, DT);
        }
        let progress_a: Vec<f32> = a.teams.iter().map(|t| t.progress).collect();
        let progress_b: Vec<f32> = b.teams.iter().map(|t| t.progress).collect();
        assert_eq!(progress_a, progress_b);
        assert_eq!(a.winner, b.winner);
    }

    #[test]
    fn reset_restores_a_fresh_match() {
        let mut world = CompetitionWorld::authored();
        run_until_finished(&mut world, &[]);
        world.reset();
        assert!(!world.finished);
        assert_eq!(world.slots_remaining, EXIT_CAPACITY);
        assert!(world.winner.is_none());
        assert!(world.teams.iter().all(|t| t.progress == 0.0 && t.racing()));
    }
}
