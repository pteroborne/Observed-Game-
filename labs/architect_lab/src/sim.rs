//! Pure fixed-tick rules for the Rogue Architect proof.
//!
//! The lab mutates a solved [`HexWfcWorld`] directly because card-driven WFC
//! replacement is the mechanic under test. Presentation never decides legality,
//! and human and bot Architects both call [`ArchitectLab::submit`].

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_facility::hex_wfc::{HexSpace, HexWfcError, HexWfcWorld};
use observed_hex::{HexCoord, HexFace, ports_compatible, travel_distance};

mod stability;
pub use stability::{LabEvent, LabEventKind, RETRACTION_TICKS};
mod cards;
pub use cards::{Card, CardId, CardKind, Deck, District, TileShape};
mod behavior;
pub use behavior::{GuardianIntent, ObserverIntent};
mod command;
pub use command::{ArchitectCommand, CommandRefusal, DoorState, ThresholdKey};
mod mode;
pub use mode::ArchitectMode;
pub mod topology;
mod util;
use util::{
    Prng, command_key, face_between, face_toward, key_face_from, lateral_face, threshold_touches,
};

pub use crate::economy::{EconomyState, GuardianKind};

pub const FIXED_HZ: u32 = 60;
pub const ACTOR_BEAT_TICKS: u32 = FIXED_HZ;
pub const ARCHITECT_COOLDOWN_TICKS: u32 = 300;
pub const HAND_SIZE: usize = 5;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TeamId(pub u8);

impl TeamId {
    pub const LOYAL: Self = Self(0);
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObserverId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuardianId(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObserverState {
    Active,
    Jailed,
    Corrupted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observer {
    pub id: ObserverId,
    pub team: TeamId,
    pub cell: HexCoord,
    pub facing: HexFace,
    pub state: ObserverState,
    pub(crate) hold_beats: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guardian {
    pub id: GuardianId,
    pub cell: HexCoord,
    pub last_detection: Option<HexCoord>,
    pub kind: GuardianKind,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BehaviorTrace {
    pub visited: Vec<&'static str>,
    pub selected: Option<&'static str>,
}

impl BehaviorTrace {
    fn test(&mut self, name: &'static str, succeeds: bool) -> bool {
        self.visited.push(name);
        if succeeds {
            self.selected = Some(name);
        }
        succeeds
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchOutcome {
    Running,
    RogueVictory,
    LoyalVictory,
}

/// Structure and actor knowledge scoped to a specific team.
///
/// §10: "Loyal knowledge never leaks undiscovered structure or actors."
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TeamKnowledge {
    pub team: TeamId,
    pub discovered_cells: BTreeSet<HexCoord>,
    pub known_observers: BTreeMap<ObserverId, HexCoord>,
    pub visible_guardians: BTreeSet<GuardianId>,
}

/// Structure and actor knowledge scoped to the Rogue AI faction.
///
/// §10: "Rogue knowledge exposes facility truth but not undetected loyal positions."
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RogueKnowledge {
    pub cells: BTreeSet<HexCoord>,
    pub guardians: BTreeSet<GuardianId>,
    pub known_observers: BTreeMap<ObserverId, HexCoord>,
}

#[derive(Clone, Debug)]
pub struct ArchitectLab {
    pub mode: ArchitectMode,
    pub tick: u64,
    pub world: HexWfcWorld,
    pub deck: Deck,
    pub cooldown: u32,
    pub bot_architect: bool,
    pub known: BTreeSet<HexCoord>,
    pub observed: BTreeSet<HexCoord>,
    pub anchored: BTreeSet<HexCoord>,
    pub prison_core: BTreeSet<HexCoord>,
    pub prison: crate::prison::PrisonState,
    pub doors: BTreeMap<ThresholdKey, DoorState>,
    pub contradictions: BTreeSet<HexCoord>,
    pub retracted: BTreeSet<HexCoord>,
    pub collapsed_floors: BTreeSet<u8>,
    pub next_retraction_tick: Option<u64>,
    /// A cell selected for retraction while somebody was standing on it, and the
    /// tick it commits anyway. Occupancy used to make a tile immune; it now buys
    /// a warning instead, so the floor can go out from under an Observer who
    /// stayed too long but never under one who had no chance to move.
    pub condemned: Option<(HexCoord, u64)>,
    pub instability_origin: Option<HexCoord>,
    pub events: VecDeque<LabEvent>,
    pub loyal_team_size: usize,
    pub team_knowledge: BTreeMap<TeamId, TeamKnowledge>,
    pub observers: BTreeMap<ObserverId, Observer>,
    pub guardians: BTreeMap<GuardianId, Guardian>,
    pub guardian_visits: BTreeMap<(GuardianId, HexCoord), u32>,
    pub rogue_directive: Option<HexCoord>,
    pub outcome: MatchOutcome,
    pub traces: BTreeMap<String, BehaviorTrace>,
    pub command_log: Vec<(u64, ArchitectCommand)>,
    pub economy: EconomyState,
    pub requisition: crate::requisition::RequisitionState,
}

impl ArchitectLab {
    pub const DEFAULT_LOYAL_TEAM_SIZE: usize = 2;

    pub fn new(seed: u64) -> Result<Self, HexWfcError> {
        Self::generate_with_team_size(
            ArchitectMode::FullAscent,
            seed,
            Self::DEFAULT_LOYAL_TEAM_SIZE,
        )
    }

    pub fn for_mode(mode: ArchitectMode) -> Result<Self, HexWfcError> {
        Self::generate_with_team_size(mode, mode.seed(), Self::DEFAULT_LOYAL_TEAM_SIZE)
    }

    pub fn for_mode_with_team_size(
        mode: ArchitectMode,
        loyal_team_size: usize,
    ) -> Result<Self, HexWfcError> {
        Self::generate_with_team_size(mode, mode.seed(), loyal_team_size)
    }

    pub fn generate(mode: ArchitectMode, seed: u64) -> Result<Self, HexWfcError> {
        Self::generate_with_team_size(mode, seed, Self::DEFAULT_LOYAL_TEAM_SIZE)
    }

    pub fn generate_with_team_size(
        mode: ArchitectMode,
        seed: u64,
        loyal_team_size: usize,
    ) -> Result<Self, HexWfcError> {
        let config = mode.config();
        let mut world = HexWfcWorld::generate(seed, config)?;
        for (&cell, register) in &mut world.architecture {
            *register = floor_register(cell.level);
        }

        let route = world
            .route_between(config.spawn(), config.exit())
            .expect("the real solver guarantees its spawn-to-exit route");
        let guardian_cell = route[0];
        let loyal_team_size = loyal_team_size.clamp(1, 3);
        let route_len = route.len();
        let idx_a = route_len.saturating_mul(2) / 3;
        let observer_a = route[idx_a];
        let observer_b = *route.last().expect("route contains its exit");
        let idx_c = route_len / 3;
        let observer_c = route[idx_c];

        let prison = crate::prison::PrisonState::new(config, &world);
        let prison_core = prison.cells.clone();

        let mut known: BTreeSet<HexCoord> = world.placements.keys().copied().collect();
        let mut observers = BTreeMap::new();
        if loyal_team_size >= 1 {
            let facing = if loyal_team_size > 1 {
                face_toward(&world, observer_a, observer_b).unwrap_or(HexFace::East)
            } else {
                HexFace::East
            };
            observers.insert(
                ObserverId(0),
                Observer {
                    id: ObserverId(0),
                    team: TeamId::LOYAL,
                    cell: observer_a,
                    facing,
                    state: ObserverState::Active,
                    hold_beats: 0,
                },
            );
        }
        if loyal_team_size >= 2 {
            observers.insert(
                ObserverId(1),
                Observer {
                    id: ObserverId(1),
                    team: TeamId::LOYAL,
                    cell: observer_b,
                    facing: HexFace::West,
                    state: ObserverState::Active,
                    hold_beats: 0,
                },
            );
        }
        if loyal_team_size >= 3 {
            observers.insert(
                ObserverId(2),
                Observer {
                    id: ObserverId(2),
                    team: TeamId::LOYAL,
                    cell: observer_c,
                    facing: face_toward(&world, observer_c, observer_a).unwrap_or(HexFace::East),
                    state: ObserverState::Active,
                    hold_beats: 0,
                },
            );
        }
        let guardians = BTreeMap::from([(
            GuardianId(0),
            Guardian {
                id: GuardianId(0),
                cell: guardian_cell,
                last_detection: None,
                kind: GuardianKind::Major,
            },
        )]);

        let economy = EconomyState::new(&world, &observers, seed);
        prison.ensure_placements(&mut world);
        known.extend(&prison_core);

        let mut lab = Self {
            mode,
            tick: 0,
            world,
            deck: Deck::for_levels(seed, config.levels),
            cooldown: 0,
            bot_architect: false,
            known,
            observed: BTreeSet::new(),
            anchored: BTreeSet::new(),
            prison_core,
            prison,
            doors: BTreeMap::new(),
            contradictions: BTreeSet::new(),
            retracted: BTreeSet::new(),
            collapsed_floors: BTreeSet::new(),
            next_retraction_tick: None,
            condemned: None,
            instability_origin: None,
            events: VecDeque::new(),
            loyal_team_size,
            team_knowledge: BTreeMap::new(),
            observers,
            guardians,
            guardian_visits: BTreeMap::new(),
            rogue_directive: None,
            outcome: MatchOutcome::Running,
            traces: BTreeMap::new(),
            command_log: Vec::new(),
            economy,
            requisition: crate::requisition::RequisitionState::new(seed),
        };
        lab.refresh_observation();
        // Damage a solved facility at separated lateral handoffs. These are
        // explicit scenario conditions; repair still uses the ordinary deck.
        let wanted = match mode {
            ArchitectMode::Pocket => 1,
            ArchitectMode::QuickClimb => 2,
            ArchitectMode::FullAscent => 3,
            // A taller stack is damaged proportionally, so retraction pressure
            // scales with the vertical space it has to act in.
            ArchitectMode::DeepStack => 5,
        };
        let mut gaps = Vec::new();
        for triple in route.windows(3) {
            let cell = triple[1];
            if lab.retraction_protected(cell)
                || triple[0].level != cell.level
                || triple[2].level != cell.level
                || gaps
                    .iter()
                    .any(|&(other, _)| travel_distance(other, cell) < 3)
            {
                continue;
            }
            let a = face_between(config, cell, triple[0]).expect("route neighbors");
            let b = face_between(config, cell, triple[2]).expect("route neighbors");
            let required = (1 << a.index()) | (1 << b.index());
            if let Some(shape) = TileShape::ALL
                .into_iter()
                .find(|shape| (0..6).any(|rotation| shape.doors(rotation) == required))
            {
                gaps.push((cell, shape));
                if gaps.len() == wanted {
                    break;
                }
            }
        }
        for (index, &(cell, shape)) in gaps.iter().enumerate() {
            let tile = lab.world.placements.get_mut(&cell).expect("route tile");
            tile.space = HexSpace::Void;
            tile.doors = 0;
            tile.up = observed_hex::PortClass::Sealed;
            tile.down = observed_hex::PortClass::Sealed;
            if index == 0 {
                lab.deck.offer_tile(shape, District::for_level(cell.level));
            }
            lab.record_event(
                LabEventKind::Warning,
                Some(cell),
                "A missing tile interrupts the hunt. Reconnect the route with a card.",
            );
        }
        lab.refresh_observation();
        Ok(lab)
    }

    #[must_use]
    pub fn selected_command(
        &self,
        hand_index: usize,
        target: HexCoord,
        rotation: u8,
    ) -> Option<ArchitectCommand> {
        self.deck
            .hand
            .get(hand_index)
            .map(|card| ArchitectCommand::Play {
                card: card.id,
                target,
                rotation: rotation % 6,
            })
    }

    #[must_use]
    pub fn refusal(&self, command: ArchitectCommand) -> Option<CommandRefusal> {
        if self.outcome != MatchOutcome::Running {
            return Some(CommandRefusal::MatchFinished);
        }
        let (card, target, rotation) = match command {
            ArchitectCommand::Play {
                card,
                target,
                rotation,
            } => {
                if self.cooldown > 0 {
                    return Some(CommandRefusal::Cooldown);
                }
                (card, target, rotation)
            }
            ArchitectCommand::Requisition => return None,
        };
        let Some(card) = self.deck.hand.iter().find(|held| held.id == card).copied() else {
            return Some(CommandRefusal::CardNotInHand);
        };
        if !self.known.contains(&target) {
            return Some(CommandRefusal::UnknownTarget);
        }
        let Some(placement) = self.world.placements.get(&target) else {
            return Some(CommandRefusal::UnknownTarget);
        };
        if self.collapsed_floors.contains(&target.level) {
            return Some(CommandRefusal::CollapsedFloor);
        }
        if self.observed.contains(&target) {
            return Some(CommandRefusal::Observed);
        }
        if self.occupied().contains(&target) {
            return Some(CommandRefusal::Occupied);
        }
        if self.anchored.contains(&target) {
            return Some(CommandRefusal::Anchored);
        }
        if self.doors.iter().any(|(&key, &state)| {
            state == DoorState::Open && threshold_touches(key, target, &self.world)
        }) {
            return Some(CommandRefusal::Anchored);
        }
        if self.prison_core.contains(&target) {
            return Some(CommandRefusal::PrisonCore);
        }

        match card.kind {
            CardKind::Tile(shape) => {
                if card.district != Some(District::for_level(target.level)) {
                    return Some(CommandRefusal::WrongDistrict);
                }
                let doors = shape.doors(rotation);
                if placement.space != HexSpace::Void && placement.doors == doors {
                    return Some(CommandRefusal::NoChange);
                }
                let fits = HexFace::LATERAL.into_iter().any(|face| {
                    doors & (1 << face.index()) != 0
                        && self
                            .world
                            .config
                            .grid()
                            .neighbor(target, face)
                            .and_then(|next| self.world.placements.get(&next))
                            .is_some_and(|next| {
                                next.space != HexSpace::Void && next.is_open(face.opposite())
                            })
                });
                (!fits).then_some(CommandRefusal::NoLocalAttachment)
            }
            CardKind::Door => {
                let Some(key) = self.threshold_key(target, lateral_face(rotation)) else {
                    return Some(CommandRefusal::InvalidThreshold);
                };
                if self.doors.contains_key(&key) {
                    return Some(CommandRefusal::DoorAlreadyPresent);
                }
                let Some(next) = self
                    .world
                    .config
                    .grid()
                    .neighbor(target, key_face_from(key, target))
                else {
                    return Some(CommandRefusal::InvalidThreshold);
                };
                let open =
                    self.world
                        .placements
                        .get(&target)
                        .is_some_and(|tile| tile.is_open(key_face_from(key, target)))
                        && self.world.placements.get(&next).is_some_and(|tile| {
                            tile.is_open(key_face_from(key, target).opposite())
                        });
                if !open {
                    return Some(CommandRefusal::InvalidThreshold);
                }
                if self.observed.contains(&next) {
                    return Some(CommandRefusal::Observed);
                }
                if self.occupied().contains(&next) {
                    return Some(CommandRefusal::Occupied);
                }
                if self.anchored.contains(&next) || self.prison_core.contains(&next) {
                    return Some(CommandRefusal::Anchored);
                }
                None
            }
        }
    }

    pub fn submit(&mut self, command: ArchitectCommand) -> Result<(), CommandRefusal> {
        if let Some(refusal) = self.refusal(command) {
            return Err(refusal);
        }
        match command {
            ArchitectCommand::Play {
                card,
                target,
                rotation,
            } => {
                let held = self
                    .deck
                    .hand
                    .iter()
                    .find(|held| held.id == card)
                    .copied()
                    .expect("legality proved the card is held");
                match held.kind {
                    CardKind::Tile(shape) => {
                        let placement = self
                            .world
                            .placements
                            .get_mut(&target)
                            .expect("legality proved the target exists");
                        placement.space = HexSpace::Hall;
                        placement.archetype = shape.archetype();
                        placement.doors = shape.doors(rotation);
                        placement.up = observed_hex::PortClass::Sealed;
                        placement.down = observed_hex::PortClass::Sealed;
                        self.retracted.remove(&target);
                        *self.world.cell_revisions.entry(target).or_default() += 1;
                        self.doors
                            .retain(|key, _| !threshold_touches(*key, target, &self.world));
                    }
                    CardKind::Door => {
                        let key = self
                            .threshold_key(target, lateral_face(rotation))
                            .expect("legality proved the threshold exists");
                        self.doors.insert(key, DoorState::Closed);
                    }
                }
                assert!(self.deck.spend(card), "legality proved the card is held");
                self.cooldown = ARCHITECT_COOLDOWN_TICKS;
                self.rogue_directive = Some(target);
                self.command_log.push((self.tick, command));
                self.instability_origin.get_or_insert(target);
                self.refresh_contradictions();
                self.sync_retraction_clock();
                // A card play feeds the meter, and a contradiction feeds it hardest:
                // instability summons the horde, clean construction does not.
                let is_contradiction = self.contradictions.contains(&target);
                let is_door = matches!(held.kind, CardKind::Door);
                if self.is_contesting_generator(target)
                    && (self.economy.is_at_generator(target) || is_contradiction || is_door)
                    && self.cut_floor_power(target.level)
                {
                    self.record_event(
                        LabEventKind::Warning,
                        Some(target),
                        "Generator power cut by contested generator play.",
                    );
                }
                self.economy
                    .on_card_played(target.level, is_contradiction, self.bot_architect);
                self.resolve_disturbance_waves(target.level);
                self.record_event(
                    LabEventKind::Played,
                    Some(target),
                    "Card played. Guardian investigates this tile.",
                );
            }
            ArchitectCommand::Requisition => {
                crate::requisition::apply_requisition(self);
            }
        }
        Ok(())
    }

    pub fn tick(&mut self) {
        if self.outcome != MatchOutcome::Running {
            return;
        }
        self.tick += 1;
        self.cooldown = self.cooldown.saturating_sub(1);
        self.advance_retraction();
        self.resolve_falls();
        if self.outcome != MatchOutcome::Running {
            return;
        }
        if !self.tick.is_multiple_of(u64::from(ACTOR_BEAT_TICKS)) {
            return;
        }

        if self.bot_architect {
            let (command, trace) = self.architect_intent();
            self.traces.insert("Architect".to_string(), trace);
            if let Some(command) = command {
                let _ = self.submit(command);
            }
        }

        let observer_ids: Vec<_> = self.observers.keys().copied().collect();
        for id in observer_ids {
            let (intent, trace) = self.observer_intent(id);
            self.traces.insert(format!("Observer {}", id.0), trace);
            self.apply_observer_intent(id, intent);
        }
        self.economy.tick_beat(&self.world, &self.observers);
        self.refresh_observation();

        let guardian_ids: Vec<_> = self.guardians.keys().copied().collect();
        for id in guardian_ids {
            let (intent, trace) = self.guardian_intent(id);
            self.traces.insert(format!("Guardian {}", id.0), trace);
            self.apply_guardian_intent(id, intent);
        }
        self.refresh_observation();
        // 1. Same-tick capture precedence: Guardian capture resolves before summit completion.
        // If all loyal observers have been eliminated (jailed or corrupted), Rogue wins.
        if !self.observers.is_empty()
            && self.observers.values().all(|observer| {
                observer.state == ObserverState::Jailed
                    || observer.state == ObserverState::Corrupted
            })
        {
            self.outcome = MatchOutcome::RogueVictory;
            return;
        }

        // 2. Corruption-adjusted summit quorum:
        // A team achieves LoyalVictory if all of its remaining loyal (uncorrupted) Observers
        // are active and present at the facility summit exit.
        // - Corrupted Observers leave the quorum (lowering the required count).
        // - Jailed Observers remain loyal, so they count toward the total required;
        //   since they are in the prison core, victory cannot be achieved while any teammate is jailed.
        // - Zero loyal Observers cannot achieve LoyalVictory.
        let teams: BTreeSet<TeamId> = self.observers.values().map(|o| o.team).collect();
        for team in teams {
            let team_observers: Vec<&Observer> =
                self.observers.values().filter(|o| o.team == team).collect();
            let loyal_observers: Vec<&Observer> = team_observers
                .into_iter()
                .filter(|o| o.state != ObserverState::Corrupted)
                .collect();
            let loyal_count = loyal_observers.len();
            if loyal_count > 0 {
                let summit_count = loyal_observers
                    .iter()
                    .filter(|o| {
                        o.state == ObserverState::Active && o.cell == self.world.config.exit()
                    })
                    .count();
                if summit_count == loyal_count {
                    self.outcome = MatchOutcome::LoyalVictory;
                    return;
                }
            }
        }
    }

    pub fn resolve_falls(&mut self) -> Vec<crate::falls::FallEvent> {
        crate::falls::resolve_falls(self)
    }

    pub fn step_beat(&mut self) {
        let target = self.tick + u64::from(ACTOR_BEAT_TICKS);
        while self.tick < target && self.outcome == MatchOutcome::Running {
            self.tick();
        }
    }

    #[must_use]
    pub fn mutable_targets(&self) -> Vec<HexCoord> {
        self.known
            .iter()
            .copied()
            .filter(|cell| {
                self.world.placements.get(cell).is_some_and(|placement| {
                    placement.space != HexSpace::Void
                        || HexFace::LATERAL.into_iter().any(|face| {
                            self.world
                                .config
                                .grid()
                                .neighbor(*cell, face)
                                .and_then(|next| self.world.placements.get(&next))
                                .is_some_and(|tile| {
                                    tile.space != HexSpace::Void && tile.is_open(face.opposite())
                                })
                        })
                }) && !self.collapsed_floors.contains(&cell.level)
                    && !self.prison_core.contains(cell)
            })
            .collect()
    }

    #[must_use]
    pub fn route(&self, from: HexCoord, to: HexCoord) -> Option<Vec<HexCoord>> {
        if from == to {
            return Some(vec![from]);
        }
        let mut parent = BTreeMap::new();
        let mut seen = BTreeSet::from([from]);
        let mut queue = VecDeque::from([from]);
        while let Some(cell) = queue.pop_front() {
            for next in self.exits(cell) {
                if seen.insert(next) {
                    parent.insert(next, cell);
                    if next == to {
                        let mut path = vec![to];
                        let mut cursor = to;
                        while let Some(previous) = parent.get(&cursor).copied() {
                            path.push(previous);
                            cursor = previous;
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

    #[must_use]
    pub fn exits_minor(&self, from: HexCoord) -> Vec<HexCoord> {
        self.exits(from)
            .into_iter()
            .filter(|cell| !self.prison_core.contains(cell))
            .collect()
    }

    #[must_use]
    pub fn route_minor(&self, from: HexCoord, to: HexCoord) -> Option<Vec<HexCoord>> {
        if self.prison_core.contains(&to) {
            return None;
        }
        if from == to {
            return Some(vec![from]);
        }
        let mut parent = BTreeMap::new();
        let mut seen = BTreeSet::from([from]);
        let mut queue = VecDeque::from([from]);
        while let Some(cell) = queue.pop_front() {
            for next in self.exits_minor(cell) {
                if seen.insert(next) {
                    parent.insert(next, cell);
                    if next == to {
                        let mut path = vec![to];
                        let mut cursor = to;
                        while let Some(previous) = parent.get(&cursor).copied() {
                            path.push(previous);
                            cursor = previous;
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

    #[must_use]
    pub fn exits(&self, from: HexCoord) -> Vec<HexCoord> {
        let Some(placement) = self.world.placements.get(&from) else {
            return Vec::new();
        };
        if placement.space == HexSpace::Void {
            return Vec::new();
        }
        HexFace::ALL
            .into_iter()
            .filter_map(|face| {
                let next = self.world.config.grid().neighbor(from, face)?;
                let other = self.world.placements.get(&next)?;
                if other.space == HexSpace::Void {
                    return None;
                }
                if face.is_lateral() {
                    if !placement.is_open(face) || !other.is_open(face.opposite()) {
                        return None;
                    }
                    if self
                        .threshold_key(from, face)
                        .and_then(|key| self.doors.get(&key))
                        == Some(&DoorState::Closed)
                    {
                        return None;
                    }
                } else {
                    if !self.economy.is_powered(from.level) || !self.economy.is_powered(next.level)
                    {
                        return None;
                    }
                    if placement.ports().port(face) == observed_hex::PortClass::Sealed
                        || !ports_compatible(
                            placement.ports().port(face),
                            other.ports().port(face.opposite()),
                        )
                    {
                        return None;
                    }
                }
                Some(next)
            })
            .collect()
    }

    #[must_use]
    pub fn guardian_reachable(&self) -> BTreeSet<HexCoord> {
        let mut seen = BTreeSet::new();
        let mut queue: VecDeque<_> = self
            .guardians
            .values()
            .map(|guardian| guardian.cell)
            .collect();
        while let Some(cell) = queue.pop_front() {
            if seen.insert(cell) {
                queue.extend(self.exits(cell));
            }
        }
        seen
    }

    #[must_use]
    pub fn occupied(&self) -> BTreeSet<HexCoord> {
        self.observers
            .values()
            .filter(|observer| observer.state == ObserverState::Active)
            .map(|observer| observer.cell)
            .chain(self.guardians.values().map(|guardian| guardian.cell))
            .collect()
    }

    pub(crate) fn threshold_key(&self, cell: HexCoord, face: HexFace) -> Option<ThresholdKey> {
        if !face.is_lateral() {
            return None;
        }
        let neighbor = self.world.config.grid().neighbor(cell, face)?;
        Some(if cell <= neighbor {
            ThresholdKey { cell, face }
        } else {
            ThresholdKey {
                cell: neighbor,
                face: face.opposite(),
            }
        })
    }

    /// Guardians detect along connected straight thresholds. Closed doors stop sight.
    #[must_use]
    pub fn detected_observers(&self) -> BTreeSet<ObserverId> {
        let mut visible = BTreeSet::new();
        for guardian in self.guardians.values() {
            visible.insert(guardian.cell);
            for face in HexFace::LATERAL {
                let mut cell = guardian.cell;
                for _ in 0..6 {
                    let Some(next) = self.step_through(cell, face) else {
                        break;
                    };
                    visible.insert(next);
                    cell = next;
                }
            }
        }
        self.observers
            .values()
            .filter(|observer| {
                observer.state == ObserverState::Active && visible.contains(&observer.cell)
            })
            .map(|observer| observer.id)
            .collect()
    }

    pub fn refresh_observation(&mut self) {
        self.observed.clear();
        for observer in self
            .observers
            .values()
            .filter(|observer| observer.state == ObserverState::Active)
        {
            self.observed.insert(observer.cell);
            if self.economy.is_powered(observer.cell.level)
                && let Some(next) = self.step_through(observer.cell, observer.facing)
            {
                self.observed.insert(next);
            }
        }
        self.update_team_knowledge();
    }

    /// Updates team-scoped knowledge for all active teams.
    ///
    /// §10: "Loyal knowledge never leaks undiscovered structure or actors."
    pub fn update_team_knowledge(&mut self) {
        let teams: BTreeSet<TeamId> = self.observers.values().map(|o| o.team).collect();
        for team in teams {
            // 1. Structure: discover cells currently observed by active observers on this team
            let mut newly_observed = Vec::new();
            for observer in self
                .observers
                .values()
                .filter(|o| o.team == team && o.state == ObserverState::Active)
            {
                newly_observed.push(observer.cell);
                if self.economy.is_powered(observer.cell.level)
                    && let Some(next) = self.step_through(observer.cell, observer.facing)
                {
                    newly_observed.push(next);
                }
            }

            let team_observed: BTreeSet<HexCoord> = newly_observed.iter().copied().collect();

            // 2. Actors: team members know each other's positions; rivals only if observed
            let mut known_obs = BTreeMap::new();
            let prior_discovered = self
                .team_knowledge
                .get(&team)
                .map(|k| k.discovered_cells.clone())
                .unwrap_or_default();

            for observer in self.observers.values() {
                let is_known = observer.team == team
                    || ((prior_discovered.contains(&observer.cell)
                        || team_observed.contains(&observer.cell))
                        && self.observed.contains(&observer.cell));
                if is_known {
                    known_obs.insert(observer.id, observer.cell);
                }
            }

            // 3. Actors: guardians are visible only if on a cell currently observed by this team
            let visible_guardians: BTreeSet<GuardianId> = self
                .guardians
                .values()
                .filter(|g| team_observed.contains(&g.cell))
                .map(|g| g.id)
                .collect();

            let tk = self
                .team_knowledge
                .entry(team)
                .or_insert_with(|| TeamKnowledge {
                    team,
                    ..Default::default()
                });
            tk.discovered_cells.extend(newly_observed);
            tk.known_observers = known_obs;
            tk.visible_guardians = visible_guardians;
        }
    }

    /// Query the knowledge available to a specific loyal team.
    #[must_use]
    pub fn team_knowledge(&self, team: TeamId) -> TeamKnowledge {
        self.team_knowledge
            .get(&team)
            .cloned()
            .unwrap_or_else(|| TeamKnowledge {
                team,
                ..Default::default()
            })
    }

    /// Query the knowledge available to Rogue AI.
    ///
    /// §10: "Rogue knowledge exposes facility truth but not undetected loyal positions."
    #[must_use]
    pub fn rogue_knowledge(&self) -> RogueKnowledge {
        let mut cells: BTreeSet<HexCoord> = self.world.placements.keys().copied().collect();
        cells.extend(&self.prison_core);
        let guardians: BTreeSet<GuardianId> = self.guardians.keys().copied().collect();
        let detected = self.detected_observers();
        let known_observers: BTreeMap<ObserverId, HexCoord> = self
            .observers
            .values()
            .filter(|o| {
                o.state == ObserverState::Jailed
                    || o.state == ObserverState::Corrupted
                    || detected.contains(&o.id)
            })
            .map(|o| (o.id, o.cell))
            .collect();
        RogueKnowledge {
            cells,
            guardians,
            known_observers,
        }
    }

    fn step_through(&self, from: HexCoord, face: HexFace) -> Option<HexCoord> {
        self.exits(from)
            .into_iter()
            .find(|&next| self.world.config.grid().neighbor(from, face) == Some(next))
    }

    fn refresh_contradictions(&mut self) {
        self.contradictions.clear();
        for (&cell, placement) in &self.world.placements {
            if placement.space == HexSpace::Void || self.collapsed_floors.contains(&cell.level) {
                continue;
            }
            for face in HexFace::ALL {
                let Some(next) = self.world.config.grid().neighbor(cell, face) else {
                    continue;
                };
                let Some(other) = self.world.placements.get(&next) else {
                    continue;
                };
                if (other.space != HexSpace::Void
                    && !ports_compatible(
                        placement.ports().port(face),
                        other.ports().port(face.opposite()),
                    ))
                    || (self.retracted.contains(&next)
                        && placement.ports().port(face) != observed_hex::PortClass::Sealed)
                {
                    self.contradictions.insert(cell);
                    if other.space != HexSpace::Void {
                        self.contradictions.insert(next);
                    }
                }
            }
        }
    }
}

/// Canonical architecture register assigned to each floor level for visual legibility.
#[must_use]
pub fn floor_register(level: u8) -> observed_content::ArchitectureRegister {
    match level {
        0 => observed_content::ArchitectureRegister::Institutional,
        1 => observed_content::ArchitectureRegister::LiminalGrid,
        2 => observed_content::ArchitectureRegister::Wellshaft,
        3 => observed_content::ArchitectureRegister::FacetMonument,
        _ => observed_content::ArchitectureRegister::Megastructure,
    }
}

/// Distinct floor architectural title for legibility.
#[must_use]
pub fn floor_title(level: u8) -> &'static str {
    match level {
        0 => "FOUNDATION // INSTITUTIONAL",
        1 => "CONCOURSE // LIMINAL GRID",
        2 => "INTERIOR // WELLSHAFT",
        3 => "GALLERY // FACET MONUMENT",
        _ => "SUMMIT // MEGASTRUCTURE",
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod pressure_tests;

#[cfg(test)]
mod playtest_instrument;
