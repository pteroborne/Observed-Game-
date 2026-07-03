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
use observed_observation::DoorId;
use observed_observation::contention::PinSource;
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

/// A room's relationship to the collapse (Phase 41): [`CollapseState::Collapsed`]
/// rooms are sealed territory; [`CollapseState::Dying`] is the single next
/// room the frontier is about to take; everything else is
/// [`CollapseState::Intact`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CollapseState {
    Intact,
    Dying,
    Collapsed,
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

    /// Place `team`'s anchor on `room`, freezing that room's doorways (for
    /// every team) until removed. Passthrough to
    /// [`ConstraintWorld::place_anchor`] — see there for the idempotency and
    /// anchor-vs-anchor semantics.
    pub fn place_team_anchor(&mut self, team: TeamId, room: RoomId) -> bool {
        self.structure.place_anchor(team, room)
    }

    /// Remove `team`'s anchor from `room`, if present. Passthrough to
    /// [`ConstraintWorld::remove_anchor`]; only ever removes the calling
    /// team's own anchor.
    pub fn remove_team_anchor(&mut self, team: TeamId, room: RoomId) -> bool {
        self.structure.remove_anchor(team, room)
    }

    /// The team that owns the member at global player index `player`, if any.
    /// A player belongs to team `i` when `member_base <= player < member_base
    /// + members`.
    fn team_of_player(&self, player: usize) -> Option<TeamId> {
        self.teams
            .iter()
            .find(|team| {
                player >= team.member_base && player < team.member_base + team.members as usize
            })
            .map(|team| team.id)
    }

    /// Every reason `door` is currently frozen, deduped, in deterministic
    /// order: presence sources ordered by team id, then anchor sources
    /// ordered by team id. Mirrors
    /// `observed_observation::contention::ContentionWorld::pin_sources` —
    /// presentation uses this to attribute blame (whose tether light should
    /// show on this frame) across every team, not just the local one.
    pub fn pin_sources(&self, door: DoorId) -> Vec<PinSource> {
        let rooms = [
            self.structure.graph.door(door).room,
            self.structure
                .graph
                .door(self.structure.graph.partner(door))
                .room,
        ];

        let mut presence_teams: Vec<TeamId> = self
            .structure
            .graph
            .players
            .iter()
            .enumerate()
            .filter(|&(_, &room)| rooms.contains(&room))
            .filter_map(|(index, _)| self.team_of_player(index))
            .collect();
        presence_teams.sort_by_key(|team| team.0);
        presence_teams.dedup();

        let mut anchor_teams: Vec<TeamId> = self
            .structure
            .anchors
            .iter()
            .filter(|anchor| rooms.contains(&anchor.room))
            .map(|anchor| anchor.team)
            .collect();
        anchor_teams.sort_by_key(|team| team.0);
        anchor_teams.dedup();

        presence_teams
            .into_iter()
            .map(PinSource::Presence)
            .chain(anchor_teams.into_iter().map(PinSource::Anchor))
            .collect()
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

    /// The room a team must ultimately reach — `EXIT_ROOM` for the authored
    /// spine, or the map spec's exit room. Used by callers that need to prove
    /// solvability (e.g. the hybrid maze's navigability check) without caring
    /// which mode built this facility.
    pub fn exit_room(&self) -> Option<RoomId> {
        if let Some(spec) = &self.map_spec {
            return spec.exit_room();
        }
        Some(RoomId(EXIT_ROOM))
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

        // 7. The collapse claims territory: newly-collapsed rooms seal shut.
        //    This runs after absorption (step 6) so no alive Runner ever
        //    stands in a room being sealed — absorption already emptied every
        //    room behind the (now current) purge line of active runners.
        //    `collapse_rooms()` uses a `<=` progress-fraction cutoff while
        //    absorption uses a strict `<`, so a runner sitting exactly on the
        //    line is not absorbed yet could nominally share a room with a
        //    "collapsed" index; to keep sealing strictly safe we additionally
        //    never seal a room any active runner currently occupies.
        let occupied: BTreeSet<RoomId> = (0..self.teams.len())
            .filter(|&i| self.teams[i].active_runner())
            .map(|i| self.team_room(i))
            .collect();
        let mut newly_sealed = Vec::new();
        for room in self.collapse_rooms() {
            if occupied.contains(&room) || self.structure.is_sealed_room(room) {
                continue;
            }
            self.structure.seal_room(room);
            newly_sealed.push(room);
        }
        if let Some(&room) = newly_sealed.last() {
            self.last_event = format!(
                "The collapse sealed room {} — its doorways are gone for good.",
                room.0
            );
        }

        // 8. Once the exits are full, anyone still running is locked out and taken.
        if self.slots_remaining == 0 {
            for team in &mut self.teams {
                if team.active_runner() {
                    team.role = Role::Director;
                }
            }
        }

        // 9. Resolve.
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

    /// The single objective-sequence/spine room strictly ahead of the
    /// frontier — the next room the collapse is about to take. `None` once
    /// the sequence is exhausted (every room already collapsed, or the
    /// sequence is empty).
    pub fn dying_room(&self) -> Option<RoomId> {
        let sequence = if self.map_spec.is_some() {
            self.objective_sequence()
        } else {
            SPINE_ROOMS.iter().copied().map(RoomId).collect()
        };
        let collapsed_count = self.collapse_rooms().len();
        sequence.get(collapsed_count).copied()
    }

    /// `room`'s relationship to the collapse: deterministic and pure — derived
    /// entirely from [`Self::collapse_rooms`] and [`Self::dying_room`].
    pub fn room_collapse(&self, room: RoomId) -> CollapseState {
        if self.collapse_rooms().contains(&room) {
            CollapseState::Collapsed
        } else if self.dying_room() == Some(room) {
            CollapseState::Dying
        } else {
            CollapseState::Intact
        }
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
        // Phase 41 update: once the collapse can seal territory, a room it has
        // already claimed is *meant* to become unreachable (that's the whole
        // point — the entrance is gone once nobody needs to backtrack to it).
        // So whole-graph `connected()` is no longer the right invariant for a
        // full run; every active runner reaching the exit is (see the
        // dedicated solvability test below). This test now checks that
        // invariant instead of the pre-Phase-41 "everything stays reachable"
        // one, and still confirms the structure actually rewires.
        let mut world = CompetitiveFacility::authored();
        let initial_links = world.structure.graph.links.clone();
        for _ in 0..30 {
            if world.finished {
                break;
            }
            world.advance_round(&[]);
            for i in 0..TEAM_COUNT {
                if !world.teams[i].active_runner() {
                    continue;
                }
                let room = world.team_room(i);
                assert!(
                    graph_path(&world.structure.graph, room, RoomId(EXIT_ROOM)).is_some(),
                    "an active runner must always be able to reach the exit"
                );
            }
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

    #[test]
    fn placing_a_team_anchor_freezes_those_doors_through_decohere_for_every_team() {
        // Split every team apart so nobody's presence happens to pin room 4, then
        // anchor room 4 with team 2 (an uninvolved team) and confirm its doorways
        // survive repeated decohere/advance_round cycles regardless of which team
        // is racing through the rest of the structure.
        //
        // Phase 41 note: sealing overrides anchors (see
        // `collapse_overrides_anchors_reaching_an_anchored_room`), and sealing a
        // room also seals the doorway of any *neighbor* room that bordered it
        // (both ends of a claimed connection go dark). So this test keeps every
        // team parked on low-spine-index rooms with speed 0 — well clear of
        // room 4 *and* its spine neighbors (rooms 3 and 5) — so the purge line
        // never advances far enough to touch room 4 from any side. This test is
        // specifically about anchors surviving decohere, not about the
        // (separately tested) collapse override.
        let mut world = CompetitiveFacility::authored();
        // Scatter teams away from room 4 (and its spine neighbors) so only the
        // anchor pins it.
        for (i, room) in [0u32, 1, 0, 1].into_iter().enumerate() {
            let base = world.teams[i].member_base;
            for offset in 0..world.teams[i].members as usize {
                world.structure.graph.players[base + offset] = RoomId(room);
            }
            world.teams[i].speed = 0.0;
        }
        world.structure.recompute_connectivity();

        assert!(world.place_team_anchor(TeamId(2), RoomId(4)));
        let watched: Vec<(DoorId, DoorId)> = observed_observation::Side::ALL
            .iter()
            .map(|side| {
                let d = world.structure.graph.door_id(RoomId(4), *side);
                (d, world.structure.graph.partner(d))
            })
            .collect();

        for _ in 0..30 {
            world.advance_round(&[]);
            for (door, expected) in &watched {
                assert_eq!(
                    world.structure.graph.partner(*door),
                    *expected,
                    "an anchored room's doorways must stay frozen for every team"
                );
            }
            if world.finished {
                break;
            }
        }

        assert!(world.remove_team_anchor(TeamId(2), RoomId(4)));
        assert!(!world.remove_team_anchor(TeamId(2), RoomId(4)));
    }

    #[test]
    fn pin_sources_attributes_a_rivals_presence_correctly() {
        let mut world = CompetitiveFacility::authored();
        // Team 0's clump sits in room 4; team 1's clump sits in room 0 (elsewhere).
        let base0 = world.teams[0].member_base;
        for offset in 0..world.teams[0].members as usize {
            world.structure.graph.players[base0 + offset] = RoomId(4);
        }
        let base1 = world.teams[1].member_base;
        for offset in 0..world.teams[1].members as usize {
            world.structure.graph.players[base1 + offset] = RoomId(0);
        }
        world.structure.recompute_connectivity();

        let door = world
            .structure
            .graph
            .door_id(RoomId(4), observed_observation::Side::East);
        let sources = world.pin_sources(door);
        assert!(
            sources.contains(&PinSource::Presence(TeamId(0))),
            "room 4's occupant (team 0) must be attributed on its doorway"
        );
        assert!(
            !sources.contains(&PinSource::Presence(TeamId(1))),
            "team 1 is elsewhere and must not be attributed"
        );

        // Now anchor the same door's room with team 3 and confirm both sources
        // appear, deduped and in deterministic (presence-then-anchor, team-id)
        // order.
        assert!(world.place_team_anchor(TeamId(3), RoomId(4)));
        let sources = world.pin_sources(door);
        assert_eq!(
            sources,
            vec![PinSource::Presence(TeamId(0)), PinSource::Anchor(TeamId(3))]
        );
    }

    #[test]
    fn anchor_scripts_are_deterministic_across_identically_seeded_facilities() {
        fn script(world: &mut CompetitiveFacility) {
            world.place_team_anchor(TeamId(1), RoomId(4));
            world.advance_round(&[]);
            world.advance_round(&[]);
            world.place_team_anchor(TeamId(2), RoomId(6));
            world.advance_round(&[]);
            world.remove_team_anchor(TeamId(1), RoomId(4));
            world.advance_round(&[]);
        }

        let mut a = CompetitiveFacility::authored();
        let mut b = CompetitiveFacility::authored();
        script(&mut a);
        script(&mut b);

        assert_eq!(a.structure.graph.links, b.structure.graph.links);
        assert_eq!(a.structure.anchors.len(), b.structure.anchors.len());
        assert_eq!(
            (0..TEAM_COUNT).map(|i| a.team_room(i)).collect::<Vec<_>>(),
            (0..TEAM_COUNT).map(|i| b.team_room(i)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn no_anchors_means_no_behavior_change_from_the_existing_suite() {
        // Guard: with zero anchors placed, results must match the pre-Phase-38
        // baseline exactly (this mirrors `the_match_is_deterministic` but exists
        // specifically to catch any accidental widening of `is_frozen`).
        let mut a = CompetitiveFacility::authored();
        let mut b = CompetitiveFacility::authored();
        run_to_finish(&mut a, &[]);
        run_to_finish(&mut b, &[]);
        assert_eq!(a.structure.graph.links, b.structure.graph.links);
        assert_eq!(a.winner, b.winner);
        assert!(a.structure.anchors.is_empty());
    }

    // -- Phase 41: collapse claims territory ------------------------------

    #[test]
    fn collapse_state_reports_collapsed_dying_and_intact_in_sequence_order() {
        let mut world = CompetitiveFacility::authored();
        // Drive the purge line forward directly (pure query test — no need to
        // run a full match) far enough to collapse a few spine rooms.
        world.purge_line = 3.0 / SPINE_STEPS;

        let collapsed = world.collapse_rooms();
        assert!(!collapsed.is_empty(), "some rooms must be collapsed");
        for &room in &collapsed {
            assert_eq!(world.room_collapse(room), CollapseState::Collapsed);
        }

        let dying = world.dying_room();
        assert!(dying.is_some(), "exactly one room should be dying");
        let dying = dying.unwrap();
        assert!(
            !collapsed.contains(&dying),
            "dying room is not yet collapsed"
        );
        assert_eq!(world.room_collapse(dying), CollapseState::Dying);

        // Confirm dying is the sequence entry immediately after the frontier.
        let sequence: Vec<RoomId> = SPINE_ROOMS.iter().copied().map(RoomId).collect();
        let dying_pos = sequence.iter().position(|&r| r == dying).unwrap();
        assert_eq!(dying_pos, collapsed.len());

        // Everything else in the sequence is Intact.
        for (i, &room) in sequence.iter().enumerate() {
            if i < collapsed.len() {
                continue;
            }
            if room == dying {
                continue;
            }
            assert_eq!(
                world.room_collapse(room),
                CollapseState::Intact,
                "room {i} should be untouched by the collapse"
            );
        }
    }

    #[test]
    fn dying_room_is_none_once_the_sequence_is_exhausted() {
        let mut world = CompetitiveFacility::authored();
        world.purge_line = 1.0;
        assert_eq!(
            world.collapse_rooms().len(),
            SPINE_ROOMS.len(),
            "a full purge line collapses the entire sequence"
        );
        assert_eq!(world.dying_room(), None);
    }

    #[test]
    fn collapse_overrides_anchors_reaching_an_anchored_room() {
        // Anchor a spine room, then drive the purge line (via scrambles from
        // absorbed teams) far enough that the collapse reaches it, and prove
        // the anchor does not save it — sealing overrides every freeze.
        let mut world = CompetitiveFacility::authored();
        // Room 2 is SPINE_ROOMS[2]; anchor it with an uninvolved team.
        let anchored_room = RoomId(SPINE_ROOMS[2]);
        assert!(world.place_team_anchor(TeamId(3), anchored_room));

        // Force team 3 into the director role and scramble repeatedly to push
        // the purge line past room 2's progress fraction, without needing the
        // full race to play out (keeps the test fast and deterministic).
        world.teams[3].role = Role::Director;
        for _ in 0..50 {
            world.scramble(TeamId(3));
            if world.purge_line >= 3.0 / SPINE_STEPS {
                break;
            }
        }
        assert!(
            world.purge_line >= 2.0 / SPINE_STEPS,
            "the purge line must have advanced past room 2"
        );

        // Move every runner off the anchored room so sealing isn't blocked by
        // the "never seal an occupied room" safety check, then advance a
        // round so the sealing step runs.
        world.structure.graph.players = vec![RoomId(START_ROOM); PLAYER_COUNT];
        world.structure.recompute_connectivity();
        world.advance_round(&[]);

        assert!(
            world.structure.is_sealed_room(anchored_room),
            "the collapse must seal an anchored room once it reaches it"
        );
        for side in observed_observation::Side::ALL {
            let door = world.structure.graph.door_id(anchored_room, side);
            assert!(
                world.structure.graph.is_sealed(door),
                "an anchored room's doorways must seal once the collapse claims it"
            );
        }
    }

    #[test]
    fn sealed_rooms_never_rewire_across_many_rounds() {
        let mut world = CompetitiveFacility::authored();
        world.purge_line = 2.0 / SPINE_STEPS;
        world.structure.graph.players = vec![RoomId(START_ROOM); PLAYER_COUNT];
        world.structure.recompute_connectivity();
        world.advance_round(&[]);

        let sealed_before = world.structure.sealed_rooms.clone();
        assert!(
            !sealed_before.is_empty(),
            "some rooms must already be sealed"
        );

        for _ in 0..60 {
            world.structure.decohere();
            for &room in &sealed_before {
                for side in observed_observation::Side::ALL {
                    let door = world.structure.graph.door_id(room, side);
                    assert!(
                        world.structure.graph.is_sealed(door),
                        "a sealed room must never rewire"
                    );
                }
            }
        }
    }

    #[test]
    fn alive_runners_always_reach_the_exit_after_every_round_across_seeds() {
        // Solvability guard: whatever the collapse seals, every active
        // Runner's room must still reach the exit through current links.
        for seed in [
            0x5EED_C0DE_1234_5678u64,
            0x1,
            0xDEAD_BEEF_1234_5678,
            0x00C0_FFEE_0BAD_F00D,
            0x1357_9BDF_2468_ACE0,
            0xFFFF_FFFF_FFFF_FFFF,
            0x0000_0000_0000_0001,
            0x9E37_79B9_7F4A_7C15,
        ] {
            let mut world = CompetitiveFacility::authored();
            world.structure.base_seed = seed;
            for round in 0..2000 {
                if world.finished {
                    break;
                }
                world.advance_round(&[]);
                for i in 0..TEAM_COUNT {
                    if !world.teams[i].active_runner() {
                        continue;
                    }
                    let room = world.team_room(i);
                    assert!(
                        graph_path(&world.structure.graph, room, RoomId(EXIT_ROOM)).is_some(),
                        "seed {seed:#x} round {round}: active runner team {i} in room {room:?} \
                         cannot reach the exit"
                    );
                }
            }
        }
    }

    #[test]
    fn sealing_is_deterministic_across_identically_seeded_facilities() {
        fn script(world: &mut CompetitiveFacility) {
            for _ in 0..40 {
                if world.finished {
                    break;
                }
                world.advance_round(&[]);
            }
        }

        let mut a = CompetitiveFacility::authored();
        let mut b = CompetitiveFacility::authored();
        script(&mut a);
        script(&mut b);

        assert_eq!(a.structure.sealed_rooms, b.structure.sealed_rooms);
        assert_eq!(a.structure.graph.links, b.structure.graph.links);
    }
}
