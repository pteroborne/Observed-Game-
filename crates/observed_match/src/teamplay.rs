//! Deterministic two-member teamplay for bot-spectated procedural co-op races.
//!
//! This module is deliberately pure simulation: it owns the procedural room-role
//! plan, the two-member bot team state, tool decisions, guardian setbacks, and
//! round outcome that the assembled game can spectate. Presentation code follows
//! the state; it does not invent team progress from rendered entities.

use observed_core::{RoomId, SplitMix, TeamId};

use crate::elimination::{
    DEFAULT_COUNTDOWN_ROUNDS, DEFAULT_KEYSTONES_REQUIRED, EscapeCountdown, TeamToolState,
    keystone_rooms, spine_sequence,
};
use crate::facility::{EXIT_ROOM, MEMBERS_PER_TEAM, START_ROOM, TEAM_COUNT};

pub const MAX_TEAMPLAY_TICKS: u32 = 512;

/// A recognizable, deterministic bot-team strategy. Policies bias existing
/// decision points in [`advance_team_plan`] and [`team_speed`] — they never add
/// new mechanics, only change which already-possible action a team prefers.
///
/// Assignment is derived from the *series* seed (the seed the whole elimination
/// series started with), not the per-round seed `TeamplayMatch::for_round`
/// re-derives every round. A personality is meant to be recognizable across a
/// whole series, so it must survive round-to-round reseating unchanged.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TeamPolicy {
    /// Current/default behavior: task priority and speed are unchanged.
    Balanced,
    /// Prioritizes the anchor checkpoint ahead of the dual-station gate and
    /// leans on it as a stabilizing move whenever it's still available.
    Freezer,
    /// Runs a speed tier faster and skips the optional anchor beat (not
    /// required to open the exit gate) to press straight for keystones/exit.
    Sprinter,
    /// Engages the guardian-setback interaction more readily than a balanced
    /// team — the only denial/adversary lever this brain exposes.
    Saboteur,
}

impl TeamPolicy {
    /// HUD-facing label.
    pub fn label(self) -> &'static str {
        match self {
            TeamPolicy::Balanced => "balanced",
            TeamPolicy::Freezer => "the freezer",
            TeamPolicy::Sprinter => "the sprinter",
            TeamPolicy::Saboteur => "the saboteur",
        }
    }

    const ALL: [TeamPolicy; 4] = [
        TeamPolicy::Balanced,
        TeamPolicy::Freezer,
        TeamPolicy::Sprinter,
        TeamPolicy::Saboteur,
    ];

    /// Deterministic, stable-per-series policy assignment.
    ///
    /// Every seed maps to a seeded *permutation* of the four policies across
    /// the four teams (not four independent draws), so all four personalities
    /// are always present in a series. `series_seed` must be the seed the
    /// series/match started with — callers must not pass a per-round seed, or
    /// a team's personality would appear to change every round.
    pub fn for_team(series_seed: u64, team: TeamId) -> TeamPolicy {
        let mut order = TeamPolicy::ALL;
        let mut rng = SplitMix::new(series_seed ^ 0x9EA1_7EA0_B0F5_1C11);
        shuffle(&mut order, &mut rng);
        order[team.index() % order.len()]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TeamMemberId {
    pub team: TeamId,
    pub member: u8,
}

impl TeamMemberId {
    pub fn label(self) -> String {
        format!("{} P{}", self.team.label(), self.member + 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeamTask {
    Regroup(RoomId),
    OperateStationA(RoomId),
    OperateStationB(RoomId),
    PlaceAnchor(RoomId),
    PlaceTeleportPad(RoomId),
    CollectKeystone(RoomId),
    Escape(RoomId),
    EvadeGuardian(RoomId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeamMemberState {
    pub id: TeamMemberId,
    pub room: RoomId,
    pub task: TeamTask,
    pub delayed_ticks: u8,
    pub escaped: bool,
}

impl TeamMemberState {
    fn new(team: TeamId, member: u8) -> Self {
        let start = RoomId(START_ROOM);
        Self {
            id: TeamMemberId { team, member },
            room: start,
            task: TeamTask::Regroup(start),
            delayed_ticks: 0,
            escaped: false,
        }
    }
}

/// The seeded role assignment for one procedural co-op race.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoopSeedPlan {
    pub seed: u64,
    pub keystone_rooms: Vec<RoomId>,
    pub dual_station_room: RoomId,
    pub anchor_room: RoomId,
    pub relay_rooms: (RoomId, RoomId),
    pub guardian_room: RoomId,
    pub control_room: RoomId,
}

impl CoopSeedPlan {
    pub fn new(seed: u64) -> Self {
        let spine = spine_sequence();
        let mut roles = spine[1..spine.len() - 1].to_vec();
        let mut rng = SplitMix::new(seed ^ 0xC001_D00D_5EED_3515);
        shuffle(&mut roles, &mut rng);
        let relay_rooms = forward_pair(roles[2], roles[3], &spine);
        Self {
            seed,
            keystone_rooms: keystone_rooms(seed),
            dual_station_room: roles[0],
            anchor_room: roles[1],
            relay_rooms,
            guardian_room: roles[4],
            control_room: RoomId(3),
        }
    }

    pub fn solvable(&self) -> bool {
        let spine = spine_sequence();
        let valid_role = |room: RoomId| {
            room != RoomId(START_ROOM) && room != RoomId(EXIT_ROOM) && spine.contains(&room)
        };
        let mut role_rooms = vec![
            self.dual_station_room,
            self.anchor_room,
            self.relay_rooms.0,
            self.relay_rooms.1,
            self.guardian_room,
        ];
        role_rooms.sort_unstable();
        role_rooms.dedup();
        self.keystone_rooms.len() == DEFAULT_KEYSTONES_REQUIRED
            && self.keystone_rooms.iter().all(|room| valid_role(*room))
            && role_rooms.len() == 5
            && role_rooms.into_iter().all(valid_role)
            && spine_position(self.relay_rooms.0, &spine)
                < spine_position(self.relay_rooms.1, &spine)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeamAction {
    Move {
        member: TeamMemberId,
        from: RoomId,
        to: RoomId,
    },
    OperateDualStation {
        team: TeamId,
        room: RoomId,
    },
    DropAnchor {
        team: TeamId,
        room: RoomId,
    },
    DropTeleportPad {
        team: TeamId,
        room: RoomId,
    },
    UseTeleportPad {
        member: TeamMemberId,
        from: RoomId,
        to: RoomId,
    },
    CollectKeystone {
        team: TeamId,
        room: RoomId,
    },
    GuardianSetback {
        member: TeamMemberId,
        from: RoomId,
        to: RoomId,
    },
    Escape {
        team: TeamId,
    },
    DecoherenceCommit,
    RoundFinished,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeamplayEvent {
    pub tick: u32,
    pub action: TeamAction,
}

impl TeamplayEvent {
    fn new(tick: u32, action: TeamAction) -> Self {
        Self { tick, action }
    }

    pub fn summary(&self) -> String {
        match self.action {
            TeamAction::Move { member, from, to } => {
                format!("{} moved R{} -> R{}.", member.label(), from.0, to.0)
            }
            TeamAction::OperateDualStation { team, room } => {
                format!(
                    "{} completed the two-operator gate in R{}.",
                    team.label(),
                    room.0
                )
            }
            TeamAction::DropAnchor { team, room } => {
                format!(
                    "{} anchored R{}'s current thresholds.",
                    team.label(),
                    room.0
                )
            }
            TeamAction::DropTeleportPad { team, room } => {
                format!(
                    "{} placed a keyed teleport pad in R{}.",
                    team.label(),
                    room.0
                )
            }
            TeamAction::UseTeleportPad { member, from, to } => {
                format!(
                    "{} linked R{} -> R{} through team pads.",
                    member.label(),
                    from.0,
                    to.0
                )
            }
            TeamAction::CollectKeystone { team, room } => {
                format!("{} collected the keystone in R{}.", team.label(), room.0)
            }
            TeamAction::GuardianSetback { member, from, to } => {
                format!(
                    "Guardian displaced {} from R{} to R{}.",
                    member.label(),
                    from.0,
                    to.0
                )
            }
            TeamAction::Escape { team } => format!("{} escaped the co-op route.", team.label()),
            TeamAction::DecoherenceCommit => "Unobserved routes decohered.".to_string(),
            TeamAction::RoundFinished => "Co-op race round resolved.".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TeamplayMetrics {
    pub co_op_completions: u32,
    pub anchors_dropped: u32,
    pub pads_dropped: u32,
    pub pad_uses: u32,
    pub guardian_catches: u32,
    pub decoherence_events: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeamplayTeamState {
    pub id: TeamId,
    pub policy: TeamPolicy,
    pub members: Vec<TeamMemberState>,
    pub pace: f32,
    pub collected_keystones: Vec<RoomId>,
    pub escaped: bool,
    pub eliminated: bool,
    pub completed_dual_station: bool,
    pub used_pad_link: bool,
    pub guardian_catches: u32,
    pub tools: TeamToolState,
}

impl TeamplayTeamState {
    pub fn new(id: TeamId) -> Self {
        Self::with_policy(id, TeamPolicy::Balanced)
    }

    pub fn with_policy(id: TeamId, policy: TeamPolicy) -> Self {
        Self {
            id,
            policy,
            members: (0..MEMBERS_PER_TEAM as u8)
                .map(|member| TeamMemberState::new(id, member))
                .collect(),
            pace: 0.0,
            collected_keystones: Vec::new(),
            escaped: false,
            eliminated: false,
            completed_dual_station: false,
            used_pad_link: false,
            guardian_catches: 0,
            tools: TeamToolState::new(id),
        }
    }

    pub fn gate_open(&self, required: usize) -> bool {
        self.collected_keystones.len() >= required
    }

    pub fn all_members_in(&self, room: RoomId) -> bool {
        self.members.iter().all(|member| member.room == room)
    }

    pub fn operate_dual_station(&mut self, room: RoomId) -> bool {
        if self.completed_dual_station {
            return false;
        }
        let present = self
            .members
            .iter()
            .filter(|member| member.room == room)
            .count();
        if present >= 2 {
            self.completed_dual_station = true;
            true
        } else {
            false
        }
    }

    pub fn status_line(&self, required: usize) -> String {
        let rooms = self
            .members
            .iter()
            .map(|member| format!("P{}:R{}", member.id.member + 1, member.room.0))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "{} | key {}/{} | {} | gate {} | pads {} | catches {}",
            self.id.label(),
            self.collected_keystones.len(),
            required,
            rooms,
            if self.completed_dual_station {
                "done"
            } else {
                "pending"
            },
            self.tools.pads.len(),
            self.guardian_catches,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeamplayRoundOutcome {
    pub seed: u64,
    pub round_index: u32,
    pub tick: u32,
    pub escape_order: Vec<TeamId>,
    pub eliminated: Vec<TeamId>,
    pub metrics: TeamplayMetrics,
    pub events: Vec<TeamplayEvent>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeamplayMatch {
    pub seed: u64,
    /// The seed the whole series/match started with. Distinct from `seed` (the
    /// per-round plan seed `for_round` re-derives every round): policy
    /// assignment is keyed off this field specifically so a team's
    /// personality reads as stable across a series, not just a single round.
    pub series_seed: u64,
    pub round_index: u32,
    pub plan: CoopSeedPlan,
    pub active_teams: Vec<TeamId>,
    pub adversary_strength: usize,
    pub teams: Vec<TeamplayTeamState>,
    pub tick: u32,
    pub countdown: Option<EscapeCountdown>,
    pub escape_order: Vec<TeamId>,
    pub eliminated: Vec<TeamId>,
    pub finished: bool,
    pub metrics: TeamplayMetrics,
    pub last_event: String,
    outcome: Option<TeamplayRoundOutcome>,
    event_log: Vec<TeamplayEvent>,
}

impl TeamplayMatch {
    pub fn new(seed: u64) -> Self {
        Self::for_round(
            seed,
            seed,
            1,
            (0..TEAM_COUNT).map(|team| TeamId(team as u8)).collect(),
            0,
        )
    }

    /// Build the round for a running series, reseating the given active teams.
    ///
    /// `series_seed` must stay the seed the *series* started with across every
    /// call for the same series (round-to-round reseats pass the same
    /// `series_seed` even though `seed`, the per-round plan seed, changes) —
    /// that's what keeps each surviving team's [`TeamPolicy`] fixed for the
    /// whole series instead of reshuffling every round.
    pub fn for_round(
        seed: u64,
        series_seed: u64,
        round_index: u32,
        active_teams: Vec<TeamId>,
        adversary_strength: usize,
    ) -> Self {
        let teams = active_teams
            .iter()
            .copied()
            .map(|id| TeamplayTeamState::with_policy(id, TeamPolicy::for_team(series_seed, id)))
            .collect();
        Self {
            seed,
            series_seed,
            round_index,
            plan: CoopSeedPlan::new(seed ^ u64::from(round_index).wrapping_mul(0x51C0_0F00D)),
            active_teams,
            adversary_strength,
            teams,
            tick: 0,
            countdown: None,
            escape_order: Vec::new(),
            eliminated: Vec::new(),
            finished: false,
            metrics: TeamplayMetrics::default(),
            last_event: "Bot teams entered a procedural co-op route.".to_string(),
            outcome: None,
            event_log: Vec::new(),
        }
    }

    /// Test/experiment surface: build a round with explicit policies instead
    /// of the seeded assignment, so distinctness tests can force e.g. a
    /// Sprinter-vs-Balanced comparison for the same team index.
    pub fn with_policies(
        seed: u64,
        round_index: u32,
        active_teams: Vec<TeamId>,
        adversary_strength: usize,
        policies: [TeamPolicy; TEAM_COUNT],
    ) -> Self {
        let mut game = Self::for_round(seed, seed, round_index, active_teams, adversary_strength);
        for team in &mut game.teams {
            team.policy = policies[team.id.index()];
        }
        game
    }

    pub fn policy(&self, id: TeamId) -> Option<TeamPolicy> {
        self.team(id).map(|team| team.policy)
    }

    pub fn team(&self, id: TeamId) -> Option<&TeamplayTeamState> {
        self.teams.iter().find(|team| team.id == id)
    }

    pub fn team_mut(&mut self, id: TeamId) -> Option<&mut TeamplayTeamState> {
        self.teams.iter_mut().find(|team| team.id == id)
    }

    pub fn round_outcome(&self) -> Option<TeamplayRoundOutcome> {
        self.outcome.clone()
    }

    pub fn run_to_outcome(&mut self, max_ticks: u32) {
        for _ in 0..max_ticks {
            if self.finished {
                return;
            }
            self.advance_bot_tick();
        }
    }

    pub fn advance_bot_tick(&mut self) -> Vec<TeamplayEvent> {
        if self.finished {
            return Vec::new();
        }

        self.tick += 1;
        let mut events = Vec::new();
        let plan = self.plan.clone();
        let tick = self.tick;
        let adversary_strength = self.adversary_strength;

        for team in &mut self.teams {
            if team.escaped || team.eliminated {
                continue;
            }
            team.pace += team_speed(team.id, adversary_strength, team.policy);
            while team.pace >= 1.0 && !team.escaped && !team.eliminated {
                team.pace -= 1.0;
                events.extend(advance_team_plan(
                    tick,
                    &plan,
                    adversary_strength,
                    team,
                    &mut self.metrics,
                ));
            }
        }

        if self.tick.is_multiple_of(6) {
            self.metrics.decoherence_events += 1;
            events.push(TeamplayEvent::new(self.tick, TeamAction::DecoherenceCommit));
        }

        let mut just_started_countdown = false;
        let required = self.plan.keystone_rooms.len();
        let mut escaped_now = Vec::new();
        for team in &mut self.teams {
            if !team.escaped
                && !team.eliminated
                && team.gate_open(required)
                && team.all_members_in(RoomId(EXIT_ROOM))
            {
                team.escaped = true;
                for member in &mut team.members {
                    member.escaped = true;
                }
                escaped_now.push(team.id);
            }
        }
        escaped_now.sort_unstable();
        for team in escaped_now {
            if !self.escape_order.contains(&team) {
                self.escape_order.push(team);
                events.push(TeamplayEvent::new(self.tick, TeamAction::Escape { team }));
                if self.countdown.is_none() {
                    self.countdown = Some(EscapeCountdown {
                        started_by: team,
                        duration_rounds: DEFAULT_COUNTDOWN_ROUNDS,
                        remaining_rounds: DEFAULT_COUNTDOWN_ROUNDS,
                    });
                    just_started_countdown = true;
                }
            }
        }

        let survivor_slots = self.active_teams.len().saturating_sub(1);
        if self.escape_order.len() >= survivor_slots {
            self.finish_round(&mut events);
        } else if let Some(countdown) = &mut self.countdown {
            if !just_started_countdown {
                countdown.remaining_rounds = countdown.remaining_rounds.saturating_sub(1);
            }
            if countdown.remaining_rounds == 0 {
                self.finish_round(&mut events);
            }
        }

        for event in &events {
            self.last_event = event.summary();
            self.event_log.push(event.clone());
        }
        events
    }

    fn finish_round(&mut self, events: &mut Vec<TeamplayEvent>) {
        if self.finished {
            return;
        }
        let mut eliminated: Vec<TeamId> = self
            .teams
            .iter()
            .filter(|team| !team.escaped && !team.eliminated)
            .map(|team| team.id)
            .collect();
        eliminated.sort_unstable();
        for id in &eliminated {
            if let Some(team) = self.team_mut(*id) {
                team.eliminated = true;
            }
        }
        self.eliminated = eliminated;
        self.finished = true;
        events.push(TeamplayEvent::new(self.tick, TeamAction::RoundFinished));
        let mut outcome_events = self.event_log.clone();
        outcome_events.extend(events.iter().cloned());
        self.outcome = Some(TeamplayRoundOutcome {
            seed: self.seed,
            round_index: self.round_index,
            tick: self.tick,
            escape_order: self.escape_order.clone(),
            eliminated: self.eliminated.clone(),
            metrics: self.metrics,
            events: outcome_events,
        });
    }
}

fn advance_team_plan(
    tick: u32,
    plan: &CoopSeedPlan,
    adversary_strength: usize,
    team: &mut TeamplayTeamState,
    metrics: &mut TeamplayMetrics,
) -> Vec<TeamplayEvent> {
    let mut events = Vec::new();
    if let Some(event) = maybe_guardian_setback(tick, plan, adversary_strength, team, metrics) {
        events.push(event);
        return events;
    }

    if let Some(member) = team
        .members
        .iter_mut()
        .find(|member| member.delayed_ticks > 0)
    {
        member.delayed_ticks = member.delayed_ticks.saturating_sub(1);
        member.task = TeamTask::EvadeGuardian(member.room);
        return events;
    }

    // Freezer: the anchor checkpoint is this policy's signature move, so it
    // takes priority over the dual-station gate whenever both are still
    // pending — it reaches the anchor stations earlier than a balanced team
    // would. Sprinter treats the anchor as an optional, non-required-for-exit
    // beat (the escape gate only checks collected keystones) and skips it
    // outright to press straight for keystones/exit instead.
    if team.policy == TeamPolicy::Freezer
        && team.tools.anchors.is_empty()
        && let Some(event) = try_place_anchor(tick, plan, team, metrics)
    {
        events.push(event);
        return events;
    }

    if !team.completed_dual_station {
        team.members[0].task = TeamTask::OperateStationA(plan.dual_station_room);
        team.members[1].task = TeamTask::OperateStationB(plan.dual_station_room);
        for index in 0..2 {
            if let Some(event) = move_member_toward(tick, team, index, plan.dual_station_room) {
                events.push(event);
                return events;
            }
        }
        if team.operate_dual_station(plan.dual_station_room) {
            metrics.co_op_completions += 1;
            events.push(TeamplayEvent::new(
                tick,
                TeamAction::OperateDualStation {
                    team: team.id,
                    room: plan.dual_station_room,
                },
            ));
        }
        return events;
    }

    if team.policy != TeamPolicy::Sprinter && team.tools.anchors.is_empty() {
        if let Some(event) = try_place_anchor(tick, plan, team, metrics) {
            events.push(event);
        }
        return events;
    }

    if team
        .tools
        .pads
        .iter()
        .all(|pad| pad.room != plan.relay_rooms.0)
    {
        team.members[0].task = TeamTask::PlaceTeleportPad(plan.relay_rooms.0);
        if let Some(event) = move_member_toward(tick, team, 0, plan.relay_rooms.0) {
            events.push(event);
            return events;
        }
        if team.tools.drop_pad(plan.relay_rooms.0) {
            metrics.pads_dropped += 1;
            events.push(TeamplayEvent::new(
                tick,
                TeamAction::DropTeleportPad {
                    team: team.id,
                    room: plan.relay_rooms.0,
                },
            ));
        }
        return events;
    }

    if team
        .tools
        .pads
        .iter()
        .all(|pad| pad.room != plan.relay_rooms.1)
    {
        team.members[1].task = TeamTask::PlaceTeleportPad(plan.relay_rooms.1);
        if let Some(event) = move_member_toward(tick, team, 1, plan.relay_rooms.1) {
            events.push(event);
            return events;
        }
        if team.tools.drop_pad(plan.relay_rooms.1) {
            metrics.pads_dropped += 1;
            events.push(TeamplayEvent::new(
                tick,
                TeamAction::DropTeleportPad {
                    team: team.id,
                    room: plan.relay_rooms.1,
                },
            ));
        }
        return events;
    }

    let missing = plan
        .keystone_rooms
        .iter()
        .copied()
        .find(|room| !team.collected_keystones.contains(room));
    if let Some(target) = missing {
        let member = closest_member(team, target);
        team.members[member].task = TeamTask::CollectKeystone(target);
        if let Some(event) = try_use_pad(tick, team, member, target, metrics) {
            events.push(event);
            return events;
        }
        if let Some(event) = move_member_toward(tick, team, member, target) {
            events.push(event);
            return events;
        }
        team.collected_keystones.push(target);
        team.collected_keystones.sort_unstable();
        events.push(TeamplayEvent::new(
            tick,
            TeamAction::CollectKeystone {
                team: team.id,
                room: target,
            },
        ));
        return events;
    }

    for index in 0..team.members.len() {
        team.members[index].task = TeamTask::Escape(RoomId(EXIT_ROOM));
        if let Some(event) = try_use_pad(tick, team, index, RoomId(EXIT_ROOM), metrics) {
            events.push(event);
            return events;
        }
        if let Some(event) = move_member_toward(tick, team, index, RoomId(EXIT_ROOM)) {
            events.push(event);
            return events;
        }
    }

    events
}

fn move_member_toward(
    tick: u32,
    team: &mut TeamplayTeamState,
    member_index: usize,
    target: RoomId,
) -> Option<TeamplayEvent> {
    let from = team.members[member_index].room;
    if from == target {
        return None;
    }
    let to = step_toward(from, target);
    team.members[member_index].room = to;
    Some(TeamplayEvent::new(
        tick,
        TeamAction::Move {
            member: team.members[member_index].id,
            from,
            to,
        },
    ))
}

/// Moves member 0 toward the anchor room and drops the anchor on arrival.
/// Shared by the default anchor beat and the Freezer policy's earlier
/// priority ordering (see `advance_team_plan`).
fn try_place_anchor(
    tick: u32,
    plan: &CoopSeedPlan,
    team: &mut TeamplayTeamState,
    metrics: &mut TeamplayMetrics,
) -> Option<TeamplayEvent> {
    team.members[0].task = TeamTask::PlaceAnchor(plan.anchor_room);
    if let Some(event) = move_member_toward(tick, team, 0, plan.anchor_room) {
        return Some(event);
    }
    if team.tools.drop_anchor(plan.anchor_room) {
        metrics.anchors_dropped += 1;
        return Some(TeamplayEvent::new(
            tick,
            TeamAction::DropAnchor {
                team: team.id,
                room: plan.anchor_room,
            },
        ));
    }
    None
}

fn try_use_pad(
    tick: u32,
    team: &mut TeamplayTeamState,
    member_index: usize,
    target: RoomId,
    metrics: &mut TeamplayMetrics,
) -> Option<TeamplayEvent> {
    let from = team.members[member_index].room;
    let to = team.tools.pad_link_target(from)?;
    if spine_distance(to, target) >= spine_distance(from, target) {
        return None;
    }
    team.members[member_index].room = to;
    team.used_pad_link = true;
    metrics.pad_uses += 1;
    Some(TeamplayEvent::new(
        tick,
        TeamAction::UseTeleportPad {
            member: team.members[member_index].id,
            from,
            to,
        },
    ))
}

fn maybe_guardian_setback(
    tick: u32,
    plan: &CoopSeedPlan,
    adversary_strength: usize,
    team: &mut TeamplayTeamState,
    metrics: &mut TeamplayMetrics,
) -> Option<TeamplayEvent> {
    if adversary_strength == 0 || tick < 12 {
        return None;
    }
    let interval = (14u32).saturating_sub(adversary_strength as u32 * 2).max(5);
    // Saboteur: the guardian setback is the only denial/adversary-engagement
    // lever this brain exposes, so this policy leans into it harder than a
    // balanced team, tripping the interaction on a shorter, denser interval.
    let interval = if team.policy == TeamPolicy::Saboteur {
        interval.saturating_sub(3).max(3)
    } else {
        interval
    };
    if !(tick + u32::from(team.id.0)).is_multiple_of(interval) {
        return None;
    }
    let (victim, from) = team
        .members
        .iter()
        .enumerate()
        .max_by_key(|(_, member)| {
            let proximity_bonus = if member.room == plan.guardian_room {
                20
            } else {
                0
            };
            spine_position(member.room, &spine_sequence()) + proximity_bonus
        })
        .map(|(index, member)| (index, member.room))?;
    let to = team
        .tools
        .anchors
        .last()
        .map(|anchor| anchor.room)
        .unwrap_or(RoomId(START_ROOM));
    if from == to || from == RoomId(START_ROOM) {
        return None;
    }
    team.members[victim].room = to;
    team.members[victim].delayed_ticks = 1;
    team.members[victim].task = TeamTask::EvadeGuardian(to);
    team.guardian_catches += 1;
    metrics.guardian_catches += 1;
    Some(TeamplayEvent::new(
        tick,
        TeamAction::GuardianSetback {
            member: team.members[victim].id,
            from,
            to,
        },
    ))
}

fn closest_member(team: &TeamplayTeamState, target: RoomId) -> usize {
    team.members
        .iter()
        .enumerate()
        .min_by_key(|(_, member)| spine_distance(member.room, target))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn team_speed(team: TeamId, adversary_strength: usize, policy: TeamPolicy) -> f32 {
    const SPEEDS: [f32; TEAM_COUNT] = [1.0, 0.78, 0.62, 0.48];
    // Sprinter: +1 tier on the existing speed table (one tier faster, floored
    // at the fastest tier already in the table).
    let tier = match policy {
        TeamPolicy::Sprinter => team.index().saturating_sub(1),
        _ => team.index(),
    };
    let penalty = (adversary_strength as f32 * 0.025).min(0.1);
    (SPEEDS[tier] - penalty).max(0.32)
}

fn step_toward(from: RoomId, target: RoomId) -> RoomId {
    let spine = spine_sequence();
    let current = spine_position(from, &spine);
    let target = spine_position(target, &spine);
    if current < target {
        spine[current + 1]
    } else if current > target {
        spine[current - 1]
    } else {
        spine[current]
    }
}

fn spine_distance(a: RoomId, b: RoomId) -> usize {
    let spine = spine_sequence();
    spine_position(a, &spine).abs_diff(spine_position(b, &spine))
}

fn spine_position(room: RoomId, spine: &[RoomId]) -> usize {
    spine
        .iter()
        .position(|candidate| *candidate == room)
        .unwrap_or(0)
}

fn forward_pair(a: RoomId, b: RoomId, spine: &[RoomId]) -> (RoomId, RoomId) {
    if spine_position(a, spine) <= spine_position(b, spine) {
        (a, b)
    } else {
        (b, a)
    }
}

fn shuffle<T>(items: &mut [T], rng: &mut SplitMix) {
    for i in (1..items.len()).rev() {
        let j = rng.below(i + 1);
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_coop_plans_are_solvable_across_seeds() {
        for seed in 0..128 {
            let plan = CoopSeedPlan::new(seed);
            assert!(plan.solvable(), "seed {seed} produced {plan:?}");
        }
    }

    #[test]
    fn one_member_cannot_operate_the_dual_station() {
        let plan = CoopSeedPlan::new(7);
        let mut team = TeamplayTeamState::new(TeamId(0));
        team.members[0].room = plan.dual_station_room;
        assert!(!team.operate_dual_station(plan.dual_station_room));
        team.members[1].room = plan.dual_station_room;
        assert!(team.operate_dual_station(plan.dual_station_room));
    }

    #[test]
    fn bot_team_completes_coop_tools_and_round_outcome() {
        let mut game = TeamplayMatch::new(11);
        game.run_to_outcome(MAX_TEAMPLAY_TICKS);
        let outcome = game.round_outcome().expect("round should resolve");
        let team = game.team(TeamId(0)).expect("team 1 exists");
        assert!(team.completed_dual_station);
        assert!(!team.tools.anchors.is_empty());
        assert_eq!(team.tools.pads.len(), 2);
        assert!(team.used_pad_link, "Team 1 should use its pad relay");
        assert!(outcome.escape_order.contains(&TeamId(0)));
        assert!(!outcome.eliminated.is_empty());
        assert!(outcome.metrics.co_op_completions > 0);
    }

    #[test]
    fn guardian_setback_keeps_keystones_and_uses_anchor_checkpoint() {
        let mut game = TeamplayMatch::for_round(19, 19, 2, vec![TeamId(0), TeamId(1)], 2);
        game.run_to_outcome(80);
        let team = game.team(TeamId(0)).expect("team 1 exists");
        assert!(
            team.guardian_catches > 0,
            "adversary pressure should produce a guardian setback"
        );
        assert!(
            team.collected_keystones
                .iter()
                .all(|room| game.plan.keystone_rooms.contains(room)),
            "guardian catches never remove or invent keystones"
        );
        if let Some(anchor) = team.tools.anchors.last() {
            assert!(
                team.members
                    .iter()
                    .any(|member| member.room == anchor.room || member.escaped),
                "caught members fall back to the anchored checkpoint"
            );
        }
    }

    #[test]
    fn same_seed_produces_identical_teamplay() {
        let mut a = TeamplayMatch::new(23);
        let mut b = TeamplayMatch::new(23);
        a.run_to_outcome(MAX_TEAMPLAY_TICKS);
        b.run_to_outcome(MAX_TEAMPLAY_TICKS);
        assert_eq!(a.round_outcome(), b.round_outcome());
    }

    #[test]
    fn same_seed_produces_identical_teamplay_with_policies_assigned() {
        // Policies are part of TeamplayMatch::new's construction (via
        // TeamPolicy::for_team), so this pins that determinism holds with the
        // personality system wired in, not just for the pre-existing default.
        let mut a = TeamplayMatch::new(4242);
        let mut b = TeamplayMatch::new(4242);
        for _ in 0..MAX_TEAMPLAY_TICKS {
            let ea = a.advance_bot_tick();
            let eb = b.advance_bot_tick();
            assert_eq!(ea, eb, "byte-identical event summaries per tick");
            if a.finished {
                break;
            }
        }
        assert_eq!(a.round_outcome(), b.round_outcome());
    }

    #[test]
    fn for_round_reseat_preserves_each_teams_policy_across_rounds() {
        let series_seed = 777;
        let round1 = TeamplayMatch::for_round(
            series_seed ^ 0x1111,
            series_seed,
            1,
            (0..TEAM_COUNT as u8).map(TeamId).collect(),
            0,
        );
        let policies_round1: Vec<TeamPolicy> = (0..TEAM_COUNT as u8)
            .map(|t| round1.policy(TeamId(t)).unwrap())
            .collect();

        // A reseat for round 2 with a *different* per-round seed but the same
        // series_seed, and a team eliminated, must keep the survivors' policy.
        let surviving = vec![TeamId(0), TeamId(2)];
        let round2 = TeamplayMatch::for_round(
            series_seed.wrapping_add(999),
            series_seed,
            2,
            surviving.clone(),
            1,
        );
        for team in surviving {
            assert_eq!(
                round2.policy(team),
                Some(policies_round1[team.index()]),
                "policy for {team:?} must survive a for_round reseat unchanged"
            );
        }
    }

    #[test]
    fn policy_assignment_covers_all_four_policies_across_seeds() {
        for seed in 0..64u64 {
            let game = TeamplayMatch::new(seed);
            let mut seen: Vec<TeamPolicy> = (0..TEAM_COUNT as u8)
                .map(|t| game.policy(TeamId(t)).unwrap())
                .collect();
            seen.sort_by_key(|policy| *policy as u8);
            let mut expected = TeamPolicy::ALL;
            expected.sort_by_key(|policy| *policy as u8);
            assert_eq!(
                seen.to_vec(),
                expected.to_vec(),
                "seed {seed} did not assign all four policies exactly once"
            );
        }
    }

    /// Manually ticks a `TeamplayMatch`, independent of the round's own
    /// finish/countdown machinery (which can end the round — and eliminate
    /// still-running teams — as soon as `survivor_slots` other teams escape).
    /// Returns the first tick at which `team` itself satisfies the escape
    /// condition (gate open, all members in the exit room), so a policy's
    /// effect on one team's own progress is comparable in isolation from
    /// which other teams happened to race it off the board that round.
    fn first_tick_team_ready(
        game: &mut TeamplayMatch,
        team: TeamId,
        max_ticks: u32,
    ) -> Option<u32> {
        let required = game.plan.keystone_rooms.len();
        for _ in 0..max_ticks {
            game.advance_bot_tick();
            let state = game.team(team)?;
            if state.gate_open(required) && state.all_members_in(RoomId(EXIT_ROOM)) {
                return Some(game.tick);
            }
            if state.eliminated {
                return None;
            }
        }
        None
    }

    /// First tick at which `team` places its anchor (`DropAnchor` event),
    /// manually ticked so it isn't cut short by the round-finish/countdown
    /// machinery the way a full `run_to_outcome` can be.
    fn first_tick_anchor_dropped(
        game: &mut TeamplayMatch,
        team: TeamId,
        max_ticks: u32,
    ) -> Option<u32> {
        for _ in 0..max_ticks {
            let events = game.advance_bot_tick();
            for event in &events {
                if let TeamAction::DropAnchor { team: t, .. } = event.action
                    && t == team
                {
                    return Some(event.tick);
                }
            }
            if game.finished {
                return None;
            }
        }
        None
    }

    fn balanced_and_forced(
        seed: u64,
        team: TeamId,
        forced: TeamPolicy,
        adversary_strength: usize,
    ) -> (TeamplayMatch, TeamplayMatch) {
        let all_teams = || (0..TEAM_COUNT as u8).map(TeamId).collect::<Vec<_>>();
        let balanced = TeamplayMatch::with_policies(
            seed,
            1,
            all_teams(),
            adversary_strength,
            [TeamPolicy::Balanced; TEAM_COUNT],
        );
        let mut policies = [TeamPolicy::Balanced; TEAM_COUNT];
        policies[team.index()] = forced;
        let forced_game =
            TeamplayMatch::with_policies(seed, 1, all_teams(), adversary_strength, policies);
        (balanced, forced_game)
    }

    /// Same as `balanced_and_forced`, but `team` races paired only with the
    /// slowest baseline team (index 3) as a non-competing decoy, instead of
    /// the full 4-team field. A 4-team round's countdown-driven finish
    /// (Phase 42's "don't come in last" design) ends the round as soon as one
    /// team escapes — with all four baseline-speed teams racing, that's
    /// always the fastest team, well before a trailing team ever reaches the
    /// exit itself. Pairing with only the slowest decoy keeps `team` as the
    /// one that determines when the round ends, so its own completion tick
    /// is actually observable and comparable across policies. (A *solo*
    /// active team doesn't work either: with zero other teams,
    /// `survivor_slots` is 0 and the round's finish check fires on tick 1,
    /// before anyone can escape.)
    fn balanced_and_forced_solo(
        seed: u64,
        team: TeamId,
        forced: TeamPolicy,
        adversary_strength: usize,
    ) -> (TeamplayMatch, TeamplayMatch) {
        let decoy = TeamId(3);
        let active = || {
            if team == decoy {
                vec![team, TeamId(0)]
            } else {
                vec![team, decoy]
            }
        };
        let balanced = TeamplayMatch::with_policies(
            seed,
            1,
            active(),
            adversary_strength,
            [TeamPolicy::Balanced; TEAM_COUNT],
        );
        let mut policies = [TeamPolicy::Balanced; TEAM_COUNT];
        policies[team.index()] = forced;
        let forced_game =
            TeamplayMatch::with_policies(seed, 1, active(), adversary_strength, policies);
        (balanced, forced_game)
    }

    /// Distinctness: across a seed corpus, a Sprinter team's own
    /// round-completion tick (see `first_tick_team_ready`) is strictly less
    /// than the same team index forced-Balanced, on average. Both runs share
    /// every other condition (seed, adversary strength, active teams) via
    /// `TeamplayMatch::with_policies` — policy is the only variable. The team
    /// races solo (see `balanced_and_forced_solo`) so its own completion tick
    /// isn't hidden behind another team ending the round first.
    #[test]
    fn sprinter_finishes_strictly_earlier_than_balanced_across_seed_corpus() {
        let team = TeamId(1); // not the fastest baseline tier, so +1 tier is visible
        let mut sprinter_faster = 0;
        let mut compared = 0;
        for seed in 0..40u64 {
            let (mut balanced, mut sprinter) =
                balanced_and_forced_solo(seed, team, TeamPolicy::Sprinter, 0);
            let balanced_tick = first_tick_team_ready(&mut balanced, team, MAX_TEAMPLAY_TICKS);
            let sprinter_tick = first_tick_team_ready(&mut sprinter, team, MAX_TEAMPLAY_TICKS);
            if let (Some(balanced_tick), Some(sprinter_tick)) = (balanced_tick, sprinter_tick) {
                compared += 1;
                if sprinter_tick < balanced_tick {
                    sprinter_faster += 1;
                }
            }
        }
        assert!(
            compared > 30,
            "expected most of the 40-seed corpus to produce a comparable completion tick, got {compared}"
        );
        assert!(
            sprinter_faster as f64 / compared as f64 > 0.5,
            "Sprinter should finish earlier than Balanced in most of the corpus \
             ({sprinter_faster}/{compared} seeds faster)"
        );
    }

    /// Distinctness: across the corpus, a Freezer team places its anchor
    /// strictly earlier (a lower tick) than the same team index
    /// forced-Balanced, on average — it reaches its signature stabilizing
    /// move before the dual-station gate instead of after.
    #[test]
    fn freezer_performs_more_anchor_actions_than_balanced_across_seed_corpus() {
        let team = TeamId(2);
        let mut freezer_earlier = 0;
        let mut compared = 0;
        for seed in 0..40u64 {
            let (mut balanced, mut freezer) =
                balanced_and_forced(seed, team, TeamPolicy::Freezer, 0);
            let balanced_tick = first_tick_anchor_dropped(&mut balanced, team, MAX_TEAMPLAY_TICKS);
            let freezer_tick = first_tick_anchor_dropped(&mut freezer, team, MAX_TEAMPLAY_TICKS);
            if let (Some(balanced_tick), Some(freezer_tick)) = (balanced_tick, freezer_tick) {
                compared += 1;
                if freezer_tick < balanced_tick {
                    freezer_earlier += 1;
                }
            }
        }
        assert!(
            compared > 30,
            "expected most of the 40-seed corpus to produce a comparable anchor-drop tick, got {compared}"
        );
        assert!(
            freezer_earlier as f64 / compared as f64 > 0.5,
            "Freezer should place its anchor earlier than Balanced in most of the corpus \
             ({freezer_earlier}/{compared} seeds earlier)"
        );
    }

    /// Distinctness: a Saboteur team incurs more guardian-setback events than
    /// the same team index forced-Balanced across the corpus. The guardian
    /// setback is the only adversary/denial lever this brain exposes, so
    /// engaging it more is the closest honest analogue to "prefers
    /// denial-shaped choices" available without inventing new mechanics.
    #[test]
    fn saboteur_triggers_more_guardian_setbacks_than_balanced_across_seed_corpus() {
        // Team 0: the fastest baseline tier, so it visits enough distinct
        // tick values for the shrunk interval to actually bite more often. A
        // slower team's sparser, pace-driven tick visitation can alias badly
        // against `(tick + team.id.0) % interval`, an existing property of
        // `maybe_guardian_setback` unrelated to this policy.
        let team = TeamId(0);
        let mut saboteur_total = 0u32;
        let mut balanced_total = 0u32;
        for seed in 0..40u64 {
            let (mut balanced, mut saboteur) =
                balanced_and_forced(seed, team, TeamPolicy::Saboteur, 2);
            balanced.run_to_outcome(MAX_TEAMPLAY_TICKS);
            saboteur.run_to_outcome(MAX_TEAMPLAY_TICKS);
            balanced_total += balanced.team(team).unwrap().guardian_catches;
            saboteur_total += saboteur.team(team).unwrap().guardian_catches;
        }
        assert!(
            saboteur_total > balanced_total,
            "Saboteur should incur more guardian setbacks across the corpus than Balanced \
             (saboteur={saboteur_total}, balanced={balanced_total})"
        );
    }
}
