//! The turn-based match: pure rules, no Bevy, no rendering, no `Entity`.
//!
//! This is the shipped game's loop — observe to freeze, spend what you have,
//! watch the unobserved rewire — expressed one turn at a time over a whole
//! visible board. It reuses the real facility solver and the real
//! observation-safe relayout rather than modelling them, so a conclusion drawn
//! here is a conclusion about the game and not about a toy.
//!
//! What it deliberately drops: bodies, physics, continuous position, and the
//! sub-room placement of objectives. A unit is a cell. That is the whole
//! simplification, and everything else follows from it.
//!
//! # Reuse, and where it stops
//!
//! `HexLanternState` and `HexPadState` are the shipped equipment rules, driven
//! here by synthesizing each unit's world position from its cell
//! (`hex_origin(cell)`). Their radius tests then compare a position against
//! itself and pass exactly when the unit and the item share a cell, which is the
//! turn-based rule stated in the terms the real one already speaks.
//!
//! Two things could not be borrowed. The Guardian's cadence is per-tick and
//! becomes per-turn ([`adversary`]), and the fog recording is per-hop rather than
//! the shipped fixed one hop ([`knowledge`]). Both are documented where they live.

pub mod action;
pub mod adversary;
pub mod knowledge;
pub mod objectives;
pub mod relayout;
pub mod unit;
pub mod vision;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;
use observed_core::{PlayerId, TeamId};
use observed_facility::hex_wfc::{
    HexObservationFrame, HexRelayoutDelta, HexThresholdKey, HexWfcError, HexWfcWorld,
};
use observed_facility::map_spec::RoomRole;
use observed_hex::{HexCoord, hex_origin};
use observed_match::hex_wfc::{
    HexGuardianStatus, HexLanternState, HexPadState, HexPlayerMapKnowledge,
};

use crate::settings::MatchSettings;

use action::{LoggedAction, Refusal, TacticsAction};
use adversary::Guardian;
use objectives::{ObjectiveState, room_at};
use relayout::{ShiftOutcome, Telegraph};
use unit::{PLAYER_TEAM, RIVAL_TEAM, Unit};

/// The port name every lab anchor is filed under.
///
/// The shipped game anchors a *named doorway* inside a room. A unit here anchors
/// the **cell it stands in**, which is the honest turn-based reduction — there is
/// no facing, so there is no doorway to choose. Reusing [`HexThresholdKey`] with
/// a per-cell key and this constant port keeps `HexLanternState`'s duplicate
/// rejection working (one anchor per cell) and keeps `apply_mutation_pins`
/// pinning the right cell.
const CELL_ANCHOR_PORT: &str = "cell_anchor";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnPhase {
    /// The squad is spending its action points.
    Acting,
    /// The match is over; nothing more is accepted.
    Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchStatus {
    Running,
    /// The player's squad got out.
    Escaped,
    /// The rival squad got out first.
    Outrun,
}

/// One deterministic step in a pointer-requested route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoveStep {
    pub face: observed_hex::HexFace,
    pub to: HexCoord,
}

/// What clicking a destination would do before it mutates the match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MovePreview {
    pub requested: HexCoord,
    pub start: HexCoord,
    pub steps: Vec<MoveStep>,
    pub affordable_steps: usize,
    pub stopping_cell: HexCoord,
    pub reaches_destination: bool,
    pub action_point_cost: u16,
    pub refusal: Option<Refusal>,
}

impl MovePreview {
    #[must_use]
    pub fn executable_steps(&self) -> &[MoveStep] {
        &self.steps[..self.affordable_steps]
    }

    #[must_use]
    pub const fn can_move(&self) -> bool {
        self.affordable_steps > 0
    }
}

/// One action's simulation-owned availability for command presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionAvailability {
    pub action: TacticsAction,
    pub cost: u8,
    pub enabled: bool,
    pub refusal: Option<Refusal>,
}

/// End-of-turn result plus the logical delta presentation must project.
#[derive(Clone, Debug)]
pub struct TurnResolution {
    pub outcome: ShiftOutcome,
    pub relayout: Option<HexRelayoutDelta>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TacticsEvent {
    Moved {
        unit: PlayerId,
        to: HexCoord,
    },
    KeystoneTaken {
        unit: PlayerId,
        cell: HexCoord,
    },
    StationProgress {
        team: TeamId,
        turns: u8,
    },
    StationComplete {
        team: TeamId,
    },
    MonitorSurveyed {
        unit: PlayerId,
        cell: HexCoord,
    },
    CacheCollected {
        unit: PlayerId,
        cell: HexCoord,
    },
    AnchorDeployed {
        unit: PlayerId,
        cell: HexCoord,
    },
    AnchorRecovered {
        unit: PlayerId,
        cell: HexCoord,
    },
    PadDeployed {
        unit: PlayerId,
        cell: HexCoord,
    },
    PadTraversed {
        unit: PlayerId,
        from: HexCoord,
        to: HexCoord,
    },
    /// The Guardian reached a unit and set it back.
    GuardianCaught {
        unit: PlayerId,
        to: HexCoord,
    },
    ExitDenied {
        unit: PlayerId,
    },
    UnitEscaped {
        unit: PlayerId,
    },
    FacilityShifted {
        cells: usize,
    },
    /// The squad was holding the pocket, so nothing changed.
    FacilityHeld,
    MatchFinished {
        status: MatchStatus,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TacticsError {
    /// The solver could not produce a facility for these settings.
    Facility(HexWfcError),
    /// The squad size is outside what a match accepts.
    InvalidSquad,
    /// The independently configured floor count is outside the setup range.
    InvalidFloors,
    /// A step costs nothing, which every "spend until you cannot" loop in the
    /// match — the rival's turn, the click-to-walk path — would take as licence
    /// to move forever. Rejected at construction rather than defended against in
    /// each loop, because it is a property of the configuration and not of the
    /// loops.
    FreeMovement,
    /// The configuration asks for more distinct keystones than the generated
    /// facility contains. Refusing it prevents an unwinnable match.
    InsufficientKeystones { required: u8, available: u8 },
}

impl From<HexWfcError> for TacticsError {
    fn from(value: HexWfcError) -> Self {
        Self::Facility(value)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TeamState {
    pub escaped: bool,
    pub finish_turn: Option<u32>,
}

/// One turn-based match.
#[derive(Clone, Debug)]
pub struct TacticsGame {
    pub settings: MatchSettings,
    pub turn: u32,
    pub phase: TurnPhase,
    pub status: MatchStatus,
    pub world: HexWfcWorld,
    pub units: BTreeMap<PlayerId, Unit>,
    pub teams: BTreeMap<TeamId, TeamState>,
    pub knowledge: BTreeMap<TeamId, HexPlayerMapKnowledge>,
    pub anchors: HexLanternState,
    pub pads: HexPadState,
    pub guardian: Option<Guardian>,
    pub objectives: ObjectiveState,
    /// The frame the last relayout was judged against. Presentation reads it to
    /// draw what is currently held.
    pub observation: HexObservationFrame,
    /// The pocket that will re-collapse when this turn ends.
    pub telegraph: Option<Telegraph>,
    pub last_shift: Option<ShiftOutcome>,
    /// Cells rewritten by the most recent successful shift. This is simulation
    /// debug state, not inferred from meshes, and lets full-map presentation
    /// show a change even when the player had never discovered that pocket.
    pub last_shifted_cells: BTreeSet<HexCoord>,
    /// Everything that has happened since the last turn boundary: the squad's
    /// own actions as they are taken, then the resolution of the turn they end.
    /// Cleared by [`Self::end_turn`], so an event never outlives the moment it
    /// describes.
    pub events: Vec<TacticsEvent>,
    /// Every action taken, in order. A match replays from settings plus this.
    pub log: Vec<LoggedAction>,
}

impl TacticsGame {
    pub fn new(settings: MatchSettings) -> Result<Self, TacticsError> {
        if !(crate::settings::MIN_SQUAD..=crate::settings::MAX_SQUAD).contains(&settings.squad_size)
        {
            return Err(TacticsError::InvalidSquad);
        }
        if !(crate::settings::MIN_FLOORS..=crate::settings::MAX_FLOORS).contains(&settings.floors) {
            return Err(TacticsError::InvalidFloors);
        }
        if settings.costs.move_step == 0 {
            return Err(TacticsError::FreeMovement);
        }
        let composition = settings.composition_profile();
        let world = HexWfcWorld::generate_with_profile(
            settings.seed,
            settings.facility(),
            None,
            &composition,
        )?;
        let available_keystones = objectives::rooms_with_role(&world, RoomRole::Keystone).count();
        let available_keystones = u8::try_from(available_keystones).unwrap_or(u8::MAX);
        if settings.objectives.keystones() && settings.keystones_required > available_keystones {
            return Err(TacticsError::InsufficientKeystones {
                required: settings.keystones_required,
                available: available_keystones,
            });
        }
        let spawn = world.config.spawn();

        let mut units = BTreeMap::new();
        for index in 0..settings.squad_size {
            let id = PlayerId(u16::from(index));
            units.insert(
                id,
                Unit::new(id, PLAYER_TEAM, spawn, settings.action_points),
            );
        }
        if settings.rival_team {
            // Rivals are numbered above the whole player squad so a unit's ID
            // never changes meaning when the squad size setting moves.
            for index in 0..settings.squad_size {
                let id = PlayerId(u16::from(crate::settings::MAX_SQUAD) + u16::from(index));
                units.insert(id, Unit::new(id, RIVAL_TEAM, spawn, settings.action_points));
            }
        }

        let teams: BTreeMap<TeamId, TeamState> = units
            .values()
            .map(|unit| (unit.team, TeamState::default()))
            .collect();
        let anchors = HexLanternState::new(units.keys().copied(), &world);
        let pads = HexPadState::new(units.keys().copied());
        let guardian = (settings.guardian.steps_per_turn() > 0).then(|| Guardian::new(&world));
        let objectives = ObjectiveState::new(&world, teams.keys().copied());
        let knowledge = teams
            .keys()
            .map(|&team| (team, HexPlayerMapKnowledge::default()))
            .collect();

        let mut game = Self {
            settings,
            turn: 1,
            phase: TurnPhase::Acting,
            status: MatchStatus::Running,
            world,
            units,
            teams,
            knowledge,
            anchors,
            pads,
            guardian,
            objectives,
            observation: HexObservationFrame::default(),
            telegraph: None,
            last_shift: None,
            last_shifted_cells: BTreeSet::new(),
            events: Vec::new(),
            log: Vec::new(),
        };
        game.observation = game.build_observation();
        game.refresh_knowledge();
        game.arm_telegraph();
        Ok(game)
    }

    // ---------------------------------------------------------------- actions

    /// Everything `id` could legally do right now.
    ///
    /// This is what the view draws *and* what [`Self::apply`] enforces — see the
    /// note in [`action`]. Anything absent from this list is refused.
    #[must_use]
    pub fn legal_actions(&self, id: PlayerId) -> Vec<TacticsAction> {
        let Some(unit) = self.units.get(&id) else {
            return Vec::new();
        };
        if self.phase == TurnPhase::Finished || unit.escaped {
            return Vec::new();
        }
        let costs = &self.settings.costs;
        let mut actions = Vec::new();
        for (face, _) in vision::exits(&self.world, unit.cell) {
            let action = TacticsAction::Move(face);
            if unit.can_afford(action.cost(costs)) {
                actions.push(action);
            }
        }
        if unit.can_afford(costs.interact) && self.interaction_at(unit).is_some() {
            actions.push(TacticsAction::Interact);
        }
        if self.settings.anchors {
            if unit.can_afford(costs.anchor)
                && self.anchors.inventory(id) > 0
                && self.anchor_in(unit.cell).is_none()
            {
                actions.push(TacticsAction::DeployAnchor);
            }
            if unit.can_afford(costs.interact)
                && self.anchor_in(unit.cell).is_some_and(|owner| owner == id)
            {
                actions.push(TacticsAction::RecoverAnchor);
            }
        }
        if self.settings.pads
            && unit.can_afford(costs.pad)
            && self.pads.inventory(id) > 0
            && self
                .pads
                .pad_under(unit.team, unit.cell, cell_position(unit.cell))
                .is_none()
        {
            actions.push(TacticsAction::DeployPad);
        }
        actions
    }

    /// Query one action without changing the match. Presentation derives both
    /// enabled and disabled controls from this simulation-owned answer.
    #[must_use]
    pub fn action_availability(&self, id: PlayerId, action: TacticsAction) -> ActionAvailability {
        let cost = action.cost(&self.settings.costs);
        let refusal = match self.units.get(&id) {
            None => Some(Refusal::NoSuchUnit),
            Some(_) if self.phase == TurnPhase::Finished => Some(Refusal::NotYourTurn),
            Some(unit) if unit.escaped => Some(Refusal::UnitEscaped),
            Some(unit) if !unit.can_afford(cost) => Some(Refusal::NotEnoughActionPoints),
            Some(_) if self.legal_actions(id).contains(&action) => None,
            Some(_) => Some(match action {
                TacticsAction::Move(_) => Refusal::Blocked,
                TacticsAction::Interact => Refusal::NothingHere,
                _ => Refusal::Unavailable,
            }),
        };
        ActionAvailability {
            action,
            cost,
            enabled: refusal.is_none(),
            refusal,
        }
    }

    /// Preview the exact route a destination click will execute, including the
    /// affordable prefix and the point where this turn runs out of AP.
    #[must_use]
    pub fn preview_move(&self, id: PlayerId, requested: HexCoord) -> MovePreview {
        let Some(unit) = self.units.get(&id).copied() else {
            return MovePreview {
                requested,
                start: requested,
                steps: Vec::new(),
                affordable_steps: 0,
                stopping_cell: requested,
                reaches_destination: false,
                action_point_cost: 0,
                refusal: Some(Refusal::NoSuchUnit),
            };
        };
        let mut visited = vec![unit.cell];
        let mut steps = Vec::new();
        let mut cursor = unit.cell;
        while cursor != requested && steps.len() < self.world.placements.len() {
            let Some(next) = adversary::advance_toward(&self.world, cursor, requested) else {
                break;
            };
            if visited.contains(&next) {
                break;
            }
            let Some(face) = vision::exits(&self.world, cursor)
                .into_iter()
                .find(|&(_, destination)| destination == next)
                .map(|(face, _)| face)
            else {
                break;
            };
            steps.push(MoveStep { face, to: next });
            visited.push(next);
            cursor = next;
        }
        let move_cost = usize::from(self.settings.costs.move_step);
        let affordable_steps = steps
            .len()
            .min(usize::from(unit.action_points) / move_cost.max(1));
        let stopping_cell = steps
            .get(affordable_steps.saturating_sub(1))
            .map_or(unit.cell, |step| step.to);
        let refusal = if self.phase == TurnPhase::Finished {
            Some(Refusal::NotYourTurn)
        } else if unit.escaped {
            Some(Refusal::UnitEscaped)
        } else if steps.is_empty() && unit.cell != requested {
            Some(Refusal::Blocked)
        } else if affordable_steps == 0 && unit.cell != requested {
            Some(Refusal::NotEnoughActionPoints)
        } else {
            None
        };
        MovePreview {
            requested,
            start: unit.cell,
            steps,
            affordable_steps,
            stopping_cell,
            reaches_destination: cursor == requested,
            action_point_cost: u16::try_from(affordable_steps * move_cost).unwrap_or(u16::MAX),
            refusal,
        }
    }

    /// Perform one action for one unit.
    pub fn apply(&mut self, id: PlayerId, action: TacticsAction) -> Result<(), Refusal> {
        if self.phase == TurnPhase::Finished {
            return Err(Refusal::NotYourTurn);
        }
        let unit = *self.units.get(&id).ok_or(Refusal::NoSuchUnit)?;
        if unit.escaped {
            return Err(Refusal::UnitEscaped);
        }
        let cost = action.cost(&self.settings.costs);
        if !unit.can_afford(cost) {
            return Err(Refusal::NotEnoughActionPoints);
        }
        if !self.legal_actions(id).contains(&action) {
            return Err(match action {
                TacticsAction::Move(_) => Refusal::Blocked,
                TacticsAction::Interact => Refusal::NothingHere,
                _ => Refusal::Unavailable,
            });
        }
        match action {
            TacticsAction::Move(face) => self.do_move(id, face),
            TacticsAction::Interact => self.do_interact(id),
            TacticsAction::DeployAnchor => self.do_deploy_anchor(id),
            TacticsAction::RecoverAnchor => self.do_recover_anchor(id),
            TacticsAction::DeployPad => self.do_deploy_pad(id),
        }
        self.units.get_mut(&id).expect("unit").spend(cost);
        self.log.push(LoggedAction {
            turn: self.turn,
            unit: id,
            action,
        });
        self.observation = self.build_observation();
        self.refresh_knowledge();
        Ok(())
    }

    fn do_move(&mut self, id: PlayerId, face: observed_hex::HexFace) {
        let unit = self.units[&id];
        let Some(destination) = vision::step_through(&self.world, unit.cell, face) else {
            return;
        };
        self.units.get_mut(&id).expect("unit").cell = destination;
        self.events.push(TacticsEvent::Moved {
            unit: id,
            to: destination,
        });
        self.resolve_pad_contact(id);
        self.resolve_escape(id);
    }

    fn do_interact(&mut self, id: PlayerId) {
        let unit = self.units[&id];
        match self.interaction_at(&unit) {
            Some(Interaction::Keystone(room)) => {
                self.objectives.available_keystones.remove(&room);
                let progress = self.objectives.team_mut(unit.team);
                progress.keystones = progress.keystones.saturating_add(1);
                self.events.push(TacticsEvent::KeystoneTaken {
                    unit: id,
                    cell: unit.cell,
                });
            }
            Some(Interaction::Monitor(room)) => {
                self.objectives.team_mut(unit.team).surveyed.insert(room);
                if let Some(map) = self.knowledge.get_mut(&unit.team) {
                    knowledge::survey(map, &self.world, unit.cell, objectives::MONITOR_SURVEY_HOPS);
                }
                self.events.push(TacticsEvent::MonitorSurveyed {
                    unit: id,
                    cell: unit.cell,
                });
            }
            Some(Interaction::Cache) if self.anchors.collect_cache(id, unit.cell).is_some() => {
                self.events.push(TacticsEvent::CacheCollected {
                    unit: id,
                    cell: unit.cell,
                });
            }
            Some(Interaction::Cache) => {}
            None => {}
        }
    }

    fn do_deploy_anchor(&mut self, id: PlayerId) {
        let unit = self.units[&id];
        if self
            .anchors
            .deploy(
                id,
                cell_anchor_key(unit.cell),
                unit.cell,
                cell_position(unit.cell),
            )
            .is_some()
        {
            self.events.push(TacticsEvent::AnchorDeployed {
                unit: id,
                cell: unit.cell,
            });
        }
    }

    fn do_recover_anchor(&mut self, id: PlayerId) {
        let unit = self.units[&id];
        // A radius of one metre against positions that are both exactly the
        // cell centre: this recovers the anchor in *this* cell and no other,
        // since neighbouring cell centres are twelve metres away.
        if self
            .anchors
            .recover_nearest(id, cell_position(unit.cell), 1.0)
            .is_some()
        {
            self.events.push(TacticsEvent::AnchorRecovered {
                unit: id,
                cell: unit.cell,
            });
        }
    }

    fn do_deploy_pad(&mut self, id: PlayerId) {
        let unit = self.units[&id];
        if self
            .pads
            .deploy(id, unit.team, unit.cell, cell_position(unit.cell))
            .is_some()
        {
            self.events.push(TacticsEvent::PadDeployed {
                unit: id,
                cell: unit.cell,
            });
        }
    }

    /// Standing on a linked plate moves you to its pair.
    fn resolve_pad_contact(&mut self, id: PlayerId) {
        if !self.settings.pads || self.pads.is_suppressed(id) {
            return;
        }
        let unit = self.units[&id];
        let Some(pad) = self
            .pads
            .pad_under(unit.team, unit.cell, cell_position(unit.cell))
            .map(|pad| pad.id)
        else {
            return;
        };
        let Some(destination) = self.pads.link_target(unit.team, pad).map(|pad| pad.cell) else {
            return;
        };
        let source = unit.cell;
        self.units.get_mut(&id).expect("unit").cell = destination;
        self.pads.suppress(id);
        self.events.push(TacticsEvent::PadTraversed {
            unit: id,
            from: source,
            to: destination,
        });
        self.resolve_escape(id);
    }

    // ------------------------------------------------------------------ turns

    /// End the squad's turn: rivals move, the Guardian hunts, the facility
    /// shifts, and a new pocket is telegraphed.
    pub fn end_turn(&mut self) -> ShiftOutcome {
        self.end_turn_detailed().outcome
    }

    /// Detailed turn boundary for authored presentation. Existing headless
    /// callers can continue using [`Self::end_turn`] when they need only the
    /// gameplay outcome.
    pub fn end_turn_detailed(&mut self) -> TurnResolution {
        if self.phase == TurnPhase::Finished {
            return TurnResolution {
                outcome: ShiftOutcome::NothingToShift,
                relayout: None,
            };
        }
        self.events.clear();
        self.step_rivals();
        self.step_guardian();
        self.step_stations();
        // Before the observation frame, so a squad that just qualified leaves
        // now rather than standing in the doorway for a turn.
        self.resolve_standing_escapes();
        self.observation = self.build_observation();
        let (outcome, relayout) = self.commit_shift();
        self.last_shift = Some(outcome);
        self.last_shifted_cells = relayout
            .as_ref()
            .map(|delta| delta.changed_cells.iter().copied().collect())
            .unwrap_or_default();
        self.refresh_knowledge();
        self.resolve_all_escapes();

        self.turn = self.turn.saturating_add(1);
        for unit in self.units.values_mut() {
            unit.action_points = self.settings.action_points;
        }
        // Pad re-arm is measured in ticks by the shipped rule; a turn is the
        // whole of the equivalent window here, so it clears wholesale rather
        // than counting sixty ticks that will never be stepped.
        self.pads.suppressed.clear();
        if self.phase != TurnPhase::Finished {
            self.arm_telegraph();
        }
        TurnResolution { outcome, relayout }
    }

    fn step_rivals(&mut self) {
        if !self.settings.rival_team {
            return;
        }
        let authorized = self.team_authorized(RIVAL_TEAM);
        let ids: Vec<PlayerId> = self
            .units
            .values()
            .filter(|unit| unit.team == RIVAL_TEAM && !unit.escaped)
            .map(|unit| unit.id)
            .collect();
        for id in ids {
            let mut budget = self.settings.action_points;
            while budget >= self.settings.costs.move_step {
                let unit = self.units[&id];
                let goal = adversary::rival_goal(&self.world, &unit, authorized);
                if unit.cell == goal {
                    // Standing on what it wants: take it, if taking is a thing
                    // that can be done here.
                    if self.interaction_at(&unit).is_some() {
                        self.do_interact(id);
                    }
                    break;
                }
                let Some(next) = adversary::advance_toward(&self.world, unit.cell, goal) else {
                    break;
                };
                self.units.get_mut(&id).expect("unit").cell = next;
                budget -= self.settings.costs.move_step;
            }
            self.resolve_escape(id);
        }
    }

    fn step_guardian(&mut self) {
        let steps = self.settings.guardian.steps_per_turn();
        let Some(mut guardian) = self.guardian else {
            return;
        };
        let quarry: Vec<Unit> = self
            .units
            .values()
            .filter(|unit| unit.team == PLAYER_TEAM && !unit.escaped)
            .copied()
            .collect();
        let observed = self.observed_cells();
        let anchored = self.anchored_cells();
        guardian.step(&self.world, &quarry, steps, &observed, &anchored);
        self.guardian = Some(guardian);

        // A catch is a setback, not a death: the unit wakes up in a recovery
        // room, or at the spawn when the solve placed none. Losing a squad member
        // outright would make the Guardian the only thing in the match worth
        // thinking about.
        let caught: Vec<PlayerId> = quarry
            .iter()
            .filter(|unit| unit.cell == guardian.cell)
            .map(|unit| unit.id)
            .collect();
        if caught.is_empty() {
            return;
        }
        let recovery = self
            .world
            .blueprints
            .iter()
            .find(|blueprint| blueprint.role == RoomRole::Recovery)
            .map(|blueprint| blueprint.anchor)
            .unwrap_or_else(|| self.world.config.spawn());
        for id in caught {
            self.units.get_mut(&id).expect("unit").cell = recovery;
            self.events.push(TacticsEvent::GuardianCaught {
                unit: id,
                to: recovery,
            });
        }
    }

    /// A station needs two units of the same squad standing in it, for a run of
    /// consecutive turns. Breaking the pairing resets the count — the whole point
    /// of the room is that it costs two people's time at once.
    fn step_stations(&mut self) {
        if !self.settings.objectives.stations() {
            return;
        }
        let teams: Vec<TeamId> = self.teams.keys().copied().collect();
        for team in teams {
            if self.objectives.team(team).station_complete {
                continue;
            }
            let held = objectives::rooms_with_role(&self.world, RoomRole::DualStation).any(
                |(_, _, cells)| {
                    self.units
                        .values()
                        .filter(|unit| {
                            unit.team == team && !unit.escaped && cells.contains(&unit.cell)
                        })
                        .count()
                        >= 2
                },
            );
            let required = self.settings.station_hold_turns;
            let progress = self.objectives.team_mut(team);
            if !held {
                progress.station_turns = 0;
                continue;
            }
            progress.station_turns = progress.station_turns.saturating_add(1);
            if progress.station_turns >= required {
                progress.station_complete = true;
                self.events.push(TacticsEvent::StationComplete { team });
            } else {
                let turns = progress.station_turns;
                self.events
                    .push(TacticsEvent::StationProgress { team, turns });
            }
        }
    }

    fn commit_shift(&mut self) -> (ShiftOutcome, Option<HexRelayoutDelta>) {
        let Some(telegraph) = self.telegraph.take() else {
            return (ShiftOutcome::NothingToShift, None);
        };
        let (outcome, delta) = relayout::commit(&mut self.world, telegraph, &self.observation);
        match outcome {
            ShiftOutcome::Committed => {
                let cells = delta.as_ref().map_or(0, |delta| delta.changed_cells.len());
                self.events.push(TacticsEvent::FacilityShifted { cells });
            }
            ShiftOutcome::Held => self.events.push(TacticsEvent::FacilityHeld),
            ShiftOutcome::NothingToShift => {}
        }
        (outcome, delta)
    }

    /// Choose and solve the pocket that will change at the end of this turn.
    fn arm_telegraph(&mut self) {
        self.telegraph = None;
        let Some(interval) = self.settings.shift.interval() else {
            return;
        };
        if !self.turn.is_multiple_of(interval) {
            return;
        }
        let frontier = self.discovered_cells();
        if frontier.is_empty() {
            return;
        }
        self.telegraph = relayout::telegraph(
            &self.world,
            &self.observation,
            &frontier,
            self.settings.decoherence_coverage,
        );
    }

    // ------------------------------------------------------------------ state

    /// The observation frame the facility is judged against.
    ///
    /// Mirrors `HexWfcMatch::build_observation`: occupied cells, everything the
    /// squads can see, the anchors' durable pins, every stamped room as a
    /// landmark, the Guardian's cell, and the objectives whose routes must
    /// survive a commit.
    #[must_use]
    pub fn build_observation(&self) -> HexObservationFrame {
        let mut frame = HexObservationFrame::default();
        for unit in self.units.values().filter(|unit| !unit.escaped) {
            frame.occupied_cells.insert(unit.id, unit.cell);
            frame.visible_cells.extend(vision::observed_from(
                &self.world,
                unit.cell,
                self.settings.sight.hops(),
            ));
        }
        self.anchors.apply_mutation_pins(&mut frame);
        vision::seed_landmarks(&self.world, &mut frame);
        if let Some(guardian) = self.guardian {
            frame.landmark_cells.insert(guardian.cell);
        }
        frame.objective_cells = objectives::outstanding_cells(
            &self.world,
            &self.objectives,
            PLAYER_TEAM,
            self.keystones_required(),
            self.settings.objectives.stations(),
        );
        frame
    }

    /// Cells the player's squad currently observes.
    #[must_use]
    pub fn observed_cells(&self) -> BTreeSet<HexCoord> {
        self.units
            .values()
            .filter(|unit| unit.team == PLAYER_TEAM && !unit.escaped)
            .flat_map(|unit| {
                vision::observed_from(&self.world, unit.cell, self.settings.sight.hops())
            })
            .collect()
    }

    /// Cells held by a deployed anchor.
    #[must_use]
    pub fn anchored_cells(&self) -> BTreeSet<HexCoord> {
        self.anchors
            .deployed
            .values()
            .map(|anchor| anchor.cell)
            .collect()
    }

    /// Everything the player's squad has ever seen — the relayout frontier.
    #[must_use]
    pub fn discovered_cells(&self) -> BTreeSet<HexCoord> {
        self.knowledge
            .get(&PLAYER_TEAM)
            .map(|map| map.cells.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Who owns the anchor in `cell`, if there is one.
    #[must_use]
    pub fn anchor_in(&self, cell: HexCoord) -> Option<PlayerId> {
        self.anchors
            .deployed
            .values()
            .find(|anchor| anchor.cell == cell)
            .map(|anchor| anchor.owner)
    }

    #[must_use]
    pub fn keystones_required(&self) -> u8 {
        if self.settings.objectives.keystones() {
            self.settings.keystones_required
        } else {
            0
        }
    }

    /// Whether `team` may leave through the exit.
    #[must_use]
    pub fn team_authorized(&self, team: TeamId) -> bool {
        let progress = self.objectives.team(team);
        progress.keystones >= self.keystones_required()
            && (!self.settings.objectives.stations() || progress.station_complete)
    }

    #[must_use]
    pub fn guardian_status(&self) -> Option<HexGuardianStatus> {
        self.guardian.map(|guardian| guardian.status)
    }

    fn refresh_knowledge(&mut self) {
        let anchored = self.anchored_cells();
        let teams: Vec<TeamId> = self.teams.keys().copied().collect();
        for team in teams {
            let occupied: BTreeSet<HexCoord> = self
                .units
                .values()
                .filter(|unit| unit.team == team && !unit.escaped)
                .map(|unit| unit.cell)
                .collect();
            let observed: BTreeSet<HexCoord> = occupied
                .iter()
                .flat_map(|&cell| {
                    vision::observed_from(&self.world, cell, self.settings.sight.hops())
                })
                .collect();
            let map = self.knowledge.entry(team).or_default();
            knowledge::record_turn(map, &self.world, &occupied, &observed, &anchored);
        }
    }

    /// Re-check every unit against the exit.
    ///
    /// Escaping cannot be a property of *moving onto* the exit alone. A unit that
    /// arrives before its squad is authorized — or that is put there by a pad or
    /// a Guardian setback — would otherwise stand in an open doorway forever,
    /// waiting for a step it has no reason to take. The condition is "standing on
    /// the exit, authorized", so it is evaluated wherever either half can change.
    fn resolve_standing_escapes(&mut self) {
        let waiting: Vec<PlayerId> = self
            .units
            .values()
            .filter(|unit| !unit.escaped && unit.cell == self.world.config.exit())
            .map(|unit| unit.id)
            .collect();
        for id in waiting {
            self.resolve_escape(id);
        }
    }

    fn resolve_escape(&mut self, id: PlayerId) {
        let unit = self.units[&id];
        if unit.escaped || unit.cell != self.world.config.exit() {
            return;
        }
        if !self.team_authorized(unit.team) {
            self.events.push(TacticsEvent::ExitDenied { unit: id });
            return;
        }
        self.units.get_mut(&id).expect("unit").escaped = true;
        self.events.push(TacticsEvent::UnitEscaped { unit: id });
        self.resolve_all_escapes();
    }

    /// A squad is out when every one of its units is out.
    fn resolve_all_escapes(&mut self) {
        let teams: Vec<TeamId> = self.teams.keys().copied().collect();
        for team in teams {
            if self.teams[&team].escaped {
                continue;
            }
            let all_out = self
                .units
                .values()
                .filter(|unit| unit.team == team)
                .all(|unit| unit.escaped);
            if !all_out {
                continue;
            }
            let turn = self.turn;
            let state = self.teams.get_mut(&team).expect("team");
            state.escaped = true;
            state.finish_turn = Some(turn);
            if self.status == MatchStatus::Running {
                self.status = if team == PLAYER_TEAM {
                    MatchStatus::Escaped
                } else {
                    MatchStatus::Outrun
                };
                self.phase = TurnPhase::Finished;
                let status = self.status;
                self.events.push(TacticsEvent::MatchFinished { status });
            }
        }
    }

    /// What `Interact` would do here, if anything.
    fn interaction_at(&self, unit: &Unit) -> Option<Interaction> {
        if self
            .anchors
            .caches
            .values()
            .any(|cache| !cache.collected && cache.cell == unit.cell)
        {
            return Some(Interaction::Cache);
        }
        let (room, role, _) = room_at(&self.world, unit.cell)?;
        match role {
            RoomRole::Keystone
                if self.settings.objectives.keystones()
                    && self.objectives.available_keystones.contains(&room)
                    && self.objectives.team(unit.team).keystones < self.keystones_required() =>
            {
                Some(Interaction::Keystone(room))
            }
            RoomRole::Monitor if !self.objectives.team(unit.team).surveyed.contains(&room) => {
                Some(Interaction::Monitor(room))
            }
            _ => None,
        }
    }

    /// A stable summary of the whole match state, for determinism assertions.
    ///
    /// Deliberately hand-rolled over the fields that define a position rather
    /// than derived: it has to be obvious at a glance what a digest covers, and
    /// a derive would silently start covering caches and other derived state.
    #[must_use]
    pub fn digest(&self) -> u64 {
        let mut hash = 0xCBF2_9CE4_8422_2325u64;
        let mut mix = |value: u64| {
            hash = (hash ^ value).wrapping_mul(0x0000_0100_0000_01B3);
        };
        mix(u64::from(self.turn));
        mix(u64::from(self.world.generation));
        for unit in self.units.values() {
            mix(u64::from(unit.id.0));
            mix(cell_key(unit.cell));
            mix(u64::from(unit.action_points));
            mix(u64::from(unit.escaped));
        }
        for (&cell, placement) in &self.world.placements {
            mix(cell_key(cell));
            mix(u64::from(placement.doors));
            mix(placement.archetype as u64);
        }
        for anchor in self.anchors.deployed.values() {
            mix(cell_key(anchor.cell));
        }
        for pad in self.pads.deployed.values() {
            mix(cell_key(pad.cell));
        }
        if let Some(guardian) = self.guardian {
            mix(cell_key(guardian.cell));
        }
        for (team, progress) in &self.objectives.teams {
            mix(u64::from(team.0));
            mix(u64::from(progress.keystones));
            mix(u64::from(progress.station_turns));
        }
        hash
    }
}

/// What interacting in a cell would produce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Interaction {
    Keystone(u64),
    Monitor(u64),
    Cache,
}

/// A cell's world position: the shipped equipment rules are written against
/// positions, and a turn-based unit is exactly at its cell's centre.
#[must_use]
pub fn cell_position(cell: HexCoord) -> Vec3 {
    Vec3::from_array(hex_origin(cell))
}

/// A stable per-cell key, used both for anchor identity and for digests.
#[must_use]
pub fn cell_key(cell: HexCoord) -> u64 {
    u64::from(cell.q) | (u64::from(cell.r) << 16) | (u64::from(cell.level) << 32)
}

/// The threshold identity a cell anchor is filed under. See [`CELL_ANCHOR_PORT`].
#[must_use]
fn cell_anchor_key(cell: HexCoord) -> HexThresholdKey {
    HexThresholdKey {
        room_generation_key: cell_key(cell),
        port: CELL_ANCHOR_PORT,
    }
}
