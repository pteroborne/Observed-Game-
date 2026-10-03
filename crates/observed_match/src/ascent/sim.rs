//! Pure fixed-tick rules for the Rogue Architect proof.
//!
//! The lab mutates a solved [`HexWfcWorld`] directly because card-driven WFC
//! replacement is the mechanic under test. Presentation never decides legality,
//! and human and bot Architects both call [`ArchitectLab::submit`].

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use observed_facility::hex_wfc::{HexPlacement, HexSpace, HexWfcError, HexWfcWorld};
use observed_hex::{HexCoord, HexFace, ports_compatible, travel_distance};

mod control;
pub use control::{ObserverAction, ObserverCommand, ObserverRefusal};
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
mod objective;
pub use objective::{DARKNESS_BEATS, RogueObjective, StateHold};
mod directive;
mod sensor;
pub use directive::{DIRECTIVE_TICKS, RogueDirective};
pub use sensor::{MAX_SENSORS, SENSOR_RANGE};
mod embodied;
mod instability;
mod loyal;
mod rogue;
mod util;
pub use embodied::{Embodiment, Place, linked_vertically};
use util::{
    Prng, command_key, face_between, face_toward, key_face_from, lateral_face, threshold_touches,
};

pub use crate::ascent::economy::{EconomyState, GuardianKind};

pub const FIXED_HZ: u32 = 60;
pub const ACTOR_BEAT_TICKS: u32 = FIXED_HZ;
pub const ARCHITECT_COOLDOWN_TICKS: u32 = 300;

/// How far an Observer can see along a facing, in cells.
///
/// Four, because that is where the measurement stops paying: across the four scenario
/// shapes and eight seeds each, raising the limit from four to eight or sixteen adds a
/// handful of cells out of thousands. Real facilities are dense enough that sight rarely
/// travels far, so a short range is nearly as powerful as an unlimited one and much
/// easier to read on a board. See `labs/suspension_lab`.
pub const OBSERVER_SIGHT_RANGE: u32 = 4;

/// How near a cell an embodied Observer sees must be to be warded by the look, metres from
/// the eye: one cell across. On a first-person facility the body's real sight
/// (`hex_wfc::sight`) stands in for the lab's lines; it wards its own cell and what it
/// sees this close, close to the lab's own cell and one step on, and knows everything it
/// sees.
pub const WARD_REACH: f32 = 14.0;
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
    /// Last seen geometry, never a live lookup into hidden world state.
    pub cells: BTreeMap<HexCoord, KnownCell>,
    /// Cells visible to this team at this tick; absent cells are remembered.
    pub visible_cells: BTreeSet<HexCoord>,
    pub team: TeamId,
    pub discovered_cells: BTreeSet<HexCoord>,
    pub known_observers: BTreeMap<ObserverId, HexCoord>,
    pub visible_guardians: BTreeSet<GuardianId>,
}

/// A structural observation stored at the time it was made.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownCell {
    pub placement: observed_facility::hex_wfc::HexPlacement,
    pub seen_at: u64,
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

/// Rules governing whether and how floor power can be restored once cut.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PowerPolicy {
    /// Today's behaviour: floors go out and never come back.
    OneWay,
    /// An Observer at a generator can bring a floor back up.
    #[default]
    Restorable,
    /// Nothing ever cuts power. For isolating other variables.
    AlwaysOn,
}

impl PowerPolicy {
    pub const ALL: [Self; 3] = [Self::Restorable, Self::OneWay, Self::AlwaysOn];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::OneWay => "One-Way",
            Self::Restorable => "Restorable",
            Self::AlwaysOn => "Always-On",
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Restorable => Self::OneWay,
            Self::OneWay => Self::AlwaysOn,
            Self::AlwaysOn => Self::Restorable,
        }
    }

    #[must_use]
    pub fn previous(self) -> Self {
        match self {
            Self::Restorable => Self::AlwaysOn,
            Self::OneWay => Self::Restorable,
            Self::AlwaysOn => Self::OneWay,
        }
    }
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
    /// Cells an Observer **wards**: protected from retraction and rewriting.
    ///
    /// Deliberately still the short set — the cell underfoot and one step onward. Sight
    /// now reaches further than this, and letting protection follow it would multiply the
    /// warded area by roughly three and take the Rogue's ability to touch anything in
    /// view of a window. Seeing and warding are different powers; see `seen`.
    pub observed: BTreeSet<HexCoord>,
    /// Cells an Observer can **see**, including across open air.
    ///
    /// Feeds knowledge and discovery rather than protection. A superset of `observed`.
    pub seen: BTreeSet<HexCoord>,
    pub anchored: BTreeSet<HexCoord>,
    pub prison_core: BTreeSet<HexCoord>,
    pub prison: crate::ascent::prison::PrisonState,
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
    /// Where a Rogue seat has sent the major Guardians (`directive`). Unlike
    /// `rogue_directive`, which every card play also sets for the lab's Guardians, only an
    /// explicit directive sets this, and a first-person host walks its bodies by it.
    pub directed: Option<RogueDirective>,
    /// The Rogue's sensors, and the tick each was installed (`sensor`).
    pub sensors: BTreeMap<HexCoord, u64>,
    /// What the Rogue wins by this match. `Purge` is the behaviour every match has had
    /// until now, and an unselected objective changes nothing.
    pub objective: RogueObjective,
    /// Active Observers lighting the cell they face, as of the last observation refresh.
    /// Maintained where each sightline is already decided; nothing re-derives it.
    pub lit_sightlines: usize,
    /// Active Observers, as of the last observation refresh.
    pub active_observers: usize,
    /// How long the facility has gone unwitnessed. Tracked in every match whether or not
    /// Darkness is the selected objective, because a streak that is never measured cannot
    /// tell us whether the objective is reachable.
    pub darkness: StateHold,
    pub outcome: MatchOutcome,
    /// The team that brought every loyal Observer to the summit, once one has.
    pub summit_team: Option<TeamId>,
    pub traces: BTreeMap<String, BehaviorTrace>,
    pub command_log: Vec<(u64, ArchitectCommand)>,
    pub economy: EconomyState,
    /// Cells with a physical standing point where an Architect may deploy a
    /// station. The first-person host refreshes this after directed rewrites.
    pub(crate) station_sites: BTreeSet<HexCoord>,
    pub requisition: crate::ascent::requisition::RequisitionState,
    pub power_policy: PowerPolicy,
    /// The match is held in planning: the shell is showing the board with time stopped.
    ///
    /// The shells own the pause; the simulation needs to know about it because the
    /// Architect's cooldown only drains inside `tick`, so a held match can never recharge.
    /// See backlog #47.
    pub planning: bool,
    /// Placements still allowed while `planning` without charging the cooldown.
    ///
    /// Setting the trap is the Rogue's opening move, and it should not cost five seconds
    /// of a clock that is not running. One hand's worth, once per match: enough to build
    /// an opening, not enough to author the facility for free.
    pub setup_placements_left: u32,
    /// These rules run over a first-person facility built from authored tiles, not over
    /// a lab board. A tile play must then be one the corpus can build, and a stamped
    /// room or a vertical link cannot be rewritten or retracted one cell at a time.
    /// Set only by `over_facility`: a lab board switched over midway would hold rewrites
    /// no host ever builds.
    pub(crate) authored: bool,
    /// Placements rewritten since the host last took them. Only an authored match keeps
    /// them: its host commits them to the physical facility (`ascent::facility`).
    pub(crate) rewrites: BTreeMap<HexCoord, HexPlacement>,
    /// Observers whose cell and facing come from a first-person body. The body is the
    /// authority: no beat moves them, and a body falls physically rather than by rule.
    pub(crate) embodied: BTreeSet<ObserverId>,
    /// What each embodied Observer's body actually sees, each cell with the nearest
    /// distance it is seen at (`see`). An Observer with an entry wards and knows by it in
    /// place of the lab's lines along its facing.
    pub(crate) sight: BTreeMap<ObserverId, BTreeMap<HexCoord, f32>>,
    /// Guardians the host moves and catches with - a first-person match's own - whose
    /// cells the rules take from it (`embody_guardian`) and never move themselves.
    pub(crate) embodied_guardians: BTreeSet<GuardianId>,
    /// Guardians released on a first-person facility since the host last took them
    /// (`take_releases`): each is given a body there, and comes back to the rules as an
    /// embodied Guardian rather than hunting in the rules alone.
    pub(crate) releases: Vec<Guardian>,
}

/// Everything that differs between one match's rules and another's at tick zero.
pub(crate) struct Parts {
    pub mode: ArchitectMode,
    pub seed: u64,
    pub world: HexWfcWorld,
    pub deck: Deck,
    pub known: BTreeSet<HexCoord>,
    pub prison_core: BTreeSet<HexCoord>,
    pub prison: crate::ascent::prison::PrisonState,
    pub loyal_team_size: usize,
    pub observers: BTreeMap<ObserverId, Observer>,
    pub guardians: BTreeMap<GuardianId, Guardian>,
    pub economy: EconomyState,
}

impl ArchitectLab {
    pub const DEFAULT_LOYAL_TEAM_SIZE: usize = 2;

    /// The district of floor `level`: its architecture, and the cards that build on it.
    #[must_use]
    pub const fn district(&self, level: u8) -> District {
        District::for_floor(level, self.world.config.levels)
    }

    /// The rules at tick zero, before observation has been refreshed.
    pub(crate) fn assemble(parts: Parts) -> Self {
        let Parts {
            mode,
            seed,
            world,
            deck,
            known,
            prison_core,
            prison,
            loyal_team_size,
            observers,
            guardians,
            economy,
        } = parts;
        Self {
            mode,
            tick: 0,
            world,
            deck,
            cooldown: 0,
            bot_architect: false,
            known,
            observed: BTreeSet::new(),
            seen: BTreeSet::new(),
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
            directed: None,
            sensors: BTreeMap::new(),
            objective: RogueObjective::default(),
            lit_sightlines: 0,
            active_observers: 0,
            darkness: StateHold::new(DARKNESS_BEATS),
            outcome: MatchOutcome::Running,
            summit_team: None,
            traces: BTreeMap::new(),
            command_log: Vec::new(),
            economy,
            station_sites: BTreeSet::new(),
            requisition: crate::ascent::requisition::RequisitionState::new(seed),
            power_policy: PowerPolicy::default(),
            planning: false,
            setup_placements_left: HAND_SIZE as u32,
            authored: false,
            rewrites: BTreeMap::new(),
            embodied: BTreeSet::new(),
            sight: BTreeMap::new(),
            embodied_guardians: BTreeSet::new(),
            releases: Vec::new(),
        }
    }

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

    pub fn for_mode_with_policy(
        mode: ArchitectMode,
        power_policy: PowerPolicy,
    ) -> Result<Self, HexWfcError> {
        Self::for_mode(mode).map(|lab| lab.with_power_policy(power_policy))
    }

    #[must_use]
    pub fn with_power_policy(mut self, power_policy: PowerPolicy) -> Self {
        self.power_policy = power_policy;
        self
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
            *register = floor_register(cell.level, config.levels);
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

        let prison = crate::ascent::prison::PrisonState::new(config, &world);
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

        let economy = EconomyState::new(&world, &observers, &prison_core, seed);
        prison.ensure_placements(&mut world);
        known.extend(&prison_core);

        let mut lab = Self::assemble(Parts {
            mode,
            seed,
            world,
            deck: Deck::for_levels(seed, config.levels),
            known,
            prison_core,
            prison,
            loyal_team_size,
            observers,
            guardians,
            economy,
        });
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
            // The card dealt must rebuild the cell as it was, every door of it: one offered
            // for the route's two faces alone would leave a solved branch or room door
            // facing a wall, and the repair would be a contradiction of its own.
            let required = lab.world.placements[&cell].doors;
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
                lab.deck.offer_tile(shape, lab.district(cell.level));
            }
            lab.record_event(
                LabEventKind::Warning,
                Some(cell),
                "A missing tile interrupts the hunt. Reconnect the route with a card.",
            );
        }
        lab.economy = EconomyState::new(&lab.world, &lab.observers, &lab.prison_core, seed);
        // Classify the unbuilt cells before anyone looks: sight crosses air and stops at
        // rock, and until this runs every unbuilt cell is rock. Deterministic and
        // idempotent, so it changes nothing about the replay contract.
        let _ = lab.world.mark_open_air();
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

    /// A free placement is available: the match is held in planning and the opening
    /// allowance has not been spent.
    #[must_use]
    pub const fn has_setup_allowance(&self) -> bool {
        self.planning && self.setup_placements_left > 0
    }

    #[must_use]
    pub fn refusal(&self, command: ArchitectCommand) -> Option<CommandRefusal> {
        self.refusal_in_context(command, &self.deck, self.cooldown, &self.known)
    }

    pub(crate) fn refusal_in_context(
        &self,
        command: ArchitectCommand,
        deck: &Deck,
        cooldown: u32,
        known: &BTreeSet<HexCoord>,
    ) -> Option<CommandRefusal> {
        if self.outcome != MatchOutcome::Running {
            return Some(CommandRefusal::MatchFinished);
        }
        let (card, target, rotation) = match command {
            ArchitectCommand::Play {
                card,
                target,
                rotation,
            } => {
                // A held match never ticks, so the cooldown never drains. The setup
                // allowance is what makes planning a phase rather than a single move.
                if cooldown > 0 && !self.has_setup_allowance() {
                    return Some(CommandRefusal::Cooldown);
                }
                (card, target, rotation)
            }
            ArchitectCommand::Requisition => return None,
        };
        let Some(card) = deck.hand.iter().find(|held| held.id == card).copied() else {
            return Some(CommandRefusal::CardNotInHand);
        };
        // The Rogue's machinery goes where it is sent, by its own rules.
        match card.kind {
            CardKind::Directive => return self.directive_refusal(target),
            CardKind::Sensor => return self.sense_refusal(target),
            _ => {}
        }
        if !known.contains(&target) {
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
                if card.district != Some(self.district(target.level)) {
                    return Some(CommandRefusal::WrongDistrict);
                }
                if self.fixed_structure(target) {
                    return Some(CommandRefusal::FixedStructure);
                }
                let doors = shape.doors(rotation);
                if self.authored
                    && observed_facility::hex_wfc::authored_hall(target, doors).is_none()
                {
                    return Some(CommandRefusal::Unbuildable);
                }
                if placement.space.built() && placement.doors == doors {
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
                            .is_some_and(|next| next.space.built() && next.is_open(face.opposite()))
                });
                (!fits).then_some(CommandRefusal::NoLocalAttachment)
            }
            CardKind::Stair => {
                if card.district != Some(self.district(target.level)) {
                    return Some(CommandRefusal::WrongDistrict);
                }
                // A stair is a climb composition (`docs/climb_compositions_plan.md`): three
                // cells along the heading on this floor and a landing above the last. One
                // card, four cells, and every one of them has to be buildable.
                let heading = lateral_face(rotation);
                let Some(cells) =
                    observed_facility::hex_wfc::authored_climb(self.world.config, target, heading)
                else {
                    return Some(CommandRefusal::Unbuildable);
                };
                let cells = cells.map(|placement| placement.coord);
                // The climb goes where the team has not been, but not through anything a
                // play could not touch at its foot.
                if cells.iter().any(|&cell| self.fixed_structure(cell)) {
                    return Some(CommandRefusal::FixedStructure);
                }
                if cells.iter().any(|cell| {
                    !self.world.placements.contains_key(cell)
                        || self.collapsed_floors.contains(&cell.level)
                }) {
                    return Some(CommandRefusal::CollapsedFloor);
                }
                // Not up into open sky, nor from it: building in the air re-derives the
                // whole open-air region, and the edges of it re-project wherever they are -
                // in front of whoever is watching them.
                if cells.iter().any(|cell| {
                    self.world
                        .placements
                        .get(cell)
                        .is_some_and(|p| p.space == HexSpace::Air)
                }) {
                    return Some(CommandRefusal::Unbuildable);
                }
                // The foot is the play's own target, judged above like any other; the rest
                // of the composition is held to the same rules.
                let rest = &cells[1..];
                if rest.iter().any(|cell| self.observed.contains(cell)) {
                    return Some(CommandRefusal::Observed);
                }
                let occupied = self.occupied();
                if rest.iter().any(|cell| occupied.contains(cell)) {
                    return Some(CommandRefusal::Occupied);
                }
                if rest
                    .iter()
                    .any(|cell| self.anchored.contains(cell) || self.prison_core.contains(cell))
                {
                    return Some(CommandRefusal::Anchored);
                }
                // Entered from the side facing away from the climb, off a walkway.
                let entrance = heading.opposite();
                let fits = self
                    .world
                    .config
                    .grid()
                    .neighbor(target, entrance)
                    .and_then(|next| self.world.placements.get(&next))
                    .is_some_and(|next| next.space.built() && next.is_open(entrance.opposite()));
                (!fits).then_some(CommandRefusal::NoLocalAttachment)
            }
            CardKind::Directive | CardKind::Sensor => {
                unreachable!("the Rogue's orders are judged above")
            }
            CardKind::Surge => (!placement.space.built() || self.retracted.contains(&target))
                .then_some(CommandRefusal::VoidTarget),
            CardKind::Station => {
                if !placement.space.built() || self.retracted.contains(&target) {
                    return Some(CommandRefusal::VoidTarget);
                }
                if self.economy.stations.contains(&target) {
                    return Some(CommandRefusal::NoChange);
                }
                if self.economy.is_at_generator(target) || self.linked_vertically(target) {
                    return Some(CommandRefusal::FixedStructure);
                }
                (self.authored && !self.station_sites.contains(&target))
                    .then_some(CommandRefusal::Unbuildable)
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
                // A climb composition's span is not a doorway: it is the flight running on,
                // metres above any floor, and a door there would be a panel across a stair.
                let face = key_face_from(key, target);
                if self
                    .world
                    .placements
                    .get(&target)
                    .is_some_and(|tile| tile.archetype.span_mask() & (1 << face.index()) != 0)
                {
                    return Some(CommandRefusal::InvalidThreshold);
                }
                // Two cells of one room share no doorway, only open floor: a first-person
                // door there would be a panel standing in the middle of the room.
                if self.authored
                    && self.world.blueprints.iter().any(|blueprint| {
                        blueprint.cells.contains(&target) && blueprint.cells.contains(&next)
                    })
                {
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

    /// The cell a tile play of `shape` builds at `target`. Legality has already refused a
    /// shape an authored facility cannot build.
    #[must_use]
    pub fn played_placement(
        &self,
        shape: TileShape,
        target: HexCoord,
        rotation: u8,
    ) -> HexPlacement {
        let doors = shape.doors(rotation);
        if self.authored {
            observed_facility::hex_wfc::authored_hall(target, doors)
                .expect("legality proved the corpus builds this tile")
        } else {
            HexPlacement {
                coord: target,
                space: HexSpace::Hall,
                archetype: shape.archetype(),
                doors,
                up: observed_hex::PortClass::Sealed,
                down: observed_hex::PortClass::Sealed,
            }
        }
    }

    /// The cells a stair play builds from `target`, turned by `rotation`: a climb
    /// composition's foot, mid and high cells along the heading and the landing above
    /// the last. Legality has already refused one that leaves the facility.
    #[must_use]
    pub fn played_stair(&self, target: HexCoord, rotation: u8) -> [HexPlacement; 4] {
        observed_facility::hex_wfc::authored_climb(
            self.world.config,
            target,
            lateral_face(rotation),
        )
        .expect("legality proved the climb fits the facility")
    }

    pub fn submit(&mut self, command: ArchitectCommand) -> Result<(), CommandRefusal> {
        self.submit_for_faction(command, None)
    }

    /// Loyal callers must supply their own hand and knowledge context. `None`
    /// denotes the shared Rogue faction, independent of human/bot input source.
    pub(crate) fn submit_for_faction(
        &mut self,
        command: ArchitectCommand,
        team: Option<TeamId>,
    ) -> Result<(), CommandRefusal> {
        if let ArchitectCommand::Play { card, .. } = command
            && team.is_some()
            && self
                .deck
                .hand
                .iter()
                .any(|held| held.id == card && held.kind.rogue_only())
        {
            return Err(CommandRefusal::RogueOnly);
        }
        if let Some(refusal) = self.refusal(command) {
            return Err(refusal);
        }
        if let ArchitectCommand::Play { card, target, .. } = command
            && let Some(kind) = self
                .deck
                .hand
                .iter()
                .find(|held| held.id == card)
                .map(|held| held.kind)
                .filter(|kind| kind.rogue_only())
        {
            // An order, not architecture: nothing is built, nothing is disturbed.
            assert!(self.deck.spend(card), "legality proved the card is held");
            if self.has_setup_allowance() {
                self.setup_placements_left -= 1;
            } else {
                self.cooldown = ARCHITECT_COOLDOWN_TICKS;
            }
            self.command_log.push((self.tick, command));
            match kind {
                CardKind::Directive => self.direct(target),
                CardKind::Sensor => self.sense(target),
                CardKind::Surge => self.surge(target),
                _ => unreachable!("only Rogue effects reach this branch"),
            }
            return Ok(());
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
                        self.rewrite(self.played_placement(shape, target, rotation));
                        self.retracted.remove(&target);
                        self.doors
                            .retain(|key, _| !threshold_touches(*key, target, &self.world));
                    }
                    CardKind::Door => {
                        let key = self
                            .threshold_key(target, lateral_face(rotation))
                            .expect("legality proved the threshold exists");
                        self.doors.insert(key, DoorState::Closed);
                    }
                    CardKind::Station => {
                        self.economy.stations.insert(target);
                    }
                    CardKind::Stair => {
                        for placement in self.played_stair(target, rotation) {
                            let cell = placement.coord;
                            self.rewrite(placement);
                            self.retracted.remove(&cell);
                            self.doors
                                .retain(|key, _| !threshold_touches(*key, cell, &self.world));
                        }
                    }
                    CardKind::Directive | CardKind::Sensor | CardKind::Surge => {
                        unreachable!("the Rogue's orders are played above")
                    }
                }
                assert!(self.deck.spend(card), "legality proved the card is held");
                if self.has_setup_allowance() {
                    self.setup_placements_left -= 1;
                } else {
                    self.cooldown = ARCHITECT_COOLDOWN_TICKS;
                }
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
                    .on_card_played(target.level, is_contradiction, team.is_none());
                self.resolve_disturbance_waves(target.level);
                self.record_event(
                    LabEventKind::Played,
                    Some(target),
                    "Card played. Guardian investigates this tile.",
                );
            }
            ArchitectCommand::Requisition => {
                let floor = team.and_then(|team| {
                    self.observers
                        .values()
                        .filter(|o| o.team == team && o.state != ObserverState::Corrupted)
                        .min_by_key(|o| (o.state != ObserverState::Active, o.id))
                        .map(|o| o.cell.level)
                });
                if let Some(floor) = floor {
                    crate::ascent::requisition::apply_requisition_on_floor(self, floor);
                } else {
                    crate::ascent::requisition::apply_requisition(self);
                }
            }
        }
        Ok(())
    }

    pub fn tick(&mut self) {
        self.tick_with_observers(&BTreeMap::new());
    }

    /// Advance the shared clock. Listed Observers are controlled externally;
    /// an explicit neutral command holds their pose instead of handing them to AI.
    /// Commands are applied each fixed tick; unlisted bots retain their beat cadence.
    pub fn tick_with_observers(&mut self, commands: &BTreeMap<ObserverId, ObserverCommand>) {
        if self.outcome != MatchOutcome::Running {
            return;
        }
        self.tick += 1;
        for (&id, &command) in commands {
            let _ = self.submit_observer(id, command);
        }
        if !commands.is_empty() {
            self.refresh_observation();
        }
        self.cooldown = self.cooldown.saturating_sub(1);
        self.keep_directive();
        self.keep_sensors();
        self.advance_retraction();
        self.resolve_falls();
        if self.outcome != MatchOutcome::Running {
            return;
        }
        if !self.tick.is_multiple_of(u64::from(ACTOR_BEAT_TICKS)) {
            return;
        }

        if self.bot_architect {
            // On a first-person facility the lab's Rogue, which previews every play on a
            // copy of the rules, costs seconds a decision; the local one does not.
            let (command, trace) = if self.authored {
                self.rogue_intent()
            } else {
                self.architect_intent()
            };
            self.traces.insert("Architect".to_string(), trace);
            if let Some(command) = command {
                let _ = self.submit(command);
            }
        }

        let observer_ids: Vec<_> = self.observers.keys().copied().collect();
        for id in observer_ids {
            if commands.contains_key(&id) {
                continue;
            }
            let (intent, trace) = self.observer_intent(id);
            self.traces.insert(format!("Observer {}", id.0), trace);
            self.apply_observer_intent(id, intent);
        }
        self.economy
            .tick_beat_except(&self.world, &self.observers, &self.embodied);
        self.refresh_observation();

        let guardian_ids: Vec<_> = self
            .guardians
            .keys()
            .copied()
            .filter(|id| !self.embodied_guardians.contains(id))
            .collect();
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

        // 1b. The facility goes unwitnessed. Evaluated once per beat, which is the only
        // cadence at which observation changes. The streak accrues in every match so the
        // instrument can report how close Darkness came; it can only *end* a match when
        // the Rogue was given that objective. Ordered with the other Rogue condition and
        // ahead of the summit, matching the capture-before-summit precedence above: the
        // hold has been running for beats, the summit arrival happens on this one.
        let darkness_completed = self.darkness.observe(self.facility_is_dark(), self.tick);
        if darkness_completed && self.objective == RogueObjective::Darkness {
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
                    self.summit_team = Some(team);
                    return;
                }
            }
        }
    }

    pub fn resolve_falls(&mut self) -> Vec<crate::ascent::falls::FallEvent> {
        crate::ascent::falls::resolve_falls(self)
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
                    placement.space.built()
                        || HexFace::LATERAL.into_iter().any(|face| {
                            self.world
                                .config
                                .grid()
                                .neighbor(*cell, face)
                                .and_then(|next| self.world.placements.get(&next))
                                .is_some_and(|tile| {
                                    tile.space.built() && tile.is_open(face.opposite())
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
        if placement.space.unbuilt() {
            return Vec::new();
        }
        HexFace::ALL
            .into_iter()
            .filter_map(|face| {
                let next = self.world.config.grid().neighbor(from, face)?;
                let other = self.world.placements.get(&next)?;
                if other.space.unbuilt() {
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
                    // A lab board's ascent rooms are powered lifts, inert on a dark floor
                    // (design section 5). A first-person facility is climbed by walked
                    // stairs and ramps, which no power failure stops, so its routes keep
                    // them: the rules agree with the bodies.
                    if !self.authored
                        && (!self.economy.is_powered(from.level)
                            || !self.economy.is_powered(next.level))
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
        self.seen.clear();
        let mut active = 0usize;
        let mut lit = 0usize;
        let watchers: Vec<(ObserverId, HexCoord, HexFace)> = self
            .observers
            .values()
            .filter(|observer| observer.state == ObserverState::Active)
            .map(|observer| (observer.id, observer.cell, observer.facing))
            .collect();
        for (id, cell, facing) in watchers {
            active += 1;
            self.observed.insert(cell);
            self.seen.insert(cell);
            if let Some(sight) = self.sight.get(&id) {
                // A body's real sight: what it sees near enough wards, all of it is seen.
                // A dark floor still costs it everything beyond its own cell.
                if self.economy.is_powered(cell.level) {
                    let mut warded = false;
                    for (&at, &distance) in sight {
                        self.seen.insert(at);
                        if distance <= WARD_REACH && at != cell {
                            self.observed.insert(at);
                            warded = true;
                        }
                    }
                    lit += usize::from(warded);
                }
                continue;
            }
            if self.economy.is_powered(cell.level) {
                // Warding: the short set, unchanged. This is what stops a retraction.
                if let Some(next) = self.step_through(cell, facing) {
                    lit += 1;
                    self.observed.insert(next);
                }
                // Seeing: further, and across open air. Feeds knowledge, not protection.
                for at in self.sight_along(cell, facing) {
                    self.seen.insert(at);
                }
            }
        }
        self.active_observers = active;
        self.lit_sightlines = lit;
        self.ward_what_would_redraw();
        self.update_team_knowledge();
    }

    /// Nobody is looking outward: Observers remain, and none of them lights the cell they
    /// face, because their floor has lost power or they are facing a wall.
    ///
    /// The "at least one Active Observer" clause is load-bearing. An empty `observed` set
    /// means every Observer is jailed or corrupted, which is the Purge victory checked in
    /// the same tick — so without this clause Darkness would be a second name for a match
    /// that has already ended, and could never fire on its own.
    #[must_use]
    pub const fn facility_is_dark(&self) -> bool {
        self.active_observers > 0 && self.lit_sightlines == 0
    }

    /// Updates team-scoped knowledge for all active teams.
    ///
    /// §10: "Loyal knowledge never leaks undiscovered structure or actors."
    pub fn update_team_knowledge(&mut self) {
        let teams: BTreeSet<TeamId> = self.observers.values().map(|o| o.team).collect();
        for team in teams {
            // Team-local sight determines both structural freshness and actor visibility.
            // Global warding is a mutation constraint, not permission to see rivals.
            let mut team_observed = BTreeSet::new();
            // A jailed body is in a space of its own and sees nothing of the facility.
            for observer in self.observers.values().filter(|o| {
                o.team == team
                    && o.state != ObserverState::Corrupted
                    && !(o.state == ObserverState::Jailed && self.embodied.contains(&o.id))
            }) {
                team_observed.insert(observer.cell);
                if self.economy.is_powered(observer.cell.level) {
                    match self.sight.get(&observer.id) {
                        Some(sight) => team_observed.extend(sight.keys().copied()),
                        None => {
                            team_observed.extend(self.sight_along(observer.cell, observer.facing));
                        }
                    }
                }
            }
            let known_obs = self
                .observers
                .values()
                .filter(|observer| observer.team == team || team_observed.contains(&observer.cell))
                .map(|observer| (observer.id, observer.cell))
                .collect();

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
            for cell in &team_observed {
                if let Some(placement) = self.world.placements.get(cell) {
                    tk.cells.insert(
                        *cell,
                        KnownCell {
                            placement: *placement,
                            seen_at: self.tick,
                        },
                    );
                }
            }
            tk.discovered_cells.extend(team_observed.iter().copied());
            tk.visible_cells = team_observed;
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
        let detected = self.rogue_detected();
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

    /// What the Rogue board shows, in the shape a team's board reads: the facility's truth,
    /// every cell as it stands now and seen now, every Guardian, and of the loyal Observers
    /// only those [`Self::rogue_knowledge`] allows - jailed, corrupted, or detected by a
    /// Guardian or a sensor. A human at the Rogue board reads this, never the teams' knowledge.
    #[must_use]
    pub fn rogue_view(&self) -> TeamKnowledge {
        let rogue = self.rogue_knowledge();
        let cells: BTreeMap<HexCoord, KnownCell> = self
            .world
            .placements
            .iter()
            .map(|(&cell, &placement)| {
                (
                    cell,
                    KnownCell {
                        placement,
                        seen_at: self.tick,
                    },
                )
            })
            .collect();
        TeamKnowledge {
            visible_cells: cells.keys().copied().collect(),
            discovered_cells: rogue.cells,
            cells,
            team: TeamId(u8::MAX),
            known_observers: rogue.known_observers,
            visible_guardians: rogue.guardians,
        }
    }

    /// Cells an Observer at `from` can see looking along `face`.
    ///
    /// Sight is **not** movement. `step_through` answers "where could I walk", and using
    /// it for vision is why an Observer could never see out of a window or across an
    /// atrium — the role defined by observing had the shortest sightline in the game, two
    /// cells, while a Guardian sees six along each of six faces.
    ///
    /// This walks the facing instead, passing through [`HexSpace::Air`] and stopping at
    /// the first thing that blocks: built structure (which is seen, then ends the line),
    /// unbuilt rock, or the edge of the lattice. Range is capped because the measurement
    /// said it saturates — see [`OBSERVER_SIGHT_RANGE`].
    #[must_use]
    pub fn sight_along(&self, from: HexCoord, face: HexFace) -> Vec<HexCoord> {
        let grid = self.world.config.grid();
        let mut seen = Vec::new();
        let mut at = from;
        for _ in 0..OBSERVER_SIGHT_RANGE {
            let Some(next) = grid.neighbor(at, face) else {
                break;
            };
            at = next;
            let Some(placement) = self.world.placements.get(&at) else {
                break;
            };
            match placement.space {
                // Rock is opaque and is not itself a sight.
                HexSpace::Void => break,
                // Air is seen through and not worth noting as a cell you observed.
                HexSpace::Air => {}
                // Structure: you see it, and nothing behind it.
                HexSpace::Room | HexSpace::Hall => {
                    seen.push(at);
                    break;
                }
            }
        }
        seen
    }

    fn step_through(&self, from: HexCoord, face: HexFace) -> Option<HexCoord> {
        self.exits(from)
            .into_iter()
            .find(|&next| self.world.config.grid().neighbor(from, face) == Some(next))
    }

    fn refresh_contradictions(&mut self) {
        self.contradictions.clear();
        for (&cell, placement) in &self.world.placements {
            if placement.space.unbuilt() || self.collapsed_floors.contains(&cell.level) {
                continue;
            }
            for face in HexFace::ALL {
                let Some(next) = self.world.config.grid().neighbor(cell, face) else {
                    continue;
                };
                let Some(other) = self.world.placements.get(&next) else {
                    continue;
                };
                if (other.space.built()
                    && !ports_compatible(
                        placement.ports().port(face),
                        other.ports().port(face.opposite()),
                    ))
                    || (self.retracted.contains(&next)
                        && placement.ports().port(face) != observed_hex::PortClass::Sealed)
                {
                    self.contradictions.insert(cell);
                    if other.space.built() {
                        self.contradictions.insert(next);
                    }
                }
            }
        }
    }
}

/// The architecture register of floor `level` of a facility `levels` tall: its district's.
#[must_use]
pub fn floor_register(level: u8, levels: u8) -> observed_content::ArchitectureRegister {
    District::for_floor(level, levels).register()
}

/// A floor's title: the place, then how it is built, as in `BACKROOMS // LIMINAL GRID`.
#[must_use]
pub fn floor_title(level: u8, levels: u8) -> String {
    let district = District::for_floor(level, levels);
    format!(
        "{} // {}",
        district.label().to_ascii_uppercase(),
        district.register().label().to_ascii_uppercase()
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod pressure_tests;

#[cfg(test)]
mod playtest_instrument;
