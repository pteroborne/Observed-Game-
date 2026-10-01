//! The Observer's kinetic tool on the facility: a push or a pull that shoves a minor
//! Guardian along the crosshair, or a plumb that turns its down to the way the tool is
//! armed, so that the architecture can take it.
//!
//! The tool kills nothing. A minor it shoves slides, and whatever it slides off (an open
//! edge, a retracted cell, a stairwell) is what destroys it, through the fall a released
//! minor already dies of (`released`).
//!
//! Fixed-tick, never physics-engine timing: a shot sets the minor's controller body moving
//! along the look, and a staggered minor slides under one fixed friction on the same
//! controller that walks it, so the same inputs put it in the same place on every peer.
//! The tool acts on minors only. It never touches a rival Observer, a teammate or a major.
//!
//! Charge is not the match's. In an Ascent match the rules own every Observer's pool: they
//! clear a shot the pool cannot pay for before the bodies move, and spend on the
//! [`HexMatchEventKind::KineticPush`] and [`HexMatchEventKind::KineticPull`] events a shot
//! that lands raises. A shot that finds nothing raises nothing and costs nothing.

use glam::{Vec2, Vec3};
use observed_core::PlayerId;

use super::{
    HexActionButtons, HexMatchEvent, HexMatchEventKind, HexPlumbAim, HexReleasedGuardian,
    HexWfcMatch,
};

/// How far the crosshair reaches, in metres from the eye.
pub const KINETIC_REACH: f32 = 8.0;
/// The speed a push sends a minor away along the look, m/s.
pub const KINETIC_PUSH_SPEED: f32 = 10.0;
/// The speed a pull draws a minor back along the look, m/s.
pub const KINETIC_PULL_SPEED: f32 = 8.0;
/// Ticks after a shot before the tool fires again.
pub const KINETIC_COOLDOWN_TICKS: u8 = 15;
/// The longest a shove keeps a minor from walking, in ticks. It usually recovers sooner,
/// as soon as it has slid to a stop.
pub const KINETIC_STAGGER_TICKS: u16 = 75;
/// The friction a staggered minor slides under, m/s^2, on the ground and in the air: a
/// push slides one about five and a half metres on the level.
pub const KINETIC_STAGGER_FRICTION: f32 = 9.0;
/// How long a plumb holds a minor, ticks: four seconds, as in `wfc_kinetic_lab`.
pub const PLUMB_TICKS: u32 = 240;
/// Ticks after a plumb before the tool fires again: longer than a shove's, because it owns
/// which way the minor falls for four seconds.
pub const PLUMB_COOLDOWN_TICKS: u8 = 45;
/// How far outside a minor's capsule the crosshair still selects it, metres.
const AIM_TOLERANCE: f32 = 0.15;

/// What the tool does to the minor it selects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexKineticVerb {
    /// Away from the Observer, along the look.
    Push,
    /// Back toward the Observer, against the look.
    Pull,
}

impl HexKineticVerb {
    /// The verb `actions` asks for this tick, if any. A push wins a tick both are pressed.
    #[must_use]
    pub const fn from_actions(actions: HexActionButtons) -> Option<Self> {
        if actions.kinetic_push {
            Some(Self::Push)
        } else if actions.kinetic_pull {
            Some(Self::Pull)
        } else {
            None
        }
    }
}

/// The minor the crosshair selects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HexKineticTarget {
    /// The released Guardian's id.
    pub guardian: u16,
    /// Metres from the eye.
    pub distance: f32,
    /// Where the crosshair meets it.
    pub point: Vec3,
}

impl HexWfcMatch {
    /// Where a body's eye is and which way it looks, while it walks the facility.
    #[must_use]
    pub fn eye_and_look(&self, player: PlayerId) -> Option<(Vec3, Vec3)> {
        if !self.players.get(&player)?.in_facility() {
            return None;
        }
        let body = self.bodies.get(&player)?;
        let config = self.content.traversal_profile().controller();
        Some((body.eye(&config), body.look_dir()))
    }

    /// The minor `player`'s crosshair selects: the nearest along the look, within
    /// [`KINETIC_REACH`], with nothing solid in front of it. Ties go to the lower id.
    #[must_use]
    pub fn kinetic_target(&self, player: PlayerId) -> Option<HexKineticTarget> {
        let (eye, look) = self.eye_and_look(player)?;
        let config = self.content.traversal_profile().controller();
        let radius = config.radius + AIM_TOLERANCE;
        let half_height = config.half_height + AIM_TOLERANCE;
        let mut best: Option<HexKineticTarget> = None;
        for (&id, guardian) in &self.released {
            let HexReleasedGuardian::Minor(minor) = guardian else {
                continue;
            };
            let Some(distance) = ray_meets_upright(eye, look, minor.position, radius, half_height)
            else {
                continue;
            };
            if distance > KINETIC_REACH || best.is_some_and(|best| distance >= best.distance) {
                continue;
            }
            best = Some(HexKineticTarget {
                guardian: id,
                distance,
                point: eye + look * distance,
            });
        }
        let target = best?;
        // Something solid between the eye and the minor stops the shot.
        match self.physics.ray_distance(eye, look, target.distance) {
            Some(wall) if wall < target.distance => None,
            _ => Some(target),
        }
    }

    /// Radians a unit of look turns a body: what a dial of the plumb turns it by too, so
    /// dialling feels like looking.
    #[must_use]
    pub fn look_step(&self) -> f32 {
        self.content.traversal_profile().controller().look_step
    }

    /// Ticks before `player`'s tool fires again.
    #[must_use]
    pub fn kinetic_cooldown(&self, player: PlayerId) -> u8 {
        self.kinetic_cooldowns.get(&player).copied().unwrap_or(0)
    }

    /// Fire `player`'s tool, if this tick's command asks it to and it is ready: a plumb if
    /// one is asked for, else a push or a pull. A shot that selects nothing does nothing:
    /// no cooldown, no event.
    pub(super) fn step_kinetic_actions(
        &mut self,
        player: PlayerId,
        actions: HexActionButtons,
        plumb: Option<HexPlumbAim>,
    ) {
        if let Some(cooldown) = self.kinetic_cooldowns.get_mut(&player) {
            *cooldown = cooldown.saturating_sub(1);
        }
        if let Some(aim) = plumb {
            self.fire_plumb(player, aim);
            return;
        }
        let Some(verb) = HexKineticVerb::from_actions(actions) else {
            return;
        };
        if self.kinetic_cooldown(player) > 0 {
            return;
        }
        let Some(target) = self.kinetic_target(player) else {
            return;
        };
        let Some((_, look)) = self.eye_and_look(player) else {
            return;
        };
        let Some(HexReleasedGuardian::Minor(minor)) = self.released.get_mut(&target.guardian)
        else {
            return;
        };
        let velocity = match verb {
            HexKineticVerb::Push => look * KINETIC_PUSH_SPEED,
            HexKineticVerb::Pull => -look * KINETIC_PULL_SPEED,
        };
        minor.shove(velocity);
        let cell = minor.cell;
        self.kinetic_cooldowns
            .insert(player, KINETIC_COOLDOWN_TICKS);
        self.recent_events.push(HexMatchEvent {
            tick: self.tick,
            kind: match verb {
                HexKineticVerb::Push => HexMatchEventKind::KineticPush,
                HexKineticVerb::Pull => HexMatchEventKind::KineticPull,
            },
            player: Some(player),
            cell: Some(cell),
        });
    }
}

impl HexWfcMatch {
    /// Plumb the minor `player`'s crosshair selects, armed `aim` about the way the body
    /// faces: its down becomes that direction for [`PLUMB_TICKS`].
    fn fire_plumb(&mut self, player: PlayerId, aim: HexPlumbAim) {
        if self.kinetic_cooldown(player) > 0 {
            return;
        }
        let Some(target) = self.kinetic_target(player) else {
            return;
        };
        let yaw = self.bodies[&player].yaw;
        let down = aim.in_world(yaw);
        let config = self.content.traversal_profile().controller();
        let Some(HexReleasedGuardian::Minor(minor)) = self.released.get_mut(&target.guardian)
        else {
            return;
        };
        if !minor.plumb(&self.physics, &config, down, PLUMB_TICKS) {
            return;
        }
        let cell = minor.cell;
        self.kinetic_cooldowns.insert(player, PLUMB_COOLDOWN_TICKS);
        self.recent_events.push(HexMatchEvent {
            tick: self.tick,
            kind: HexMatchEventKind::KineticPlumb,
            player: Some(player),
            cell: Some(cell),
        });
    }
}

/// How far along the ray from `origin` in `direction` (normalised) it first meets an
/// upright cylinder around `center`, of `radius` and reaching `half_height` above and
/// below it: a minor's capsule, near enough for aim. `None` if it never does ahead.
fn ray_meets_upright(
    origin: Vec3,
    direction: Vec3,
    center: Vec3,
    radius: f32,
    half_height: f32,
) -> Option<f32> {
    // The span of the ray inside the cylinder's height.
    let (mut enter, mut leave) = if direction.y.abs() < 1e-6 {
        if (origin.y - center.y).abs() > half_height {
            return None;
        }
        (0.0, f32::INFINITY)
    } else {
        let a = (center.y - half_height - origin.y) / direction.y;
        let b = (center.y + half_height - origin.y) / direction.y;
        (a.min(b), a.max(b))
    };
    // And within its radius, in plan.
    let offset = Vec2::new(origin.x - center.x, origin.z - center.z);
    let plan = Vec2::new(direction.x, direction.z);
    let a = plan.length_squared();
    let c = offset.length_squared() - radius * radius;
    if a < 1e-9 {
        if c > 0.0 {
            return None;
        }
    } else {
        let b = offset.dot(plan);
        let discriminant = b * b - a * c;
        if discriminant < 0.0 {
            return None;
        }
        let root = discriminant.sqrt();
        enter = enter.max((-b - root) / a);
        leave = leave.min((-b + root) / a);
    }
    let enter = enter.max(0.0);
    (enter <= leave).then_some(enter)
}

#[cfg(test)]
impl HexWfcMatch {
    /// Turn `player`'s body to look straight at `at`.
    pub(crate) fn aim_body_for_tests(&mut self, player: PlayerId, at: Vec3) {
        let (eye, _) = self.eye_and_look(player).expect("a body in the facility");
        let offset = at - eye;
        let body = self.bodies.get_mut(&player).expect("a body");
        body.yaw = offset.x.atan2(-offset.z);
        body.pitch = offset.y.atan2(offset.with_y(0.0).length());
    }

    /// Stand released minor `id` at rest at `position`, a body's centre.
    pub(crate) fn stand_minor_for_tests(&mut self, id: u16, position: Vec3) {
        let Some(HexReleasedGuardian::Minor(minor)) = self.released.get_mut(&id) else {
            panic!("no minor {id}");
        };
        minor.stand_at(position);
    }

    /// Stand `player`'s body at rest with its feet at `feet`, in `cell`.
    pub(crate) fn stand_body_for_tests(
        &mut self,
        player: PlayerId,
        cell: observed_hex::HexCoord,
        feet: Vec3,
    ) {
        let config = self.content.traversal_profile().controller();
        let position = feet + Vec3::Y * (config.half_height + 0.02);
        let yaw = self.bodies[&player].yaw;
        self.bodies
            .insert(player, observed_traversal::FpsBody::spawned(position, yaw));
        self.players.get_mut(&player).expect("a player").cell = cell;
        self.sync_player_from_body(player);
    }

    /// Where `player`'s body is, its centre.
    pub(crate) fn body_position_for_tests(&self, player: PlayerId) -> Vec3 {
        self.bodies[&player].position
    }

    /// How far along `direction` from `from` the first solid thing is, within `reach`.
    pub(crate) fn solid_along_for_tests(
        &self,
        from: Vec3,
        direction: Vec3,
        reach: f32,
    ) -> Option<f32> {
        self.physics.ray_distance(from, direction, reach)
    }
}

mod drops;
pub use drops::HexKillingPush;
#[cfg(test)]
mod edges;
#[cfg(test)]
mod tests;
