//! Guardians released into a match after it began: by a floor's disturbance wave, or as
//! the price of an Architect's requisition.
//!
//! The match's own Guardian ([`super::HexGuardianState`]) hunts from tick zero. These
//! arrive later, each under the id the rules released it with, and live here with
//! bodies, so the rules never hunt with a Guardian nobody can see.
//!
//! - A **major** is the match's Guardian again: a looking problem, frozen by sight.
//! - A **minor** is a doing problem. It walks on a body the same controller moves that
//!   moves the players, so it climbs, collides and falls exactly as they do; it is never
//!   frozen by sight; it chases the nearest body it can detect on its floor; and it is
//!   destroyed only by the architecture: a minor that falls out of the facility is gone.
//!   It never enters the prison lobby.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use glam::Vec3;
use observed_core::PlayerId;
use observed_hex::{HexCoord, HexFace, hex_origin};
use observed_traversal::FpsBody;
use observed_traversal::rapier_controller::step_character_with_settings;
use player_input::PlayerIntent;

use super::{
    FIXED_DT, FLOOR_SLAB_TOP, HexGuardianState, HexMatchEvent, HexMatchEventKind, HexWfcMatch,
};

/// How far a minor detects a body, in steps through its floor's open doors: the same
/// six cells the rules' Guardians see down a line.
pub const MINOR_SIGHT_STEPS: usize = 6;
/// Ticks between a minor's choices of prey and way, staggered by id so a wave does not
/// search on one tick.
const MINOR_REPLAN_TICKS: u64 = 30;
/// A minor walks, where a body can sprint away from it.
const MINOR_PACE: f32 = 0.85;
/// Plan-view reach, in metres, at which a minor has its prey.
const MINOR_CATCH_DISTANCE: f32 = 1.2;
/// Height difference, in metres, beyond which a minor beside its prey is on another deck.
const MINOR_CATCH_HEIGHT: f32 = 1.5;
/// How far below the arena floor a body has fallen out of the facility.
const OUT_OF_WORLD_DEPTH: f32 = 4.0;

/// Which of the two classes a released Guardian is.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HexReleasedKind {
    Major,
    Minor,
}

/// A minor Guardian: a body, and who it is after.
#[derive(Clone, Debug, PartialEq)]
pub struct HexMinorState {
    pub cell: HexCoord,
    pub position: Vec3,
    pub yaw: f32,
    /// The body it is chasing, if it has detected one.
    pub target: Option<PlayerId>,
    /// The next cell on its way to the target, or its own cell once it shares one.
    waypoint: Option<HexCoord>,
    body: FpsBody,
}

/// A Guardian released after tick zero.
#[derive(Clone, Debug, PartialEq)]
pub enum HexReleasedGuardian {
    Major(HexGuardianState),
    Minor(HexMinorState),
}

impl HexReleasedGuardian {
    #[must_use]
    pub const fn kind(&self) -> HexReleasedKind {
        match self {
            Self::Major(_) => HexReleasedKind::Major,
            Self::Minor(_) => HexReleasedKind::Minor,
        }
    }

    #[must_use]
    pub const fn cell(&self) -> HexCoord {
        match self {
            Self::Major(major) => major.cell,
            Self::Minor(minor) => minor.cell,
        }
    }

    /// Where it is drawn: the body's centre for a minor, the Guardian's for a major.
    #[must_use]
    pub const fn position(&self) -> Vec3 {
        match self {
            Self::Major(major) => major.position,
            Self::Minor(minor) => minor.position,
        }
    }
}

impl HexWfcMatch {
    /// Release a Guardian of `kind` into `cell` under `id`, the rules' id for it.
    ///
    /// Refused (`false`) where the id is already in the facility, or the cell is not
    /// built. A minor stands on the cell's floor at a point its body fits.
    pub fn release_guardian(&mut self, id: u16, kind: HexReleasedKind, cell: HexCoord) -> bool {
        let built = self
            .facility
            .placements
            .get(&cell)
            .is_some_and(|placement| placement.space.built());
        if self.released.contains_key(&id) || !built {
            return false;
        }
        let guardian = match kind {
            HexReleasedKind::Major => HexReleasedGuardian::Major(HexGuardianState::at(cell)),
            HexReleasedKind::Minor => {
                let config = self.content.traversal_config();
                let origin = Vec3::from_array(hex_origin(cell));
                let position =
                    super::clear_spawn_position(&self.physics, origin, &config, (id % 8) as u8)
                        .unwrap_or(origin + Vec3::Y * (FLOOR_SLAB_TOP + config.half_height));
                HexReleasedGuardian::Minor(HexMinorState {
                    cell,
                    position,
                    yaw: 0.0,
                    target: None,
                    waypoint: None,
                    body: FpsBody::spawned(position, 0.0),
                })
            }
        };
        self.released.insert(id, guardian);
        self.recent_events.push(HexMatchEvent {
            tick: self.tick,
            kind: HexMatchEventKind::GuardianReleased,
            player: None,
            cell: Some(cell),
        });
        true
    }

    /// Take a released Guardian out of the facility, as a floor's collapse does. Whether
    /// there was one.
    pub fn remove_released(&mut self, id: u16) -> bool {
        self.released.remove(&id).is_some()
    }

    /// One fixed step of every released Guardian, after the match's own.
    pub(super) fn step_released(&mut self) {
        let ids: Vec<u16> = self.released.keys().copied().collect();
        for id in ids {
            match self.released.get_mut(&id) {
                Some(HexReleasedGuardian::Major(major)) => major.step(
                    self.tick,
                    &self.facility,
                    &self.lanterns,
                    &mut self.players,
                    &mut self.recent_events,
                    self.prison.as_ref(),
                ),
                Some(HexReleasedGuardian::Minor(_)) => self.step_minor(id),
                None => {}
            }
        }
    }

    fn step_minor(&mut self, id: u16) {
        let Some(HexReleasedGuardian::Minor(mut minor)) = self.released.get(&id).cloned() else {
            return;
        };
        let replan = (self.tick + u64::from(id)).is_multiple_of(MINOR_REPLAN_TICKS);
        let lost_target = minor.target.is_none_or(|player| !self.huntable(player));
        if replan || lost_target {
            (minor.target, minor.waypoint) = self.minor_plan(minor.cell);
        }

        let intent = minor.target.map_or_else(PlayerIntent::default, |player| {
            let prey = self.players[&player].position;
            let aim = match minor.waypoint {
                Some(next) if next != minor.cell => {
                    let origin = hex_origin(next);
                    Vec3::new(origin[0], minor.body.position.y, origin[2])
                }
                _ => prey,
            };
            super::bot::steer_toward_with_speed(
                minor.body.yaw,
                minor.body.position,
                aim,
                false,
                MINOR_PACE,
            )
        });
        let profile = self.content.traversal_profile();
        let _ = step_character_with_settings(
            &self.physics,
            &mut minor.body,
            intent,
            &profile.controller(),
            profile.rapier(),
            FIXED_DT,
        );

        let floor_y = self.geometry.arena.floor_y;
        if !minor.body.position.is_finite() || minor.body.position.y < floor_y - OUT_OF_WORLD_DEPTH
        {
            // The architecture destroyed it: it went over at the last cell it stood in.
            self.released.remove(&id);
            self.recent_events.push(HexMatchEvent {
                tick: self.tick,
                kind: HexMatchEventKind::GuardianLost,
                player: None,
                cell: Some(minor.cell),
            });
            return;
        }
        minor.position = minor.body.position;
        minor.yaw = minor.body.yaw;
        minor.cell = self.body_cell(minor.position, minor.cell);
        if minor.waypoint == Some(minor.cell) {
            minor.waypoint = None;
        }

        if let Some(player) = minor.target {
            let prey = &self.players[&player];
            let offset = prey.position - minor.position;
            if offset.y.abs() <= MINOR_CATCH_HEIGHT
                && offset.with_y(0.0).length() <= MINOR_CATCH_DISTANCE
            {
                self.recent_events.push(HexMatchEvent {
                    tick: self.tick,
                    kind: HexMatchEventKind::GuardianCatch,
                    player: Some(player),
                    cell: Some(prey.cell),
                });
                minor.target = None;
                minor.waypoint = None;
            }
        }
        self.released.insert(id, HexReleasedGuardian::Minor(minor));
    }

    /// Whether a minor may chase `player`: a body in the facility, out of the lobby.
    fn huntable(&self, player: PlayerId) -> bool {
        self.players.get(&player).is_some_and(|state| {
            state.in_facility()
                && !state.escaped
                && self
                    .prison
                    .as_ref()
                    .is_none_or(|prison| !prison.lobby.contains(&state.cell))
        })
    }

    /// The nearest body a minor on `from` detects, and the next cell toward it: a search
    /// through its floor's open doors, at most [`MINOR_SIGHT_STEPS`] deep, never into the
    /// prison lobby. Ties break by search order, the same on every peer.
    fn minor_plan(&self, from: HexCoord) -> (Option<PlayerId>, Option<HexCoord>) {
        let mut prey: BTreeMap<HexCoord, PlayerId> = BTreeMap::new();
        for player in self.players.values() {
            if player.cell.level == from.level && self.huntable(player.id) {
                prey.entry(player.cell).or_insert(player.id);
            }
        }
        if prey.is_empty() {
            return (None, None);
        }
        let lobby = self.prison.as_ref().map(|prison| &prison.lobby);
        let grid = self.facility.config.grid();
        let mut came_from: BTreeMap<HexCoord, HexCoord> = BTreeMap::from([(from, from)]);
        let mut frontier = VecDeque::from([(from, 0usize)]);
        while let Some((cell, steps)) = frontier.pop_front() {
            if let Some(&player) = prey.get(&cell) {
                let mut next = cell;
                while came_from[&next] != from && next != from {
                    next = came_from[&next];
                }
                return (Some(player), Some(next));
            }
            if steps == MINOR_SIGHT_STEPS {
                continue;
            }
            let Some(placement) = self.facility.placements.get(&cell) else {
                continue;
            };
            for face in HexFace::LATERAL {
                let Some(neighbour) = grid.neighbor(cell, face) else {
                    continue;
                };
                let open = placement.space.built()
                    && placement.is_open(face)
                    && self
                        .facility
                        .placements
                        .get(&neighbour)
                        .is_some_and(|other| other.space.built() && other.is_open(face.opposite()));
                if !open
                    || came_from.contains_key(&neighbour)
                    || lobby.is_some_and(|lobby: &BTreeSet<HexCoord>| lobby.contains(&neighbour))
                {
                    continue;
                }
                came_from.insert(neighbour, cell);
                frontier.push_back((neighbour, steps + 1));
            }
        }
        (None, None)
    }
}

#[cfg(test)]
mod tests;
