//! Authoritative fixed-tick kinetic simulation. No camera, UI or Bevy entities.
use crate::{
    arena::{self, Solid},
    physics::{Physics, gv, rv},
};
use glam::{Quat, Vec3};
use observed_traversal::{
    FIXED_DT, FpsBody, FpsConfig, RapierKinematicSettings,
    rapier_controller::step_character_in_query,
};
use player_input::PlayerIntent;
use rapier3d::prelude::*;
use std::collections::BTreeMap;

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
    Eliminated(ActorId),
    Power(bool),
    Retracting,
    Retracted,
    Wave(u8),
    Ended(Outcome),
    Recharge,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub reach: f32,
    pub cost: f32,
    pub push: f32,
    pub pull: f32,
    pub cooldown: u32,
    pub stagger: u32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            reach: 8.,
            cost: 10.,
            push: 10.,
            pull: 7.,
            cooldown: 15,
            stagger: 27,
        }
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
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
}

/// An in-memory snapshot clones every durable Rapier structure as well as rules.
/// The same type is used for replay continuation and reset/independent-world tests.
#[derive(Clone)]
pub struct KineticWorld {
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
    pub bridge_warning: Option<u32>,
    pub bridge_present: bool,
    pub kills: u32,
    pub events: Vec<Event>,
    pub actors: BTreeMap<ActorId, Actor>,
    pub physics: Physics,
    pub solids: Vec<Solid>,
    pub player_handle: RigidBodyHandle,
    pub player_collider: ColliderHandle,
    structural: BTreeMap<u32, ColliderHandle>,
    next_id: u32,
    nav: Vec<Vec3>,
    edges: Vec<Vec<usize>>,
}
impl KineticWorld {
    pub fn new(mode: Mode) -> Self {
        let mut physics = Physics::default();
        let solids = arena::solids();
        let mut structural = BTreeMap::new();
        for s in &solids {
            let h = physics.colliders.insert(
                ColliderBuilder::cuboid(s.half.x, s.half.y, s.half.z)
                    .translation(rv(s.center))
                    .friction(0.65)
                    .user_data(s.id as u128)
                    .build(),
            );
            structural.insert(s.id, h);
        }
        let player_config = FpsConfig::deliberate_rapier();
        let mut player = FpsBody::spawned(arena::SPAWN + Vec3::Y * player_config.half_height, 0.);
        player.pitch = -0.17;
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
            .user_data(999)
            .build(),
            player_handle,
            &mut physics.bodies,
        );
        let mut world = Self {
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
            bridge_warning: None,
            bridge_present: true,
            kills: 0,
            events: Vec::new(),
            actors: BTreeMap::new(),
            physics,
            solids,
            player_handle,
            player_collider,
            structural,
            next_id: 1000,
            nav: arena::waypoints(),
            edges: Vec::new(),
        };
        if mode == Mode::Practice {
            for p in [
                Vec3::new(-11., 0.6, 1.),
                Vec3::new(-1., 3.6, -4.),
                Vec3::new(3., 0.6, 6.5),
            ] {
                world.spawn(Kind::Minor, p);
            }
        }
        world.spawn(Kind::Prop, Vec3::new(-7., 0.5, 3.));
        world.spawn(Kind::Prop, Vec3::new(-11., 0.5, -1.));
        world.physics.step();
        world.rebuild_navigation();
        world
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
                .user_data(id.0 as u128)
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
            },
        );
        id
    }
    pub fn pose(&self, id: ActorId) -> Pose {
        let body = &self.physics.bodies[self.actors[&id].body];
        Pose {
            position: gv(body.translation()),
            rotation: Quat::from_array(body.rotation().to_array()),
        }
    }
    pub fn velocity(&self, id: ActorId) -> Vec3 {
        gv(self.physics.bodies[self.actors[&id].body].linvel())
    }
    pub fn eye(&self) -> Vec3 {
        self.player.eye(&self.player_config)
    }
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
            .find(|a| a.collider == handle && a.alive)
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
                let a = self.actors.get_mut(&target.id).unwrap();
                a.stagger = self.config.stagger;
                let body = &mut self.physics.bodies[a.body];
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
    pub fn interaction(&self) -> Option<&'static str> {
        let feet = self.player.position - Vec3::Y * self.player_config.half_height;
        [
            (arena::GENERATOR, "Toggle generator"),
            (arena::PANEL, "Retract bridge"),
        ]
        .into_iter()
        .filter(|(p, _)| feet.distance(*p) < 2.5)
        .filter(|(p, _)| self.line_clear(self.eye(), *p + Vec3::Y * 1.3))
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
            Some("Retract bridge") if self.bridge_present && self.bridge_warning.is_none() => {
                self.bridge_warning = Some(120);
                self.events.push(Event::Retracting);
            }
            _ => {}
        }
    }
    pub fn step(&mut self, command: Command) {
        self.events.clear();
        if self.outcome != Outcome::Playing {
            return;
        }
        self.tick += 1;
        self.cooldown = self.cooldown.saturating_sub(1);
        // Hazards commit before movement so every query sees the same support.
        if let Some(left) = self.bridge_warning {
            if left <= 1 {
                self.bridge_warning = None;
                self.bridge_present = false;
                self.physics.colliders[self.structural[&arena::BRIDGE]].set_enabled(false);
                self.rebuild_navigation();
                self.events.push(Event::Retracted);
            } else {
                self.bridge_warning = Some(left - 1);
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
        if report.recovered || self.player.position.y < arena::VOID_Y {
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
                && feet.distance(arena::STATION) < 2.2
                && self.line_clear(self.eye(), arena::STATION + Vec3::Y * 1.3)
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
            let pos = self.pose(id).position;
            let velocity = self.velocity(id);
            if pos.y < arena::VOID_Y {
                let a = self.actors.get_mut(&id).unwrap();
                a.alive = false;
                self.physics.bodies[a.body].set_enabled(false);
                self.physics.colliders[a.collider].set_enabled(false);
                if a.kind == Kind::Minor {
                    self.kills += 1;
                    self.events.push(Event::Eliminated(id));
                }
                continue;
            }
            // A stair can support the front of a body while its centre hangs
            // over the previous tread. Use real contact normals, not a centre ray.
            let collider = self.actors[&id].collider;
            let grounded = self
                .physics
                .narrow
                .contact_pairs_with(collider)
                .any(|pair| {
                    let sign = if pair.collider1 == collider { -1. } else { 1. };
                    pair.manifolds
                        .iter()
                        .any(|m| !m.data.solver_contacts.is_empty() && m.data.normal.y * sign > 0.5)
                })
                && velocity.y.abs() < 1.5;
            let contact_visible = self.line_clear(pos, self.player.position);
            let a = self.actors.get_mut(&id).unwrap();
            a.grounded = grounded;
            if a.kind == Kind::Minor && velocity.with_y(0.).length() > 4. && a.stagger == 0 {
                a.stagger = self.config.stagger;
            }
            if self.mode == Mode::Encounter
                && a.kind == Kind::Minor
                && a.stagger == 0
                && grounded
                && contact_visible
                && pos.distance(self.player.position) < 1.15
            {
                a.behavior = Behavior::Capture;
                self.finish(Outcome::Captured);
                return;
            }
        }
    }
    fn advance_waves(&mut self) {
        if self
            .actors
            .values()
            .any(|a| a.alive && a.kind == Kind::Minor)
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
        let positions = [
            Vec3::new(-12., 0.6, -9.),
            Vec3::new(-11., 0.6, 10.),
            Vec3::new(-2., 3.6, -6.),
            Vec3::new(-12., 0.6, 2.),
            Vec3::new(-5., 0.6, -10.),
            Vec3::new(-1., 0.6, 10.),
        ];
        let count = self.wave as usize + 1;
        let mut choices: Vec<_> = positions
            .into_iter()
            .filter(|p| p.distance(self.player.position) > 4.)
            .collect();
        choices.sort_by(|a, b| {
            b.distance_squared(self.player.position)
                .total_cmp(&a.distance_squared(self.player.position))
        });
        for p in choices.into_iter().take(count) {
            self.spawn(Kind::Minor, p);
        }
        self.events.push(Event::Wave(self.wave));
    }
    /// Fixed priority behavior tree: practice -> stagger -> airborne -> pursuit.
    /// Every leaf emits a desired velocity; physics remains the movement authority.
    fn think(&mut self) {
        let ids: Vec<_> = self.actors.keys().copied().collect();
        for id in ids {
            if !self.actors[&id].alive || self.actors[&id].kind != Kind::Minor {
                continue;
            }
            let a = &self.actors[&id];
            let behavior = if a.stagger > 0 {
                Behavior::Staggered
            } else if !a.grounded {
                Behavior::Airborne
            } else if self.mode == Mode::Practice {
                Behavior::Practice
            } else {
                Behavior::Pursue
            };
            let desired = if behavior == Behavior::Pursue {
                self.route_direction(self.pose(id).position - Vec3::Y * 0.55)
            } else {
                Vec3::ZERO
            };
            let stepped = if behavior == Behavior::Pursue {
                Some(self.physics.walk_minor(a.body, desired * 2.5))
            } else {
                None
            };
            let a = self.actors.get_mut(&id).unwrap();
            a.behavior = behavior;
            a.stagger = a.stagger.saturating_sub(1);
            if matches!(behavior, Behavior::Pursue | Behavior::Practice) {
                let body = &mut self.physics.bodies[a.body];
                let old = body.linvel();
                let horizontal =
                    Vec3::new(old.x, 0., old.z).move_towards(desired * 2.5, 15. * FIXED_DT);
                let velocity = stepped.unwrap_or(horizontal);
                // Autostep is a collision-checked positional correction. Turning
                // its rise into velocity would launch a body above the staircase.
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
    pub fn line_clear(&self, from: Vec3, to: Vec3) -> bool {
        let offset = to - from;
        let distance = offset.length();
        if distance < 0.001 {
            return true;
        }
        let ray = Ray::new(rv(from), rv(offset / distance));
        !self.structural.values().any(|h| {
            let c = &self.physics.colliders[*h];
            c.is_enabled()
                && c.shape()
                    .cast_ray(c.position(), &ray, distance, true)
                    .is_some()
        })
    }
    fn support_height(&self, feet: Vec3) -> Option<f32> {
        let ray = Ray::new(rv(feet + Vec3::Y * 0.4), -Vector::Y);
        self.structural
            .values()
            .filter_map(|h| {
                let c = &self.physics.colliders[*h];
                if c.is_enabled() {
                    c.shape().cast_ray(c.position(), &ray, 1.2, true)
                } else {
                    None
                }
            })
            .min_by(f32::total_cmp)
            .map(|distance| feet.y + 0.4 - distance)
    }
    fn supported(&self, feet: Vec3) -> bool {
        self.support_height(feet).is_some()
    }
    fn walkable(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let length = d.length();
        if length < 0.01 {
            return self.supported(a);
        }
        // Raised paths must be approached through stairs, never by vertical jumps.
        if d.y.abs() > d.with_y(0.).length() * 0.85 + 0.05 {
            return false;
        }
        let side = d.cross(Vec3::Y).normalize_or_zero() * 0.57;
        if [-side, Vec3::ZERO, side]
            .into_iter()
            .any(|s| !self.line_clear(a + s + Vec3::Y * 0.8, b + s + Vec3::Y * 0.8))
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
            // must be a step the controller can actually climb or descend.
            if previous.is_some_and(|old: f32| (height - old).abs() > 0.42) {
                return false;
            }
            previous = Some(height);
        }
        true
    }
    fn rebuild_navigation(&mut self) {
        self.edges = vec![Vec::new(); self.nav.len()];
        for a in 0..self.nav.len() {
            for b in a + 1..self.nav.len() {
                if (self.nav[a].y - self.nav[b].y).abs() <= 0.42
                    && self.nav[a].distance(self.nav[b]) < 4.5
                    && self.walkable(self.nav[a], self.nav[b])
                {
                    self.edges[a].push(b);
                    self.edges[b].push(a);
                }
            }
        }
    }
    fn route_direction(&self, feet: Vec3) -> Vec3 {
        let target = self.player.position - Vec3::Y * self.player_config.half_height;
        if (feet.y - target.y).abs() <= 0.42 && self.walkable(feet, target) {
            return (target - feet).with_y(0.).normalize_or_zero();
        }
        let nearest = |p: Vec3| {
            self.nav
                .iter()
                .enumerate()
                .filter(|(_, n)| p.distance(**n) < 6. && self.walkable(p, **n))
                .min_by(|(_, a), (_, b)| {
                    p.distance_squared(**a).total_cmp(&p.distance_squared(**b))
                })
                .map(|(i, _)| i)
        };
        let (Some(start), Some(goal)) = (nearest(feet), nearest(target)) else {
            return Vec3::ZERO;
        };
        let mut previous = vec![usize::MAX; self.nav.len()];
        previous[start] = start;
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(at) = queue.pop_front() {
            if at == goal {
                break;
            }
            for &next in &self.edges[at] {
                if previous[next] == usize::MAX {
                    previous[next] = at;
                    queue.push_back(next);
                }
            }
        }
        if previous[goal] == usize::MAX {
            return Vec3::ZERO;
        }
        // String-pull the connected path to the furthest visible waypoint.
        // Requiring proximity to the nearest node every tick makes an actor turn
        // back halfway between nodes; visibility lets it make continuous progress.
        let mut path = vec![goal];
        let mut at = goal;
        while at != start {
            at = previous[at];
            path.push(at);
        }
        let waypoint = path
            .into_iter()
            .map(|i| self.nav[i])
            .find(|p| (p.y - feet.y).abs() <= 0.42 && self.walkable(feet, *p))
            .unwrap_or(self.nav[start]);
        (waypoint - feet).with_y(0.).normalize_or_zero()
    }
    /// Replay fingerprint includes configuration, static geometry, input-dependent
    /// rule state and every body's position, velocity, rotation and sleep state.
    pub fn digest(&self) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        let mut add = |n: u64| {
            h ^= n;
            h = h.wrapping_mul(0x100000001b3);
        };
        for n in [
            2,   // continuous kinetic simulation contract version
            999, // stable local Observer identity
            self.tick,
            self.mode as u64,
            self.outcome as u64,
            self.powered as u64,
            self.wave as u64,
            self.wave_delay as u64,
            self.bridge_warning.unwrap_or(u32::MAX) as u64,
            self.bridge_present as u64,
            self.cooldown as u64,
            self.kills as u64,
            self.next_id as u64,
            self.config.cooldown as u64,
            self.config.stagger as u64,
        ] {
            add(n);
        }
        for f in [
            self.charge,
            self.config.reach,
            self.config.cost,
            self.config.push,
            self.config.pull,
            self.player.yaw,
            self.player.pitch,
            self.player.jump_cd,
        ] {
            add(f.to_bits() as u64);
        }
        let c = self.player_config;
        for f in [
            c.walk_speed,
            c.run_speed,
            c.ground_accel,
            c.ground_decel,
            c.air_accel,
            c.gravity,
            c.max_fall,
            c.jump_speed,
            c.jump_cooldown,
            c.radius,
            c.half_height,
            c.eye_height,
            c.look_step,
            c.pitch_limit,
            c.substep,
            c.step_height,
            c.controller_offset,
            c.minimum_step_width,
            c.maximum_slope_degrees,
            c.ground_snap,
            self.player.spawn_yaw,
        ] {
            add(f.to_bits() as u64);
        }
        for v in [
            self.player.position,
            self.player.velocity,
            self.player.spawn,
        ] {
            for f in v.to_array() {
                add(f.to_bits() as u64);
            }
        }
        add(self.player.grounded as u64);
        for s in &self.solids {
            add(s.id as u64);
            for f in s.center.to_array().into_iter().chain(s.half.to_array()) {
                add(f.to_bits() as u64);
            }
        }
        for a in self.actors.values() {
            add(a.id.0 as u64);
            add(a.kind as u64);
            add(a.alive as u64);
            add(a.stagger as u64);
            add(a.grounded as u64);
            add(a.behavior as u64);
            let b = &self.physics.bodies[a.body];
            for f in b
                .translation()
                .to_array()
                .into_iter()
                .chain(b.rotation().to_array())
                .chain(b.linvel().to_array())
                .chain(b.angvel().to_array())
            {
                add(f.to_bits() as u64);
            }
            add(b.is_sleeping() as u64);
        }
        h
    }
}

#[cfg(test)]
mod tests;
