//! Phase 11 integration: the **competitive facility**. It takes the mutable
//! structure proven in [`mutable_facility`] (observation + the protected spine,
//! itself reusing [`constraint_lab`] and [`observation_lab`]) and runs a *full
//! competitive match* over it by folding in the two proven competitive systems:
//!
//! - **Competition** ([`competition_lab`]): several teams race the same structure
//!   toward **capacity-limited exits**; finishing order is a deterministic
//!   placement (by `TeamId`), and a single **contested control** (its
//!   [`RaceAction`]) is the only way to interfere — a team spends a round seizing
//!   it instead of advancing, and the holder sprints. No team ever lowers another
//!   team's progress; interference is purely indirect.
//! - **Facility director** ([`director_lab`]): a **collapse line** chases the
//!   leader; any team that falls a gap behind is **absorbed into the director**
//!   (its [`Role`] flips to `Director`), and each absorbed team accelerates the
//!   collapse. The leader is never absorbed. Absorbed teams may `scramble` to push
//!   the collapse — the "eliminated teams join the facility AI" rule.
//!
//! The crucial change from the scalar feasibility labs is that **progress is graph
//! position**, not an abstract number: a team's progress is how far its clump has
//! travelled along the protected spine (`mutable_facility::spine_next`) toward the
//! exit, while the unobserved structure rewires behind it every round. The spine
//! stays traversable, so "follow the gold path" always works — the race resolves
//! despite the churn.
//!
//! Exit criterion (ROADMAP Phase 11): a full match resolves to a deterministic
//! placement with at least one team escaping and the rest absorbed.

use crate::competition::RaceAction;
use crate::director::Role;
use crate::mutable::spine_next;
use observed_core::{RoomId, TeamId};
use observed_facility::{
    constraints::ConstraintWorld,
    map_spec::{MapSpec, RoomRole},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Re-exported from `mutable_facility` — the entrance and exit rooms this lab's
/// presentation and tests refer to.
pub use crate::mutable::{EXIT_ROOM, START_ROOM};

pub const TEAM_COUNT: usize = 4;
pub const MEMBERS_PER_TEAM: usize = 2;
pub const PLAYER_COUNT: usize = TEAM_COUNT * MEMBERS_PER_TEAM;
pub const EXIT_CAPACITY: u8 = 2;

/// The protected spanning path, in visiting order (matches `constraint_lab`'s
/// spine and `mutable_facility::spine_next`). Progress is an index into this.
const SPINE_ROOMS: [u32; 9] = [0, 1, 2, 5, 4, 3, 6, 7, 8];
/// The number of steps from entrance to exit — the denominator for progress.
const SPINE_STEPS: f32 = (SPINE_ROOMS.len() - 1) as f32;

/// Authored per-team pace (team 0 is fastest). A team accumulates its pace each
/// round and takes one spine step per whole unit — deterministic differing speed
/// without leaving the discrete graph.
const SPEEDS: [f32; TEAM_COUNT] = [1.0, 0.82, 0.64, 0.5];
/// Extra pace for the team currently holding the contested control.
const SPRINT_BONUS: f32 = 0.5;
/// How far behind the leader (in progress fraction) a team may fall before the
/// collapse absorbs it.
const PURGE_GAP: f32 = 0.30;
/// Extra collapse speed per round contributed by each absorbed team.
const PURGE_PER_TEAM: f32 = 0.015;
/// A joined (director) team's manual shove to the collapse.
const SCRAMBLE_BUMP: f32 = 0.06;

fn spine_index(room: RoomId) -> usize {
    SPINE_ROOMS.iter().position(|&r| r == room.0).unwrap_or(0)
}

#[derive(Clone, Copy, Debug)]
pub struct TeamRun {
    pub id: TeamId,
    /// Index of this team's first member in the shared `structure.graph.players`.
    pub member_base: usize,
    pub members: u8,
    pub speed: f32,
    pub pace: f32,
    pub role: Role,
    pub placement: Option<u8>,
    pub objective_index: usize,
}

impl TeamRun {
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
            (None, Role::Runner) => String::new(), // progress filled in by caller
        }
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug)]
pub struct CompetitiveFacility {
    /// The mutable structure, shared by every team (their members are the
    /// observation set). Reused wholesale from the constraint/observation labs.
    pub structure: ConstraintWorld,
    pub teams: Vec<TeamRun>,
    /// The single shared control; holding it grants the sprint bonus.
    pub control_holder: Option<TeamId>,
    /// The collapse frontier, as a progress fraction in `[-gap, 1]`.
    pub purge_line: f32,
    pub exit_capacity: u8,
    pub slots_remaining: u8,
    pub next_placement: u8,
    pub winner: Option<TeamId>,
    pub director_actions: u32,
    pub round: u32,
    pub decohere_count: u32,
    pub finished: bool,
    pub last_event: String,
    pub map_spec: Option<MapSpec>,
}

impl CompetitiveFacility {
    pub fn authored() -> Self {
        let mut structure = ConstraintWorld::authored();
        // Every team starts as a clump at the entrance; all members occupy the
        // shared players vec, so every team's room is observed and frozen.
        structure.graph.players = vec![RoomId(START_ROOM); PLAYER_COUNT];
        structure.recompute_connectivity();

        let teams = (0..TEAM_COUNT)
            .map(|i| TeamRun {
                id: TeamId(i as u8),
                member_base: i * MEMBERS_PER_TEAM,
                members: MEMBERS_PER_TEAM as u8,
                speed: SPEEDS[i],
                pace: 0.0,
                role: Role::Runner,
                placement: None,
                objective_index: 0,
            })
            .collect();

        Self {
            structure,
            teams,
            control_holder: None,
            purge_line: -PURGE_GAP,
            exit_capacity: EXIT_CAPACITY,
            slots_remaining: EXIT_CAPACITY,
            next_placement: 1,
            winner: None,
            director_actions: 0,
            round: 0,
            decohere_count: 0,
            finished: false,
            last_event: format!(
                "{TEAM_COUNT} teams race the shifting facility; {EXIT_CAPACITY} exits. Fall behind and it takes you."
            ),
            map_spec: None,
        }
    }

    pub fn for_map_spec(spec: MapSpec) -> Self {
        spec.validate_or_panic();
        let start = spec.start_room().expect("validated map start");
        let mut structure = ConstraintWorld::from_map_spec(&spec);
        structure.graph.players = vec![start; PLAYER_COUNT];
        structure.recompute_connectivity();

        let teams = (0..TEAM_COUNT)
            .map(|i| TeamRun {
                id: TeamId(i as u8),
                member_base: i * MEMBERS_PER_TEAM,
                members: MEMBERS_PER_TEAM as u8,
                speed: SPEEDS[i],
                pace: 0.0,
                role: Role::Runner,
                placement: None,
                objective_index: 0,
            })
            .collect();

        Self {
            structure,
            teams,
            control_holder: None,
            purge_line: -PURGE_GAP,
            exit_capacity: EXIT_CAPACITY,
            slots_remaining: EXIT_CAPACITY,
            next_placement: 1,
            winner: None,
            director_actions: 0,
            round: 0,
            decohere_count: 0,
            finished: false,
            last_event: format!(
                "{}: redundant graph, no protected route; tools stabilize practical paths.",
                spec.name
            ),
            map_spec: Some(spec),
        }
    }

    pub fn reset(&mut self) {
        if let Some(spec) = self.map_spec.clone() {
            *self = Self::for_map_spec(spec);
        } else {
            *self = Self::authored();
        }
    }

    pub fn team(&self, id: TeamId) -> Option<&TeamRun> {
        self.teams.iter().find(|t| t.id == id)
    }

    /// The room a team's clump currently occupies (its lead member).
    pub fn team_room(&self, index: usize) -> RoomId {
        self.structure.graph.players[self.teams[index].member_base]
    }

    /// A team's progress toward the exit, as a fraction of the spine.
    pub fn team_progress(&self, index: usize) -> f32 {
        if self.map_spec.is_some() {
            let sequence = self.objective_sequence();
            if sequence.is_empty() {
                return 0.0;
            }
            return (self.teams[index].objective_index.min(sequence.len()) as f32
                / sequence.len() as f32)
                .min(1.0);
        }
        spine_index(self.team_room(index)) as f32 / SPINE_STEPS
    }

    pub fn progress_of(&self, id: TeamId) -> f32 {
        self.teams
            .iter()
            .position(|t| t.id == id)
            .map(|i| self.team_progress(i))
            .unwrap_or(0.0)
    }

    /// The furthest progress any team has reached (escaped teams count as 1.0).
    pub fn leader_progress(&self) -> f32 {
        (0..self.teams.len())
            .map(|i| self.team_progress(i))
            .fold(0.0, f32::max)
    }

    pub fn director_strength(&self) -> usize {
        self.teams
            .iter()
            .filter(|t| t.role == Role::Director)
            .count()
    }

    pub fn escaped_count(&self) -> usize {
        self.teams.iter().filter(|t| t.placement.is_some()).count()
    }

    pub fn absorbed_count(&self) -> usize {
        self.teams
            .iter()
            .filter(|t| t.role == Role::Director && t.placement.is_none())
            .count()
    }

    /// How fast the collapse advances per round, given absorbed teams.
    pub fn purge_rate(&self) -> f32 {
        self.absorbed_count() as f32 * PURGE_PER_TEAM
    }

    pub fn connected(&self) -> bool {
        self.structure.connected
    }

    /// Standings: escaped teams (by rank) first, then running teams by progress,
    /// then absorbed teams.
    pub fn standings(&self) -> Vec<TeamId> {
        let mut order: Vec<usize> = (0..self.teams.len()).collect();
        order.sort_by(|&a, &b| {
            let rank = |i: usize| match (self.teams[i].placement, self.teams[i].role) {
                (Some(p), _) => (0u8, p, 0.0f32),
                (None, Role::Runner) => (1, 0, -self.team_progress(i)),
                (None, Role::Director) => (2, 0, 0.0),
            };
            rank(a)
                .partial_cmp(&rank(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        order.into_iter().map(|i| self.teams[i].id).collect()
    }

    fn action_for(intents: &[(TeamId, RaceAction)], id: TeamId) -> RaceAction {
        intents
            .iter()
            .find(|(team, _)| *team == id)
            .map(|(_, a)| *a)
            .unwrap_or(RaceAction::Advance)
    }

    /// Move a team's whole clump one step along the protected spine. Because the
    /// spine never rewires, this lands on the next spine room however the rest of
    /// the structure has decohered.
    fn step_team(&mut self, index: usize) {
        if self.map_spec.is_some() {
            self.step_team_toward_objective(index);
            return;
        }
        let room = self.team_room(index);
        if let Some((_next, side)) = spine_next(room) {
            let base = self.teams[index].member_base;
            for offset in 0..self.teams[index].members as usize {
                self.structure.traverse(base + offset, side);
            }
        }
    }

    fn team_at_exit(&self, index: usize) -> bool {
        if let Some(spec) = &self.map_spec {
            let at_exit = spec
                .exit_room()
                .is_some_and(|exit| self.team_room(index) == exit);
            return at_exit && self.teams[index].objective_index >= self.objective_sequence().len();
        }
        self.team_room(index).0 == EXIT_ROOM
    }

    pub fn next_room_for_team(&self, index: usize) -> Option<RoomId> {
        if self.map_spec.is_none() {
            return spine_next(self.team_room(index)).map(|(room, _)| room);
        }
        let goal = self.current_goal(index)?;
        let room = self.team_room(index);
        if room == goal {
            return Some(goal);
        }
        graph_path(&self.structure.graph, room, goal).and_then(|path| path.get(1).copied())
    }

    fn step_team_toward_objective(&mut self, index: usize) {
        self.sync_completed_objectives(index);
        let Some(goal) = self.current_goal(index) else {
            return;
        };
        let room = self.team_room(index);
        if room == goal {
            self.sync_completed_objectives(index);
            return;
        }
        let Some(next) =
            graph_path(&self.structure.graph, room, goal).and_then(|path| path.get(1).copied())
        else {
            return;
        };
        let Some(side) = side_to_neighbor(&self.structure.graph, room, next) else {
            return;
        };
        let base = self.teams[index].member_base;
        for offset in 0..self.teams[index].members as usize {
            self.structure.traverse(base + offset, side);
        }
        self.sync_completed_objectives(index);
    }

    fn sync_completed_objectives(&mut self, index: usize) {
        let sequence = self.objective_sequence();
        while let Some(goal) = sequence.get(self.teams[index].objective_index).copied() {
            if self.team_room(index) != goal {
                break;
            }
            self.teams[index].objective_index += 1;
        }
    }

    fn current_goal(&self, index: usize) -> Option<RoomId> {
        let sequence = self.objective_sequence();
        sequence.get(self.teams[index].objective_index).copied()
    }

    pub fn objective_sequence(&self) -> Vec<RoomId> {
        let Some(spec) = &self.map_spec else {
            return SPINE_ROOMS.iter().copied().map(RoomId).collect();
        };
        let mut sequence = Vec::new();
        sequence.extend(spec.keystone_rooms());
        for role in [
            RoomRole::DualStation,
            RoomRole::AnchorCheckpoint,
            RoomRole::TeleportRelay,
            RoomRole::Exit,
        ] {
            if let Some(room) = spec.role_room(role)
                && !sequence.contains(&room)
            {
                sequence.push(room);
            }
        }
        sequence
    }

    /// Advance the match by one round from per-team race actions (default
    /// `Advance`). Deterministic: contention and exits resolve in `TeamId` order.
    pub fn advance_round(&mut self, intents: &[(TeamId, RaceAction)]) {
        if self.finished {
            return;
        }
        self.round += 1;

        // 1. Contest the shared control — lowest TeamId among seizers wins. A
        //    seizing team forgoes advancing this round (the cost of interfering).
        let mut seizers: Vec<TeamId> = self
            .teams
            .iter()
            .filter(|t| t.active_runner() && Self::action_for(intents, t.id) == RaceAction::Seize)
            .map(|t| t.id)
            .collect();
        seizers.sort_by_key(|t| t.0);
        if let Some(&winner) = seizers.first()
            && self.control_holder != Some(winner)
        {
            self.control_holder = Some(winner);
            self.last_event = format!("{} seized the shared control.", winner.label());
        }

        // 2. Advance each racing team along the spine by its pace.
        let holder = self.control_holder;
        for index in 0..self.teams.len() {
            if !self.teams[index].active_runner() {
                continue;
            }
            let id = self.teams[index].id;
            if Self::action_for(intents, id) == RaceAction::Seize {
                continue; // seized instead of advancing
            }
            let bonus = if holder == Some(id) {
                SPRINT_BONUS
            } else {
                0.0
            };
            self.teams[index].pace += self.teams[index].speed + bonus;
            while self.teams[index].pace >= 1.0 {
                self.teams[index].pace -= 1.0;
                self.step_team(index);
            }
        }

        // 3. The unobserved structure rewires (spine + occupied rooms hold).
        self.structure.decohere();
        self.decohere_count += 1;

        // 4. Claim capacity-limited exits in deterministic order.
        let mut finishers: Vec<usize> = (0..self.teams.len())
            .filter(|&i| self.teams[i].active_runner() && self.team_at_exit(i))
            .collect();
        finishers.sort_by_key(|&i| self.teams[i].id.0);
        for index in finishers {
            if self.slots_remaining > 0 {
                let place = self.next_placement;
                self.next_placement += 1;
                self.slots_remaining -= 1;
                self.teams[index].placement = Some(place);
                if place == 1 {
                    self.winner = Some(self.teams[index].id);
                }
                self.last_event = format!(
                    "{} reached an exit — placement {place}.",
                    self.teams[index].id.label()
                );
            }
        }

        // 5. The collapse rises (director-driven) and chases the leader.
        let target = self.leader_progress() - PURGE_GAP;
        self.purge_line = (self.purge_line + self.purge_rate()).max(target).min(1.0);

        // 6. Teams behind the collapse are absorbed into the director.
        let line = self.purge_line;
        for index in 0..self.teams.len() {
            if self.teams[index].active_runner() && self.team_progress(index) < line {
                self.teams[index].role = Role::Director;
                self.last_event = format!(
                    "{} fell behind — absorbed into the facility.",
                    self.teams[index].id.label()
                );
            }
        }

        // 7. Once the exits are full, anyone still running is locked out and taken.
        if self.slots_remaining == 0 {
            for team in &mut self.teams {
                if team.active_runner() {
                    team.role = Role::Director;
                }
            }
        }

        // 8. Resolve.
        self.finished = !self.teams.iter().any(TeamRun::active_runner);
        if self.finished {
            self.last_event = format!(
                "Match over — {} escaped, {} absorbed by the facility.",
                self.escaped_count(),
                self.absorbed_count()
            );
        }
    }

    /// The spine rooms the collapse has already swallowed (progress fraction at or
    /// below the collapse line). Used to draw the creeping collapse frontier.
    pub fn collapse_rooms(&self) -> Vec<RoomId> {
        if self.map_spec.is_some() {
            let sequence = self.objective_sequence();
            if sequence.is_empty() {
                return Vec::new();
            }
            return sequence
                .iter()
                .enumerate()
                .filter(|(i, _)| (*i as f32 / sequence.len() as f32) <= self.purge_line)
                .map(|(_, &room)| room)
                .collect();
        }
        SPINE_ROOMS
            .iter()
            .enumerate()
            .filter(|(i, _)| (*i as f32 / SPINE_STEPS) <= self.purge_line)
            .map(|(_, &room)| RoomId(room))
            .collect()
    }

    /// The furthest spine room the collapse currently reaches (its frontier).
    pub fn collapse_frontier(&self) -> Option<RoomId> {
        self.collapse_rooms().into_iter().next_back()
    }

    /// A joined (director) team shoves the collapse forward. Only absorbed teams
    /// may act — the "eliminated teams join the facility AI" rule.
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

fn graph_path(
    graph: &observed_observation::ObservationWorld,
    start: RoomId,
    goal: RoomId,
) -> Option<Vec<RoomId>> {
    if start == goal {
        return Some(vec![start]);
    }
    if start.0 as usize >= graph.room_count || goal.0 as usize >= graph.room_count {
        return None;
    }
    let mut parent = BTreeMap::<RoomId, RoomId>::new();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    seen.insert(start);
    queue.push_back(start);
    while let Some(room) = queue.pop_front() {
        for next in graph_neighbors(graph, room) {
            if seen.insert(next) {
                parent.insert(next, room);
                if next == goal {
                    let mut path = vec![goal];
                    let mut current = goal;
                    while let Some(&prev) = parent.get(&current) {
                        path.push(prev);
                        current = prev;
                        if current == start {
                            break;
                        }
                    }
                    path.reverse();
                    return Some(path);
                }
                queue.push_back(next);
            }
        }
    }
    None
}

fn graph_neighbors(graph: &observed_observation::ObservationWorld, room: RoomId) -> Vec<RoomId> {
    let mut out = Vec::new();
    for side in observed_observation::Side::ALL {
        let door = graph.door_id(room, side);
        if !graph.is_sealed(door) {
            out.push(graph.door(graph.partner(door)).room);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn side_to_neighbor(
    graph: &observed_observation::ObservationWorld,
    room: RoomId,
    neighbor: RoomId,
) -> Option<observed_observation::Side> {
    observed_observation::Side::ALL.into_iter().find(|&side| {
        let door = graph.door_id(room, side);
        !graph.is_sealed(door) && graph.door(graph.partner(door)).room == neighbor
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_to_finish(world: &mut CompetitiveFacility, intents: &[(TeamId, RaceAction)]) {
        for _ in 0..100_000 {
            if world.finished {
                break;
            }
            world.advance_round(intents);
        }
        assert!(world.finished, "the match must terminate");
    }

    #[test]
    fn authored_match_has_all_teams_at_the_entrance() {
        let world = CompetitiveFacility::authored();
        assert_eq!(world.teams.len(), TEAM_COUNT);
        assert_eq!(world.structure.graph.players.len(), PLAYER_COUNT);
        assert!((0..TEAM_COUNT).all(|i| world.team_room(i).0 == START_ROOM));
        assert!(world.connected());
        assert_eq!(world.slots_remaining, EXIT_CAPACITY);
        assert!(world.teams.iter().all(TeamRun::active_runner));
    }

    #[test]
    fn the_match_resolves_to_a_deterministic_placement() {
        // The exit criterion: at least one team escapes, the rest are absorbed.
        let mut world = CompetitiveFacility::authored();
        run_to_finish(&mut world, &[]);

        assert!(world.escaped_count() >= 1, "the leader always escapes");
        assert_eq!(
            world.escaped_count(),
            EXIT_CAPACITY as usize,
            "exits are capacity-limited"
        );
        assert_eq!(
            world.escaped_count() + world.absorbed_count(),
            TEAM_COUNT,
            "every team is resolved as escaped or absorbed"
        );
        // The fastest team wins.
        assert_eq!(world.winner, Some(TeamId(0)));
        assert_eq!(world.team(TeamId(0)).unwrap().placement, Some(1));
    }

    #[test]
    fn the_structure_keeps_shifting_yet_stays_traversable() {
        let mut world = CompetitiveFacility::authored();
        let initial_links = world.structure.graph.links.clone();
        for _ in 0..30 {
            if world.finished {
                break;
            }
            world.advance_round(&[]);
            assert!(world.connected(), "the spine keeps the exit reachable");
        }
        assert_ne!(
            world.structure.graph.links, initial_links,
            "the unobserved structure rewired during the match"
        );
    }

    #[test]
    fn falling_behind_is_absorbed_and_accelerates_the_collapse() {
        let mut world = CompetitiveFacility::authored();
        assert_eq!(world.purge_rate(), 0.0);
        for _ in 0..100_000 {
            world.advance_round(&[]);
            if world.absorbed_count() > 0 {
                break;
            }
        }
        assert!(world.absorbed_count() > 0, "a laggard is absorbed");
        assert!(
            world.purge_rate() > 0.0,
            "each absorbed team speeds the collapse"
        );
        // The fastest team is never the one absorbed.
        assert_eq!(world.team(TeamId(0)).unwrap().role, Role::Runner);
    }

    #[test]
    fn the_leader_escapes_and_is_never_absorbed() {
        let mut world = CompetitiveFacility::authored();
        run_to_finish(&mut world, &[]);
        let leader = world.team(TeamId(0)).unwrap();
        assert!(leader.placement.is_some());
        assert_eq!(leader.role, Role::Runner);
    }

    #[test]
    fn seizing_grants_the_sprint_but_forgoes_a_round() {
        // Team 3 (slowest) seizes once to grab the control, then sprints. With the
        // boost it makes more progress over the same rounds than it would untouched.
        let mut boosted = CompetitiveFacility::authored();
        boosted.advance_round(&[(TeamId(3), RaceAction::Seize)]);
        assert_eq!(boosted.control_holder, Some(TeamId(3)));
        for _ in 0..6 {
            boosted.advance_round(&[(TeamId(3), RaceAction::Advance)]);
        }

        let mut plain = CompetitiveFacility::authored();
        for _ in 0..7 {
            plain.advance_round(&[]);
        }
        assert!(
            boosted.progress_of(TeamId(3)) > plain.progress_of(TeamId(3)),
            "the sprint outpaces a plain run despite the seized round"
        );
    }

    #[test]
    fn interference_never_lowers_an_opponents_progress() {
        let mut world = CompetitiveFacility::authored();
        let mut previous: Vec<f32> = (0..TEAM_COUNT).map(|i| world.team_progress(i)).collect();
        for _ in 0..200 {
            // Team 0 keeps seizing the control; the others race.
            world.advance_round(&[(TeamId(0), RaceAction::Seize)]);
            for (i, &prev) in previous.iter().enumerate() {
                assert!(
                    world.team_progress(i) + 1e-6 >= prev,
                    "no team's progress may drop"
                );
            }
            previous = (0..TEAM_COUNT).map(|i| world.team_progress(i)).collect();
            if world.finished {
                break;
            }
        }
    }

    #[test]
    fn scramble_is_director_only_and_advances_the_collapse() {
        let mut world = CompetitiveFacility::authored();
        // A runner cannot scramble.
        assert!(!world.scramble(TeamId(0)));
        // Force a team into the director, then it can.
        world.teams[3].role = Role::Director;
        let before = world.purge_line;
        assert!(world.scramble(TeamId(3)));
        assert!(world.purge_line > before);
        assert_eq!(world.director_actions, 1);
    }

    #[test]
    fn the_match_is_deterministic() {
        let mut a = CompetitiveFacility::authored();
        let mut b = CompetitiveFacility::authored();
        run_to_finish(&mut a, &[]);
        run_to_finish(&mut b, &[]);
        assert_eq!(a.standings(), b.standings());
        assert_eq!(a.structure.graph.links, b.structure.graph.links);
        assert_eq!(a.winner, b.winner);
    }

    #[test]
    fn reset_restores_a_fresh_match() {
        let mut world = CompetitiveFacility::authored();
        run_to_finish(&mut world, &[]);
        world.reset();
        assert!(!world.finished);
        assert_eq!(world.slots_remaining, EXIT_CAPACITY);
        assert!(world.winner.is_none());
        assert_eq!(world.director_strength(), 0);
        assert!((0..TEAM_COUNT).all(|i| world.team_room(i).0 == START_ROOM));
        assert!(world.teams.iter().all(TeamRun::active_runner));
    }
}
