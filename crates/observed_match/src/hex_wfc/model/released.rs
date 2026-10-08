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
//!   destroyed only by the architecture: a minor that falls out of the facility is gone,
//!   and so is one that falls further than [`MINOR_BREAKING_DROP`] onto whatever is below.
//!   It never enters the prison lobby.
//!
//! Measured on production facilities (`kinetic::edges`), a minor that could only be lost
//! out of the facility was all but unkillable: a push really killed one from under 1% of
//! cells, none above the fourth floor, because the open edges up there hang over lower
//! roofs and a retracted hall drops a minor onto the deck below it. The design sends a
//! minor "off a ledge or unrailed balcony"; a fall is that ledge.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use glam::Vec3;
use observed_core::PlayerId;
use observed_hex::{HexCoord, HexFace, hex_origin};
use observed_traversal::FpsBody;
use observed_traversal::gravity::{BodyFrame, ObserverGravity};
use observed_traversal::rapier_controller::step_character_with_settings;
use player_input::PlayerIntent;

use super::{
    FIXED_DT, FLOOR_SLAB_TOP, HexGuardianState, HexMatchEvent, HexMatchEventKind, HexWfcMatch,
    KINETIC_STAGGER_FRICTION, KINETIC_STAGGER_TICKS,
};

/// Below this plan-view speed, m/s, a grounded staggered minor has stopped and recovers.
const STAGGER_RECOVERED_SPEED: f32 = 0.5;

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
pub(super) const OUT_OF_WORLD_DEPTH: f32 = 4.0;
/// A minor that lands this far, in metres, below the highest point of its fall breaks:
/// more than half a storey (`TILE_LEVEL_HEIGHT` is 8), so a drop through a retracted hall
/// or off a balcony is lethal, and a stair, a ramp or a step down never is.
pub const MINOR_BREAKING_DROP: f32 = 5.0;

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
    /// Ticks left sliding from a kinetic shove, when it neither walks nor catches.
    pub stagger: u16,
    /// The highest it has been since it last stood on something, while it is falling.
    /// A plumb's flight is not a fall: only the world's down breaks a minor.
    fall_peak: Option<f32>,
    /// Which way is down for it: the world's, or a plumb's while one holds it
    /// (`kinetic::plumb`), on the same lifecycle that turns an Observer's.
    gravity: ObserverGravity,
    body: FpsBody,
}

impl HexMinorState {
    /// Whether it is sliding from a kinetic shove.
    #[must_use]
    pub const fn staggered(&self) -> bool {
        self.stagger > 0
    }

    /// Whether a plumb holds it, or it is still turning back upright from one: it neither
    /// walks nor catches until its down is the world's again.
    #[must_use]
    pub fn plumbed(&self) -> bool {
        self.gravity.remaining > 0 || self.gravity.returning || !self.gravity.frame.is_upright()
    }

    /// Its frame as drawn: the plumb's, eased in and out (`ObserverGravity::visual_frame`).
    #[must_use]
    pub fn visual_frame(&self) -> BodyFrame {
        self.gravity.visual_frame()
    }

    /// Its down now, in the world.
    #[must_use]
    pub fn down(&self) -> Vec3 {
        -self.gravity.frame.up()
    }

    /// Plumb it: its down becomes `down` for `ticks`, and it falls that way. It stops
    /// walking and forgets its prey. Refused (`false`) where its body would not fit turned.
    pub(super) fn plumb(
        &mut self,
        scene: &observed_traversal::rapier_controller::RapierTraversalScene,
        config: &observed_traversal::FpsConfig,
        down: Vec3,
        ticks: u32,
    ) -> bool {
        let (gravity, body) = (&mut self.gravity, &mut self.body);
        if !scene.with_query(|query| gravity.activate(query, body, config, down, ticks)) {
            return false;
        }
        self.stagger = 0;
        self.fall_peak = None;
        self.target = None;
        self.waypoint = None;
        true
    }

    /// Stand it at rest at `position`, its body's centre.
    pub(super) fn stand_at(&mut self, position: Vec3) {
        self.body = FpsBody::spawned(position, self.body.yaw);
        self.position = position;
    }

    /// Set it sliding at `velocity`: it stops walking, forgets its prey, and slides under
    /// [`KINETIC_STAGGER_FRICTION`] until it stops or the stagger runs out.
    pub(super) fn shove(&mut self, velocity: Vec3) {
        self.body.velocity = velocity;
        if velocity.y > 0.0 {
            self.body.grounded = false;
        }
        self.stagger = KINETIC_STAGGER_TICKS;
        self.target = None;
        self.waypoint = None;
    }
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
            HexReleasedKind::Major => {
                let mut major = HexGuardianState::at(cell);
                self.prepare_major(&mut major);
                if !major.physically_placed() {
                    return false;
                }
                HexReleasedGuardian::Major(major)
            }
            HexReleasedKind::Minor => {
                let config = self.content.traversal_config();
                let origin = Vec3::from_array(hex_origin(cell));
                let lift = Vec3::Y * (config.half_height + 0.02);
                // A clear spot is only a place to stand if something holds it up: on a
                // climb's flight the floor is metres above the slab, and the clear space
                // under it is a drop out of the facility.
                let position =
                    super::clear_spawn_position(&self.physics, origin, &config, (id % 8) as u8)
                        .filter(|&centre| self.stands_at(centre - lift).is_some())
                        .or_else(|| self.surface_point(cell).map(|feet| feet + lift))
                        .unwrap_or(origin + Vec3::Y * (FLOOR_SLAB_TOP + config.half_height));
                HexReleasedGuardian::Minor(HexMinorState {
                    cell,
                    position,
                    yaw: 0.0,
                    target: None,
                    waypoint: None,
                    stagger: 0,
                    fall_peak: None,
                    gravity: ObserverGravity::default(),
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

    /// Where a body fits standing on `cell`'s own walking surface, found from above: as
    /// near the centre as it can, on a flight's slope as on a level floor. `None` where no
    /// body fits anywhere near the centre.
    fn surface_point(&self, cell: HexCoord) -> Option<Vec3> {
        const RINGS: [f32; 4] = [0.0, 1.5, 3.0, 4.0];
        const STEPS: u8 = 12;
        // Under the lowest ceiling a cell has, a climb's Foot and Mid at 7.5 m.
        const PROBE: f32 = 7.4;
        let top = Vec3::from_array(hex_origin(cell)) + Vec3::Y * PROBE;
        RINGS
            .into_iter()
            .flat_map(|radius| {
                let steps = if radius == 0.0 { 1 } else { STEPS };
                (0..steps).map(move |step| {
                    let angle = f32::from(step) * std::f32::consts::TAU / f32::from(STEPS);
                    Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius)
                })
            })
            .find_map(|offset| {
                let from = top + offset;
                let drop = self.physics.ray_distance(from, Vec3::NEG_Y, PROBE)?;
                self.stands_at(from - Vec3::Y * drop)
            })
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
            match self.released.get(&id).cloned() {
                Some(HexReleasedGuardian::Major(mut major)) => {
                    self.step_major(&mut major);
                    self.released.insert(id, HexReleasedGuardian::Major(major));
                }
                Some(HexReleasedGuardian::Minor(_)) => self.step_minor(id),
                None => {}
            }
        }
    }

    fn step_minor(&mut self, id: u16) {
        let Some(HexReleasedGuardian::Minor(mut minor)) = self.released.get(&id).cloned() else {
            return;
        };
        // A plumbed minor is as good as staggered: it neither walks nor catches.
        let staggered = minor.staggered() || minor.plumbed();
        let replan = (self.tick + u64::from(id)).is_multiple_of(MINOR_REPLAN_TICKS);
        let lost_target = minor.target.is_none_or(|player| !self.huntable(player));
        if !staggered && (replan || lost_target) {
            (minor.target, minor.waypoint) = self.minor_plan(minor.cell);
        }

        // A shoved minor does not walk: it slides, on the ground and through the air,
        // under one fixed friction, on the same controller.
        let target = if staggered { None } else { minor.target };
        let intent = target.map_or_else(PlayerIntent::default, |player| {
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
        let mut controller = profile.controller();
        if staggered {
            controller.ground_decel = KINETIC_STAGGER_FRICTION;
            controller.air_accel = KINETIC_STAGGER_FRICTION;
        }
        let before = minor.body.position.y;
        let mut recovered = false;
        if minor.plumbed() {
            // Down is the plumb's: the same capsule and contract, in the minor's frame.
            let bounds = self.physics.safety_bounds();
            let gravity = &mut minor.gravity;
            let body = &mut minor.body;
            recovered = self.physics.with_query(|query| {
                gravity
                    .step(query, body, PlayerIntent::default(), &controller, bounds)
                    .recovered
            });
        } else {
            let _ = step_character_with_settings(
                &self.physics,
                &mut minor.body,
                intent,
                &controller,
                profile.rapier(),
                FIXED_DT,
            );
        }
        if minor.staggered() {
            let stopped = minor.body.grounded
                && minor.body.velocity.with_y(0.0).length() < STAGGER_RECOVERED_SPEED;
            minor.stagger = if stopped { 0 } else { minor.stagger - 1 };
        }

        // A fall is measured from its highest point to where it lands. A plumb's flight is
        // a lash, not a fall: a minor plumbed into a wall lands on it unhurt, and only when
        // the plumb lets go does it fall, from wherever it then is.
        let height = minor.body.position.y;
        let broke = if minor.plumbed() {
            minor.fall_peak = None;
            false
        } else if minor.body.grounded {
            minor
                .fall_peak
                .take()
                .is_some_and(|peak| peak - height > MINOR_BREAKING_DROP)
        } else {
            let peak = minor.fall_peak.unwrap_or(before).max(height);
            minor.fall_peak = Some(peak);
            false
        };
        let floor_y = self.geometry.arena.floor_y;
        // A body the controller would recover from outside the world has left it.
        if broke
            || recovered
            || !minor.body.position.is_finite()
            || minor.body.position.y < floor_y - OUT_OF_WORLD_DEPTH
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

        if let Some(player) = minor.target.filter(|_| !staggered) {
            let prey = &self.players[&player];
            let offset = prey.position - minor.position;
            // A closed door between them is a door between them, however close.
            if offset.y.abs() <= MINOR_CATCH_HEIGHT
                && offset.with_y(0.0).length() <= MINOR_CATCH_DISTANCE
                && !self.closed_door_between(minor.cell, prey.cell)
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
                        .is_some_and(|other| other.space.built() && other.is_open(face.opposite()))
                    && !self.closed_door_between(cell, neighbour);
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
