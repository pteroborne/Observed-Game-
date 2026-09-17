//! Authoritative fixed-tick kinetic simulation on a solved floor.
//!
//! No camera, no UI, no Bevy entities. The only difference from the authored
//! chamber next door is where the architecture came from: every structural
//! collider in here was projected from a real WFC solve, and the rules read it
//! rather than a hand-written list of boxes.

use std::collections::BTreeMap;
use std::sync::Arc;

use glam::{Quat, Vec3};
use kinetic_lab::physics::{Physics, gv, rv};
use observed_hex::HexCoord;
use observed_traversal::{
    FIXED_DT, FpsBody, FpsConfig, RapierKinematicSettings,
    rapier_controller::step_character_in_query,
};
use player_input::PlayerIntent;
use rapier3d::prelude::*;

use crate::site::{Site, VOID_Y, build_collider};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActorId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Minor,
    Prop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Practice,
    Encounter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Playing,
    Cleared,
    Captured,
    Fell,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Action {
    #[default]
    None,
    Push,
    Pull,
    Interact,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Command {
    pub movement: PlayerIntent,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Empty,
    Blocked,
    TooFar,
    Cooldown,
    EmptyCharge,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub id: ActorId,
    pub distance: f32,
    pub point: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Fired(Action, ActorId, Vec3),
    Refused(Refusal),
    /// A body left the facility. The cell is where it went over, when the fall
    /// started inside the lattice.
    Eliminated(ActorId, Option<HexCoord>),
    Power(bool),
    Retracting,
    Retracted(HexCoord),
    Wave(u8),
    Ended(Outcome),
    Recharge,
}

/// Tool and opposition tuning, adjustable at runtime so the lab can be used to
/// find these numbers rather than to assert them.
///
/// The authored chamber's values did not survive the move, and that is itself a
/// finding. That chamber is a room you can cross in a few strides; a solved
/// floor is 14 m cells whose doorways sit 7 m from the centre. The same 10 m/s
/// shove that sent a body clear across the chamber barely moves it out of the
/// tile it is standing in, so push and pull are both up here. Everything else is
/// unchanged, so a difference in feel is still the architecture talking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub reach: f32,
    pub cost: f32,
    /// Nominal velocity change of a push, in m/s.
    pub push: f32,
    /// Nominal velocity change of a pull, in m/s.
    pub pull: f32,
    pub cooldown: u32,
    pub stagger: u32,
    /// How fast a pursuing minor walks, in m/s.
    pub minor_speed: f32,
    /// How hard it accelerates toward that speed, in m/s^2.
    pub minor_accel: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            reach: 8.,
            cost: 10.,
            push: 17.,
            pull: 13.,
            cooldown: 15,
            stagger: 27,
            minor_speed: 2.5,
            minor_accel: 15.,
        }
    }
}

impl Config {
    /// Scale the tool's output, keeping push and pull in proportion.
    pub fn scale_force(&mut self, factor: f32) {
        self.push = (self.push * factor).clamp(2., 60.);
        self.pull = (self.pull * factor).clamp(2., 60.);
    }

    /// Scale how fast minors close the distance.
    pub fn scale_minor_speed(&mut self, factor: f32) {
        self.minor_speed = (self.minor_speed * factor).clamp(0.4, 12.);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behavior {
    Practice,
    Staggered,
    Airborne,
    Pursue,
    Capture,
}

#[derive(Clone, Debug)]
pub struct Actor {
    pub id: ActorId,
    pub kind: Kind,
    pub body: RigidBodyHandle,
    pub collider: ColliderHandle,
    pub alive: bool,
    pub stagger: u32,
    pub behavior: Behavior,
    pub grounded: bool,
    /// The last cell this body was supported over.
    ///
    /// Reported when it leaves the facility, because that is the interesting
    /// fact: *where it went over*. Sampling the position at the moment it
    /// crosses the void plane instead reports wherever a shove had carried it
    /// on the way down, which with a strong push is a cell it never touched.
    pub last_cell: Option<HexCoord>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
}

/// A whole playable floor plus its rule state.
///
/// A clone is an in-memory continuation snapshot: every durable Rapier
/// structure is cloned with it, and the site behind the `Arc` is immutable, so
/// replaying commands from a clone reproduces the original tick for tick.
#[derive(Clone)]
pub struct WfcKineticWorld {
    pub site: Arc<Site>,
    pub config: Config,
    pub mode: Mode,
    pub outcome: Outcome,
    pub tick: u64,
    pub player: FpsBody,
    pub player_config: FpsConfig,
    pub charge: f32,
    pub powered: bool,
    pub cooldown: u32,
    pub wave: u8,
    pub wave_delay: u32,
    pub retract_warning: Option<u32>,
    /// Whether the retracting cell's geometry is still present.
    pub cell_present: bool,
    pub kills: u32,
    pub events: Vec<Event>,
    pub actors: BTreeMap<ActorId, Actor>,
    pub physics: Physics,
    pub player_handle: RigidBodyHandle,
    pub player_collider: ColliderHandle,
    /// Projected structural colliders by stable ID.
    structural: BTreeMap<u32, ColliderHandle>,
    /// Which stable collider IDs belong to which lattice cell, so a retraction
    /// can remove exactly one tile's geometry and nothing else.
    cell_colliders: BTreeMap<HexCoord, Vec<u32>>,
    next_id: u32,
    nav: Vec<Vec3>,
    edges: Vec<Vec<usize>>,
}

impl WfcKineticWorld {
    #[must_use]
    pub fn new(site: Arc<Site>, mode: Mode) -> Self {
        let mut physics = Physics::default();
        let mut structural = BTreeMap::new();
        for spec in site.colliders() {
            let Some(collider) = build_collider(spec) else {
                continue;
            };
            let mut collider = collider;
            // Tag the projected architecture. Sight, support and navigation all
            // ask "is there a wall here", never "is there a crate here", and the
            // tag is what lets those queries run against the broad phase instead
            // of walking every collider in the facility.
            collider.user_data = STRUCTURAL_TAG | u128::from(spec.id.0);
            structural.insert(spec.id.0, physics.colliders.insert(collider));
        }
        let mut cell_colliders: BTreeMap<HexCoord, Vec<u32>> = BTreeMap::new();
        for piece in &site.snapshot.pieces {
            cell_colliders
                .entry(piece.source_cell)
                .or_default()
                .push(piece.id.0);
        }

        let player_config = FpsConfig::deliberate_rapier();
        // Stand the Observer up facing the way out. Spawning at yaw zero
        // pointed at whichever wall the solver happened to put there.
        let facing = site.spawn_facing;
        let yaw = if facing.length_squared() > 0. {
            facing.x.atan2(-facing.z)
        } else {
            0.
        };
        let mut player = FpsBody::spawned(site.spawn + Vec3::Y * player_config.half_height, yaw);
        player.pitch = -0.06;
        let player_handle = physics.bodies.insert(
            RigidBodyBuilder::kinematic_position_based()
                .translation(rv(player.position))
                .build(),
        );
        let player_collider = physics.colliders.insert_with_parent(
            ColliderBuilder::capsule_y(
                player_config.half_height - player_config.radius,
                player_config.radius,
            )
            .user_data(PLAYER_USER_DATA)
            .build(),
            player_handle,
            &mut physics.bodies,
        );

        let nav = site.nav.clone();
        let mut world = Self {
            site,
            config: Config::default(),
            mode,
            outcome: Outcome::Playing,
            tick: 0,
            player,
            player_config,
            charge: 100.,
            powered: true,
            cooldown: 0,
            wave: 0,
            wave_delay: 300,
            retract_warning: None,
            cell_present: true,
            kills: 0,
            events: Vec::new(),
            actors: BTreeMap::new(),
            physics,
            player_handle,
            player_collider,
            structural,
            cell_colliders,
            next_id: 1000,
            nav,
            edges: Vec::new(),
        };
        world.populate();
        world.physics.step();
        world.rebuild_navigation();
        world
    }

    /// Practice targets and crates, placed on the floor the solver produced.
    fn populate(&mut self) {
        let site = Arc::clone(&self.site);
        if self.mode == Mode::Practice {
            for feet in site.muster.iter().take(3) {
                self.spawn(Kind::Minor, *feet + Vec3::Y * 0.6);
            }
        }
        // Crates go next to the devices, which is where an Observer will be
        // standing when they first want something to push.
        for anchor in [site.station, site.spawn] {
            self.spawn(Kind::Prop, anchor + Vec3::new(1.6, 0.5, 0.8));
        }
    }

    pub fn spawn(&mut self, kind: Kind, position: Vec3) -> ActorId {
        let id = ActorId(self.next_id);
        self.next_id += 1;
        let mut builder = RigidBodyBuilder::dynamic()
            .translation(rv(position))
            .ccd_enabled(true)
            .linear_damping(0.2)
            .angular_damping(1.);
        if kind == Kind::Minor {
            builder = builder.lock_rotations();
        }
        let body = self.physics.bodies.insert(builder.build());
        let size = if kind == Kind::Minor { 0.55 } else { 0.45 };
        let collider = self.physics.colliders.insert_with_parent(
            ColliderBuilder::cuboid(size, size, size)
                .density(if kind == Kind::Minor { 1. } else { 2. })
                .friction(0.4)
                .restitution(0.05)
                .user_data(u128::from(id.0))
                .build(),
            body,
            &mut self.physics.bodies,
        );
        self.physics.bodies[body].recompute_mass_properties_from_colliders(&self.physics.colliders);
        self.physics
            .bodies
            .propagate_modified_body_positions_to_colliders(&mut self.physics.colliders);
        self.actors.insert(
            id,
            Actor {
                id,
                kind,
                body,
                collider,
                alive: true,
                stagger: 0,
                behavior: Behavior::Practice,
                grounded: false,
                last_cell: None,
            },
        );
        id
    }

    #[must_use]
    pub fn pose(&self, id: ActorId) -> Pose {
        let body = &self.physics.bodies[self.actors[&id].body];
        Pose {
            position: gv(body.translation()),
            rotation: Quat::from_array(body.rotation().to_array()),
        }
    }

    #[must_use]
    pub fn velocity(&self, id: ActorId) -> Vec3 {
        gv(self.physics.bodies[self.actors[&id].body].linvel())
    }

    #[must_use]
    pub fn eye(&self) -> Vec3 {
        self.player.eye(&self.player_config)
    }

    /// The body the crosshair currently selects, or why it selects nothing.
    pub fn target(&self) -> Result<Target, Refusal> {
        let origin = self.eye();
        let direction = self.player.look_dir();
        let Some((handle, distance)) =
            self.physics
                .ray(origin, direction, 100., Some(self.player_collider))
        else {
            return Err(Refusal::Empty);
        };
        let actor = self
            .actors
            .values()
            .find(|actor| actor.collider == handle && actor.alive)
            .ok_or(Refusal::Blocked)?;
        if distance > self.config.reach {
            return Err(Refusal::TooFar);
        }
        Ok(Target {
            id: actor.id,
            distance,
            point: origin + direction * distance,
        })
    }

    pub fn fire_ready(&self) -> Result<Target, Refusal> {
        let target = self.target()?;
        if self.cooldown > 0 {
            return Err(Refusal::Cooldown);
        }
        if self.mode == Mode::Encounter && self.charge < self.config.cost {
            return Err(Refusal::EmptyCharge);
        }
        Ok(target)
    }

    fn fire(&mut self, action: Action) {
        match self.fire_ready() {
            Err(reason) => self.events.push(Event::Refused(reason)),
            Ok(target) => {
                let direction = if action == Action::Push {
                    self.player.look_dir()
                } else {
                    (self.eye() - self.pose(target.id).position).normalize_or_zero()
                };
                let speed = if action == Action::Push {
                    self.config.push
                } else {
                    self.config.pull
                };
                let actor = self.actors.get_mut(&target.id).expect("selected actor");
                actor.stagger = self.config.stagger;
                let body = &mut self.physics.bodies[actor.body];
                body.apply_impulse(rv(direction * speed * body.mass()), true);
                if self.mode == Mode::Encounter {
                    self.charge -= self.config.cost;
                }
                self.cooldown = self.config.cooldown;
                self.events
                    .push(Event::Fired(action, target.id, target.point));
            }
        }
    }

    /// The device within reach, if any.
    #[must_use]
    pub fn interaction(&self) -> Option<&'static str> {
        let feet = self.player.position - Vec3::Y * self.player_config.half_height;
        [
            (self.site.generator, "Toggle generator"),
            (self.site.panel, "Retract tile"),
        ]
        .into_iter()
        .filter(|(at, _)| feet.distance(*at) < 2.5)
        .filter(|(at, _)| self.line_clear(self.eye(), *at + Vec3::Y * 1.3))
        .min_by(|(a, _), (b, _)| {
            feet.distance_squared(*a)
                .total_cmp(&feet.distance_squared(*b))
        })
        .map(|(_, label)| label)
    }

    fn interact(&mut self) {
        match self.interaction() {
            Some("Toggle generator") => {
                self.powered = !self.powered;
                self.events.push(Event::Power(self.powered));
            }
            Some("Retract tile") if self.cell_present && self.retract_warning.is_none() => {
                self.retract_warning = Some(120);
                self.events.push(Event::Retracting);
            }
            _ => {}
        }
    }

    /// Remove one tile's geometry. Everything standing on it loses its floor on
    /// the same tick, which is the whole point of retraction being a tile
    /// rather than a plank.
    fn retract(&mut self) {
        let cell = self.site.retracting;
        if let Some(ids) = self.cell_colliders.get(&cell).cloned() {
            for id in ids {
                if let Some(handle) = self.structural.get(&id) {
                    self.physics.colliders[*handle].set_enabled(false);
                }
            }
        }
        self.cell_present = false;
        self.rebuild_navigation();
        self.events.push(Event::Retracted(cell));
    }

    pub fn step(&mut self, command: Command) {
        self.events.clear();
        if self.outcome != Outcome::Playing {
            return;
        }
        self.tick += 1;
        self.cooldown = self.cooldown.saturating_sub(1);
        // Hazards commit before movement so every query this tick sees the same
        // support the player is about to be resolved against.
        if let Some(left) = self.retract_warning {
            if left <= 1 {
                self.retract_warning = None;
                self.retract();
            } else {
                self.retract_warning = Some(left - 1);
            }
        }
        let report = step_character_in_query(
            &self.physics.query(self.player_handle),
            &mut self.player,
            command.movement,
            &self.player_config,
            RapierKinematicSettings::shipped(&self.player_config),
            FIXED_DT,
            (Vec3::ZERO, Vec3::splat(1000.)),
        );
        self.physics.bodies[self.player_handle]
            .set_next_kinematic_translation(rv(self.player.position));
        if report.recovered || self.player.position.y < VOID_Y {
            if self.mode == Mode::Practice {
                self.player.reset();
                self.physics.bodies[self.player_handle]
                    .set_translation(rv(self.player.position), true);
            } else {
                self.finish(Outcome::Fell);
                return;
            }
        }
        match command.action {
            Action::Push | Action::Pull => self.fire(command.action),
            Action::Interact => self.interact(),
            Action::None => {}
        }
        self.think();
        self.physics.step();
        self.resolve_contacts();
        if self.mode == Mode::Encounter {
            let feet = self.player.position - Vec3::Y * self.player_config.half_height;
            if self.powered
                && feet.distance(self.site.station) < 2.2
                && self.line_clear(self.eye(), self.site.station + Vec3::Y * 1.3)
            {
                let before = self.charge;
                self.charge = (self.charge + 25. * FIXED_DT).min(100.);
                if before < 100. && self.charge >= 100. {
                    self.events.push(Event::Recharge);
                }
            }
            if self.outcome == Outcome::Playing {
                self.advance_waves();
            }
        }
    }

    fn finish(&mut self, outcome: Outcome) {
        self.outcome = outcome;
        self.events.push(Event::Ended(outcome));
    }

    fn resolve_contacts(&mut self) {
        let ids: Vec<_> = self.actors.keys().copied().collect();
        for id in ids {
            if !self.actors[&id].alive {
                continue;
            }
            let position = self.pose(id).position;
            let velocity = self.velocity(id);
            if position.y < VOID_Y {
                // Where it went over, falling back to where it went out for a
                // body that was never on the floor to begin with.
                let dropped = self.site.cell_containing(position);
                let actor = self.actors.get_mut(&id).expect("live actor");
                let cell = actor.last_cell.or(dropped);
                actor.alive = false;
                self.physics.bodies[actor.body].set_enabled(false);
                self.physics.colliders[actor.collider].set_enabled(false);
                if actor.kind == Kind::Minor {
                    self.kills += 1;
                    self.events.push(Event::Eliminated(id, cell));
                }
                continue;
            }
            let collider = self.actors[&id].collider;
            // A deck edge can support the front of a body while its centre
            // hangs over nothing. Use real contact normals, not a centre ray.
            let grounded = self
                .physics
                .narrow
                .contact_pairs_with(collider)
                .any(|pair| {
                    let sign = if pair.collider1 == collider { -1. } else { 1. };
                    pair.manifolds.iter().any(|manifold| {
                        !manifold.data.solver_contacts.is_empty()
                            && manifold.data.normal.y * sign > 0.5
                    })
                })
                && velocity.y.abs() < 1.5;
            let contact_visible = self.line_clear(position, self.player.position);
            let over = self.site.cell_containing(position);
            let actor = self.actors.get_mut(&id).expect("live actor");
            actor.grounded = grounded;
            if grounded && let Some(cell) = over {
                actor.last_cell = Some(cell);
            }
            if actor.kind == Kind::Minor && velocity.with_y(0.).length() > 4. && actor.stagger == 0
            {
                actor.stagger = self.config.stagger;
            }
            if self.mode == Mode::Encounter
                && actor.kind == Kind::Minor
                && actor.stagger == 0
                && grounded
                && contact_visible
                && position.distance(self.player.position) < 1.15
            {
                actor.behavior = Behavior::Capture;
                self.finish(Outcome::Captured);
                return;
            }
        }
    }

    fn advance_waves(&mut self) {
        if self
            .actors
            .values()
            .any(|actor| actor.alive && actor.kind == Kind::Minor)
        {
            return;
        }
        if self.wave == 3 {
            self.finish(Outcome::Cleared);
            return;
        }
        if self.wave_delay > 0 {
            self.wave_delay -= 1;
            return;
        }
        self.wave += 1;
        self.wave_delay = 300;
        let count = self.wave as usize + 1;
        let mut choices: Vec<Vec3> = self
            .site
            .muster
            .iter()
            .copied()
            .filter(|feet| feet.distance(self.player.position) > 6.)
            .collect();
        if choices.is_empty() {
            choices = self.site.muster.clone();
        }
        choices.sort_by(|a, b| {
            b.distance_squared(self.player.position)
                .total_cmp(&a.distance_squared(self.player.position))
        });
        // A seven-cell floor has fewer standable cells than a late wave has
        // minors, so muster points cycle rather than capping the wave.
        for index in 0..count {
            let feet = choices[index % choices.len()];
            let jitter = Vec3::new((index % 3) as f32 * 0.8 - 0.8, 0., (index / 3) as f32 * 0.8);
            self.spawn(Kind::Minor, feet + Vec3::Y * 0.6 + jitter);
        }
        self.events.push(Event::Wave(self.wave));
    }

    /// Fixed priority behavior tree: practice -> stagger -> airborne -> pursuit.
    /// Every leaf emits a desired velocity; physics remains the movement
    /// authority, so a minor cannot walk through a wall the solver built.
    fn think(&mut self) {
        // One route solve per tick, shared by every minor, instead of a fresh
        // breadth-first search and two graph entries per minor per tick. They
        // are all chasing the same Observer, so they were all solving the same
        // problem and throwing the answer away.
        let target = self.player.position - Vec3::Y * self.player_config.half_height;
        let goal = self.nearest_waypoint(target);
        let toward_goal = goal.map(|node| self.breadth_first(node));

        let ids: Vec<_> = self.actors.keys().copied().collect();
        for id in ids {
            if !self.actors[&id].alive || self.actors[&id].kind != Kind::Minor {
                continue;
            }
            let actor = &self.actors[&id];
            let behavior = if actor.stagger > 0 {
                Behavior::Staggered
            } else if !actor.grounded {
                Behavior::Airborne
            } else if self.mode == Mode::Practice {
                Behavior::Practice
            } else {
                Behavior::Pursue
            };
            let desired = if behavior == Behavior::Pursue {
                self.steer(
                    self.pose(id).position - Vec3::Y * 0.55,
                    target,
                    toward_goal.as_deref(),
                )
            } else {
                Vec3::ZERO
            };
            let speed = self.config.minor_speed;
            let accel = self.config.minor_accel;
            let stepped = if behavior == Behavior::Pursue {
                Some(self.physics.walk_minor(actor.body, desired * speed))
            } else {
                None
            };
            let actor = self.actors.get_mut(&id).expect("live minor");
            actor.behavior = behavior;
            actor.stagger = actor.stagger.saturating_sub(1);
            if matches!(behavior, Behavior::Pursue | Behavior::Practice) {
                let body = &mut self.physics.bodies[actor.body];
                let old = body.linvel();
                let horizontal =
                    Vec3::new(old.x, 0., old.z).move_towards(desired * speed, accel * FIXED_DT);
                let velocity = stepped.unwrap_or(horizontal);
                // Autostep is a collision-checked positional correction.
                // Turning its rise into velocity would launch a body upstairs.
                if velocity.y > 0. {
                    body.set_translation(
                        body.translation() + Vector::Y * velocity.y * FIXED_DT,
                        true,
                    );
                }
                body.set_linvel(
                    Vector::new(
                        velocity.x,
                        if velocity.y > 0. { 0. } else { old.y },
                        velocity.z,
                    ),
                    true,
                );
            }
        }
    }

    /// Run a query against the projected architecture only, through the broad
    /// phase.
    ///
    /// The linear version of this walked all ~150 structural colliders per call,
    /// and navigation calls it tens of thousands of times a tick: it cost 18 ms
    /// a tick with two minors on the floor and a 134 ms freeze whenever the
    /// graph was rebuilt. The BVH already exists — it is what the character
    /// controller steps against — so this just asks it the same question.
    fn structural_query<R>(&self, act: impl FnOnce(&QueryPipeline<'_>) -> R) -> R {
        let predicate = |_handle: ColliderHandle, collider: &Collider| {
            collider.is_enabled() && collider.user_data >= STRUCTURAL_TAG
        };
        let query = self.physics.broad.as_query_pipeline(
            self.physics.narrow.query_dispatcher(),
            &self.physics.bodies,
            &self.physics.colliders,
            QueryFilter::default().predicate(&predicate),
        );
        act(&query)
    }

    /// Whether nothing structural stands between two points.
    #[must_use]
    pub fn line_clear(&self, from: Vec3, to: Vec3) -> bool {
        let offset = to - from;
        let distance = offset.length();
        if distance < 0.001 {
            return true;
        }
        let ray = Ray::new(rv(from), rv(offset / distance));
        self.structural_query(|query| query.cast_ray(&ray, distance, true).is_none())
    }

    fn support_height(&self, feet: Vec3) -> Option<f32> {
        let ray = Ray::new(rv(feet + Vec3::Y * 0.4), -Vector::Y);
        self.structural_query(|query| query.cast_ray(&ray, 1.2, true))
            .map(|(_, distance)| feet.y + 0.4 - distance)
    }

    fn supported(&self, feet: Vec3) -> bool {
        self.support_height(feet).is_some()
    }

    fn walkable(&self, a: Vec3, b: Vec3) -> bool {
        let delta = b - a;
        let length = delta.length();
        if length < 0.01 {
            return self.supported(a);
        }
        // Raised decks must be approached through ramps, never vertical jumps.
        if delta.y.abs() > delta.with_y(0.).length() * 0.85 + 0.05 {
            return false;
        }
        let side = delta.cross(Vec3::Y).normalize_or_zero() * 0.57;
        if [-side, Vec3::ZERO, side]
            .into_iter()
            .any(|offset| !self.line_clear(a + offset + Vec3::Y * 0.8, b + offset + Vec3::Y * 0.8))
        {
            return false;
        }
        let samples = (length / 0.35).ceil() as u32;
        let mut previous = None;
        for i in 0..=samples {
            let Some(height) = self.support_height(a.lerp(b, i as f32 / samples as f32)) else {
                return false;
            };
            // A line above a ledge is not a ramp. Every sampled support change
            // must be a step the controller could actually take.
            if previous.is_some_and(|old: f32| (height - old).abs() > 0.42) {
                return false;
            }
            previous = Some(height);
        }
        true
    }

    /// Test hook: force the graph rebuild a retraction would cause.
    #[cfg(test)]
    pub fn rebuild_navigation_for_profiling(&mut self) {
        self.rebuild_navigation();
    }

    fn rebuild_navigation(&mut self) {
        self.edges = vec![Vec::new(); self.nav.len()];
        for a in 0..self.nav.len() {
            for b in a + 1..self.nav.len() {
                if (self.nav[a].y - self.nav[b].y).abs() <= 0.42
                    && self.nav[a].distance(self.nav[b]) < 8.0
                    && self.walkable(self.nav[a], self.nav[b])
                {
                    self.edges[a].push(b);
                    self.edges[b].push(a);
                }
            }
        }
    }

    /// Whether the navigation graph still connects two points. Used by tests to
    /// prove a retraction actually severed a crossing.
    #[must_use]
    pub fn navigable(&self, from: Vec3, to: Vec3) -> bool {
        let nearest = |point: Vec3| self.nearest_waypoint(point);
        let (Some(start), Some(goal)) = (nearest(from), nearest(to)) else {
            return false;
        };
        self.breadth_first(start)[goal] != usize::MAX
    }

    /// The waypoint a body should enter the graph at.
    ///
    /// Sorted by distance first and reachability-checked only for the closest
    /// few. Checking every waypoint in range was the single most expensive
    /// thing the simulation did.
    fn nearest_waypoint(&self, point: Vec3) -> Option<usize> {
        const CHECKED: usize = 4;
        let mut candidates: Vec<(f32, usize)> = self
            .nav
            .iter()
            .enumerate()
            .map(|(index, node)| (point.distance_squared(*node), index))
            .filter(|(distance, _)| *distance < 81.)
            .collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        candidates
            .into_iter()
            .take(CHECKED)
            .find(|(_, index)| self.walkable(point, self.nav[*index]))
            .map(|(_, index)| index)
    }

    fn breadth_first(&self, start: usize) -> Vec<usize> {
        let mut previous = vec![usize::MAX; self.nav.len()];
        previous[start] = start;
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(at) = queue.pop_front() {
            for &next in &self.edges[at] {
                if previous[next] == usize::MAX {
                    previous[next] = at;
                    queue.push_back(next);
                }
            }
        }
        previous
    }

    /// Which way a minor should walk this tick.
    ///
    /// `toward_goal` is the shared breadth-first solve rooted at the Observer's
    /// own waypoint, so `toward_goal[n]` is the next node on the way there.
    fn steer(&self, feet: Vec3, target: Vec3, toward_goal: Option<&[usize]>) -> Vec3 {
        // A clear walk straight at the Observer needs no graph at all.
        if (feet.y - target.y).abs() <= 0.42 && self.walkable(feet, target) {
            return (target - feet).with_y(0.).normalize_or_zero();
        }
        let (Some(parents), Some(start)) = (toward_goal, self.nearest_waypoint(feet)) else {
            return Vec3::ZERO;
        };
        if parents[start] == usize::MAX {
            return Vec3::ZERO;
        }
        // String-pull one hop: aim at the next node's successor when it is
        // directly reachable, so a minor makes continuous progress instead of
        // turning back toward whichever node it happens to be nearest.
        let next = parents[start];
        let after = parents[next];
        let waypoint = if after != usize::MAX
            && after != next
            && (self.nav[after].y - feet.y).abs() <= 0.42
            && self.walkable(feet, self.nav[after])
        {
            self.nav[after]
        } else {
            self.nav[next]
        };
        (waypoint - feet).with_y(0.).normalize_or_zero()
    }

    /// Replay fingerprint: the site, the rules, and every body's physical state.
    #[must_use]
    pub fn digest(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut add = |value: u64| {
            hash ^= value;
            hash = hash.wrapping_mul(0x100_0000_01b3);
        };
        for value in [
            1, // solved-floor kinetic contract version
            PLAYER_USER_DATA as u64,
            self.site.seed,
            self.site.cells.len() as u64,
            self.site.colliders().len() as u64,
            self.tick,
            self.mode as u64,
            self.outcome as u64,
            u64::from(self.powered),
            u64::from(self.wave),
            u64::from(self.wave_delay),
            u64::from(self.retract_warning.unwrap_or(u32::MAX)),
            u64::from(self.cell_present),
            u64::from(self.cooldown),
            u64::from(self.kills),
            u64::from(self.next_id),
            u64::from(self.config.cooldown),
            u64::from(self.config.stagger),
        ] {
            add(value);
        }
        for value in [
            self.charge,
            self.config.reach,
            self.config.push,
            self.config.pull,
            self.config.minor_speed,
            self.config.minor_accel,
            self.player.position.x,
            self.player.position.y,
            self.player.position.z,
            self.player.yaw,
            self.player.pitch,
        ] {
            add(u64::from(value.to_bits()));
        }
        for (id, actor) in &self.actors {
            add(u64::from(id.0));
            add(u64::from(actor.alive));
            add(u64::from(actor.stagger));
            add(actor.behavior as u64);
            add(u64::from(actor.grounded));
            let body = &self.physics.bodies[actor.body];
            for value in [
                body.translation().x,
                body.translation().y,
                body.translation().z,
                body.linvel().x,
                body.linvel().y,
                body.linvel().z,
                body.angvel().x,
                body.angvel().y,
                body.angvel().z,
            ] {
                add(u64::from(value.to_bits()));
            }
            for value in body.rotation().to_array() {
                add(u64::from(value.to_bits()));
            }
            add(u64::from(body.is_sleeping()));
        }
        hash
    }
}

/// Stable local Observer identity in collider user data.
const PLAYER_USER_DATA: u128 = 999;

/// Marks a collider as part of the projected architecture rather than an actor.
/// Set high so actor identities keep their small, stable values.
const STRUCTURAL_TAG: u128 = 1 << 100;

#[cfg(test)]
mod tests;
