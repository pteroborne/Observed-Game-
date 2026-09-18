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
use observed_core::PlayerId;
use observed_facility::hex_wfc::{
    HexInfluenceField, HexObservationFrame, HexRelayoutCandidate, HexRelayoutProgress, HexSpace,
    HexWfcError, HexWfcWorld,
};
use observed_hex::{HexCoord, HexFace, hex_origin};
use observed_match::hex_wfc::HexWfcGeometrySnapshot;
use observed_traversal::{
    FIXED_DT, FpsBody, FpsConfig,
    gravity::ObserverGravity,
};
use player_input::PlayerIntent;
use rapier3d::prelude::*;
use std::collections::BTreeSet;

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
    /// Impart the armed plumb to whatever the crosshair has.
    Plumb,
    /// Point the armed plumb along the current look direction.
    Arm,
    SelfPlumb,
    Release,
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
    Reorienting,
    Clearance,
}

/// A redirected gravity riding on one body.
pub use plumb_lab::model::Plumb;

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
    /// A pocket has been selected and telegraphed. It can still be saved.
    Decohering(HexCoord),
    /// The pocket re-collapsed. The cell is where it happened.
    Relaid(HexCoord),
    /// The commit was refused because the Observer was watching or standing in
    /// the pocket. This is a result, not a failure.
    Held,
    /// Nothing reachable from here would re-collapse into anything else. Walk
    /// somewhere else and try again.
    Inert,
    /// A tile was retracted toward void, leaving the doorways into it opening
    /// onto nothing.
    Retracted(HexCoord),
    Wave(u8),
    Ended(Outcome),
    Recharge,
    /// A plumb landed on a body, and which way it now falls.
    Plumbed(ActorId, Vec3),
    /// A plumb wore off.
    Unplumbed(ActorId),
    /// The armed direction changed.
    Armed(Vec3),
    SelfPlumbed,
    GravityReleased,
    GravityWarning,
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
    /// What a plumb costs from the pool. More than a shove: it does not move a
    /// body once, it owns which way the body falls for four seconds.
    pub plumb_cost: f32,
    /// The acceleration a plumb imparts, in m/s^2.
    pub plumb_strength: f32,
    /// How long a plumb lasts, in ticks.
    pub plumb_ticks: u32,
    /// Ticks before the plumb can be used again.
    pub plumb_cooldown: u32,
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
            plumb_cost: 25.,
            plumb_strength: 20.,
            plumb_ticks: 240,
            plumb_cooldown: 45,
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
    /// Under a plumb. Physics owns the body: this lab's pursuit is a Y-up
    /// navigation graph, and a minor whose down points at a wall has no
    /// business being steered by it.
    Plumbed,
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
    /// The redirected gravity riding on this body, if any.
    pub lash: Option<Plumb>,
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
    pub gravity: ObserverGravity,
    pub charge: f32,
    pub powered: bool,
    pub cooldown: u32,
    /// Ticks before the plumb is ready again.
    pub plumb_cooldown: u32,
    /// Which way the armed plumb currently points.
    pub armed: Vec3,
    pub wave: u8,
    pub wave_delay: u32,
    /// The live facility. A relayout mutates this rather than the site, which
    /// stays the floor as it was dealt.
    pub world: HexWfcWorld,
    pub snapshot: HexWfcGeometrySnapshot,
    /// Bumped whenever geometry changes, so presentation knows to rebuild.
    pub geometry_generation: u32,
    /// A telegraphed pocket, its warning, and the candidate that will be
    /// offered to the solver when the warning expires.
    pub telegraph: Option<(u32, HexRelayoutCandidate)>,
    /// Cells retracted toward void.
    ///
    /// Retraction is a different verb from relayout and the lab needs both.
    /// A legal relayout can never open a door onto void — that is precisely the
    /// invariant the corpus enforces, and it holds after a rewrite exactly as
    /// it held before — so rewriting alone can never produce a hole a body can
    /// be put through. Canon already separates them: contradictions "retract
    /// implicated tiles toward void until a compatible play repairs the
    /// constraint". This is that, and it is deliberately not a legal state.
    pub retracted: BTreeSet<HexCoord>,
    pub kills: u32,
    pub events: Vec<Event>,
    pub actors: BTreeMap<ActorId, Actor>,
    pub physics: Physics,
    pub player_handle: RigidBodyHandle,
    pub player_collider: ColliderHandle,
    /// Projected structural colliders by stable ID.
    structural: BTreeMap<u32, ColliderHandle>,

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
        let (facility, snapshot) = (site.world.clone(), site.snapshot.clone());
        let mut world = Self {
            site,
            config: Config::default(),
            mode,
            outcome: Outcome::Playing,
            tick: 0,
            player,
            player_config,
            gravity: ObserverGravity::default(),
            charge: 100.,
            powered: true,
            cooldown: 0,
            plumb_cooldown: 0,
            // Armed straight up by default: the most obviously *not* gravity
            // direction, so the first shot reads as the tool doing something.
            armed: Vec3::Y,
            wave: 0,
            wave_delay: 300,
            world: facility,
            snapshot,
            geometry_generation: 0,
            telegraph: None,
            retracted: BTreeSet::new(),
            kills: 0,
            events: Vec::new(),
            actors: BTreeMap::new(),
            physics,
            player_handle,
            player_collider,
            structural,
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
                lash: None,
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
        self.gravity.frame.eye(&self.player, &self.player_config)
    }

    /// The body the crosshair currently selects, or why it selects nothing.
    pub fn target(&self) -> Result<Target, Refusal> {
        let origin = self.eye();
        let direction = self.look_dir();
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
        if self.gravity.transition > 0 { return Err(Refusal::Reorienting); }
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
                    self.look_dir()
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

    pub fn look_dir(&self) -> Vec3 { self.gravity.frame.look(&self.player) }
    pub fn self_plumb_ready(&self) -> Result<(), Refusal> {
        if self.gravity.transition > 0 { return Err(Refusal::Reorienting); }
        if self.plumb_cooldown > 0 { return Err(Refusal::Cooldown); }
        if self.mode == Mode::Encounter && self.charge < self.config.plumb_cost { return Err(Refusal::EmptyCharge); }
        let next = self.gravity.frame.toward(-self.armed);
        observed_traversal::gravity::reorient(&self.physics.query(self.player_handle), &self.player, self.gravity.frame, next, &self.player_config).ok_or(Refusal::Clearance)?;
        Ok(())
    }
    fn self_plumb(&mut self) {
        if let Err(reason) = self.self_plumb_ready() { self.events.push(Event::Refused(reason)); return; }
        if self.gravity.activate(&self.physics.query(self.player_handle), &mut self.player, &self.player_config, self.armed, 480) {
            if self.mode == Mode::Encounter { self.charge -= self.config.plumb_cost; }
            self.plumb_cooldown = self.config.plumb_cooldown;
            self.events.push(Event::SelfPlumbed);
        }
    }
    /// Cells intersected by the Observer and the surface bearing their weight.
    /// A ceiling or wall contact is just as occupied as the floor under a foot.
    pub fn protected_body_cells(&self) -> BTreeSet<HexCoord> {
        let mut cells = BTreeSet::new();
        let up = self.gravity.frame.up();
        for at in [self.player.position, self.player.position + up * self.player_config.half_height, self.player.position - up * (self.player_config.half_height + 0.15)] {
            if let Some(cell) = self.site.cell_containing(at) { cells.insert(cell); }
        }
        let capsule = Capsule::new_y(self.player_config.half_height - self.player_config.radius, self.player_config.radius + 0.16);
        let query = self.physics.query(self.player_handle);
        for (_, collider) in query.intersect_shape(self.gravity.frame.pose(self.player.position), &capsule) {
            if collider.user_data & STRUCTURAL_TAG == 0 { continue; }
            if let Some(piece) = self.snapshot.pieces.iter().find(|piece| u128::from(piece.id.0) == (collider.user_data & !STRUCTURAL_TAG)) { cells.insert(piece.source_cell); }
        }
        cells
    }

    /// Point the armed plumb where the Observer is looking.
    ///
    /// Arming is deliberately separate from firing. The tool's whole idea is
    /// that you decide which way down will be *before* you commit it to
    /// something, and folding the two together would make it a shove with extra
    /// steps.
    fn arm(&mut self) {
        if self.gravity.transition > 0 { self.events.push(Event::Refused(Refusal::Reorienting)); return; }
        let direction = self.look_dir().normalize_or(Vec3::Y);
        self.armed = direction;
        self.events.push(Event::Armed(direction));
    }

    /// Whether the plumb could be fired right now, and at what.
    pub fn plumb_ready(&self) -> Result<Target, Refusal> {
        if self.gravity.transition > 0 { return Err(Refusal::Reorienting); }
        let target = self.target()?;
        if self.plumb_cooldown > 0 {
            return Err(Refusal::Cooldown);
        }
        if self.mode == Mode::Encounter && self.charge < self.config.plumb_cost {
            return Err(Refusal::EmptyCharge);
        }
        Ok(target)
    }

    /// Commit the armed direction to whatever the crosshair has.
    fn fire_plumb(&mut self) {
        match self.plumb_ready() {
            Err(reason) => self.events.push(Event::Refused(reason)),
            Ok(target) => {
                let plumb = Plumb::new(
                    self.armed,
                    self.config.plumb_strength,
                    self.config.plumb_ticks,
                );
                self.attach(target.id, plumb);
                if self.mode == Mode::Encounter {
                    self.charge -= self.config.plumb_cost;
                }
                self.plumb_cooldown = self.config.plumb_cooldown;
                self.events.push(Event::Plumbed(target.id, plumb.direction));
            }
        }
    }

    /// Put a body under a redirected gravity.
    ///
    /// The body leaves world gravity entirely and a user force supplies the
    /// replacement. Rapier keeps that force until it is reset, so this is
    /// applied once rather than every tick.
    fn attach(&mut self, id: ActorId, plumb: Plumb) {
        let Some(actor) = self.actors.get(&id) else {
            return;
        };
        let handle = actor.body;
        let mass = self.physics.bodies[handle].mass();
        let body = &mut self.physics.bodies[handle];
        body.set_gravity_scale(0., true);
        body.reset_forces(true);
        body.add_force(rv(plumb.direction * plumb.strength * mass), true);
        // A plumbed body is allowed to tumble: locked upright while falling
        // sideways reads as a bug rather than as a body under strange gravity.
        body.set_enabled_rotations(true, true, true, true);
        if let Some(actor) = self.actors.get_mut(&id) {
            actor.lash = Some(plumb);
            actor.stagger = actor.stagger.max(self.config.stagger);
        }
    }

    /// Tick every live plumb down, and hand expired bodies back to the world.
    fn expire_plumbs(&mut self) {
        let expiring: Vec<ActorId> = self
            .actors
            .iter_mut()
            .filter_map(|(id, actor)| {
                let plumb = actor.lash.as_mut()?;
                if plumb.ticks <= 1 {
                    actor.lash = None;
                    Some(*id)
                } else {
                    plumb.ticks -= 1;
                    None
                }
            })
            .collect();
        for id in expiring {
            if let Some(actor) = self.actors.get(&id) {
                let handle = actor.body;
                let kind = actor.kind;
                let body = &mut self.physics.bodies[handle];
                body.reset_forces(true);
                body.set_gravity_scale(1., true);
                if kind == Kind::Minor {
                    // Minors stand upright again, because the pursuit they are
                    // about to resume is a Y-up one.
                    body.set_rotation(Rotation::IDENTITY, true);
                    body.set_angvel(Vector::ZERO, true);
                    body.set_enabled_rotations(false, false, false, true);
                }
            }
            self.events.push(Event::Unplumbed(id));
        }
    }

    /// The device within reach, if any.
    #[must_use]
    pub fn interaction(&self) -> Option<Device> {
        let feet = self.player.position - self.gravity.frame.up() * self.player_config.half_height;
        [
            (self.site.generator, Device::Generator),
            (self.site.panel, Device::Decoherence),
            (self.site.demolition, Device::Demolition),
        ]
        .into_iter()
        .filter(|(at, _)| feet.distance(*at) < 2.5)
        .filter(|(at, _)| self.line_clear(self.eye(), *at + Vec3::Y * 1.3))
        .min_by(|(a, _), (b, _)| {
            feet.distance_squared(*a)
                .total_cmp(&feet.distance_squared(*b))
        })
        .map(|(_, device)| device)
    }

    fn interact(&mut self) {
        match self.interaction() {
            Some(Device::Generator) => {
                self.powered = !self.powered;
                self.events.push(Event::Power(self.powered));
            }
            Some(Device::Decoherence) if self.telegraph.is_none() => self.telegraph(),
            Some(Device::Decoherence) => {}
            Some(Device::Demolition) => self.retract(),
            None => {}
        }
    }

    /// What the Observer can currently see, in the solver's own terms.
    ///
    /// This is the whole mechanic in one function. The solver already refuses
    /// to rewrite cells that are observed or occupied; first person is what
    /// makes that a *verb*, because what you can see is now a consequence of
    /// where you stand and which way you are facing.
    #[must_use]
    pub fn observation(&self) -> HexObservationFrame {
        let eye = self.eye();
        let look = self.look_dir();
        let mut visible = BTreeSet::new();
        for cell in self.world.placements.keys().copied() {
            if self.world.placements[&cell].space == HexSpace::Void {
                continue;
            }
            let centre = Vec3::from_array(hex_origin(cell)) + Vec3::Y * 1.2;
            let offset = centre - eye;
            let distance = offset.length();
            if distance > OBSERVATION_RANGE {
                continue;
            }
            // The cell you are standing in counts as seen however you face.
            let facing =
                distance < 1.0 || offset.normalize_or_zero().dot(look) > OBSERVATION_COSINE;
            if facing && self.line_clear(eye, centre) {
                visible.insert(cell);
            }
        }
        let mut occupied = BTreeMap::new();
        if let Some(cell) = self.site.cell_containing(self.player.position) {
            occupied.insert(PlayerId(0), cell);
        }
        HexObservationFrame {
            visible_cells: visible.union(&self.protected_body_cells()).copied().collect(),
            visible_thresholds: BTreeSet::new(),
            occupied_cells: occupied,
            // The devices are the floor's fixed points. An Architect may
            // rewrite the architecture around them; a lab that let a relayout
            // delete the station out from under its own recharge rule would be
            // testing a different thing.
            landmark_cells: [self.site.generator, self.site.station, self.site.panel]
                .into_iter()
                .filter_map(|at| self.site.cell_containing(at))
                .collect(),
            objective_cells: BTreeSet::new(),
        }
    }

    /// Select and telegraph a pocket. Nothing changes yet.
    ///
    /// Keeps looking until it finds a pocket that would actually differ from
    /// what is already there. Roughly half the pockets a small floor offers
    /// re-collapse to themselves — a run of void re-collapses faithfully into
    /// void, and a cell bounded by frozen neighbours often has one legal
    /// answer — and telegraphing one of those spends the warning, the sound and
    /// the player's attention on a floor that was never going to move.
    pub fn telegraph(&mut self) {
        let observation = self.observation();
        let influence = HexInfluenceField::encourage_decay();
        for frontier in self.frontiers(&observation) {
            let mut work = self.world.begin_frontier_relayout_sized(
                &observation,
                &frontier,
                crate::site::POCKET_TARGET_CELLS,
                crate::site::POCKET_MAX_CELLS,
            );
            for _ in 0..crate::site::POCKET_ATTEMPTS {
                match self.world.advance_driven_relayout(work, &influence) {
                    Ok(HexRelayoutProgress::Pending(next)) => work = next,
                    Ok(HexRelayoutProgress::Ready(candidate)) => {
                        if candidate.changed_cells.is_empty() {
                            break;
                        }
                        let Some(cell) = candidate.region.cells.iter().next().copied() else {
                            break;
                        };
                        self.telegraph = Some((DECOHERENCE_WARNING, candidate));
                        self.events.push(Event::Decohering(cell));
                        return;
                    }
                    Err(_) => break,
                }
            }
        }
        // Every frontier has been tried and none of them would move.
        self.events.push(Event::Inert);
    }

    /// Where to look for a pocket, nearest the Observer first.
    ///
    /// The frontier decides what the pocket is chosen *near*. Passing only the
    /// visible set selects nothing when the Observer faces a wall, and the
    /// pocket lands in the empty part of the lattice where nothing can change.
    fn frontiers(&self, observation: &HexObservationFrame) -> Vec<BTreeSet<HexCoord>> {
        let here = self.site.cell_containing(self.player.position);
        let mut anchored = observation.visible_cells.clone();
        anchored.extend(observation.occupied_cells.values().copied());
        anchored.extend(here);

        let mut built: Vec<HexCoord> = self
            .world
            .placements
            .iter()
            .filter(|(_, placement)| placement.space != HexSpace::Void)
            .map(|(cell, _)| *cell)
            .collect();
        let origin = here.unwrap_or(self.site.cells[0]);
        built.sort_by_key(|cell| {
            (
                observed_hex::lateral_distance(origin, *cell),
                cell.q,
                cell.r,
            )
        });

        let mut frontiers = Vec::new();
        if !anchored.is_empty() {
            frontiers.push(anchored);
        }
        frontiers.extend(built.into_iter().map(|cell| BTreeSet::from([cell])));
        frontiers
    }

    /// Offer the telegraphed pocket to the solver against the observation the
    /// Observer actually finished the warning with.
    ///
    /// The refusal is the point. `commit_relayout_delta` re-derives the
    /// protected set from this frame, so walking into the pocket or simply
    /// turning to look at it saves the floor.
    fn commit(&mut self) {
        let Some((_, candidate)) = self.telegraph.take() else {
            return;
        };
        let cell = candidate.region.cells.iter().next().copied();
        let latest = self.observation();
        match self.world.commit_relayout_delta(candidate, &latest) {
            Ok(delta) if delta.changed_cells.is_empty() => self.events.push(Event::Held),
            Ok(delta) => {
                let geometry = self.snapshot.project_delta_with_rooms(
                    &self.world,
                    &delta,
                    self.site.content.cells(),
                    self.site.content.rooms(),
                );
                match geometry {
                    Ok(geometry) => {
                        self.apply_geometry(&geometry);
                        if let Some(cell) = cell {
                            self.events.push(Event::Relaid(cell));
                        }
                    }
                    // The logical floor moved and its geometry did not, which
                    // would leave the two disagreeing. Nothing in this lab can
                    // repair that, so say so rather than play on.
                    Err(error) => panic!("projected relayout failed: {error:?}"),
                }
            }
            // Every refusal here is the facility being held, which is what the
            // Observer was trying to do. The variants differ in why, and none
            // of them is worth a different word on screen.
            Err(HexWfcError::UnsafeChange(_) | HexWfcError::StaleCandidate) => {
                self.events.push(Event::Held);
            }
            Err(_) => self.events.push(Event::Held),
        }
    }

    /// Swap the changed cells' geometry into the live collision world.
    fn apply_geometry(&mut self, geometry: &observed_match::hex_wfc::HexGeometryDelta) {
        for id in &geometry.colliders.removed {
            if let Some(handle) = self.structural.remove(&id.0) {
                self.physics.colliders.remove(
                    handle,
                    &mut self.physics.islands,
                    &mut self.physics.bodies,
                    false,
                );
            }
        }
        for spec in &geometry.colliders.upserted {
            // An upsert replaces a collider with the same stable ID.
            if let Some(handle) = self.structural.remove(&spec.id.0) {
                self.physics.colliders.remove(
                    handle,
                    &mut self.physics.islands,
                    &mut self.physics.bodies,
                    false,
                );
            }
            let Some(collider) = build_collider(spec) else {
                continue;
            };
            let mut collider = collider;
            collider.user_data = STRUCTURAL_TAG | u128::from(spec.id.0);
            self.structural
                .insert(spec.id.0, self.physics.colliders.insert(collider));
        }
        self.snapshot
            .apply_delta(geometry)
            .expect("the delta was projected from this snapshot");
        self.geometry_generation += 1;
        self.rebuild_navigation_near(&geometry.changed_cells);
    }

    /// Retract the tile the demolition control overlooks, toward void.
    ///
    /// Geometry only: the solver's world is left alone, because a cell with
    /// doors pointing into it is not a layout the solver would ever produce and
    /// feeding one back to it would corrupt every relayout after. The lab
    /// tracks the retraction beside the facility instead.
    pub(crate) fn retract(&mut self) {
        let cell = self.site.retracting;
        // Never the floor underfoot. The control stands beside its tile, but a
        // rule is cheaper than trusting that, and an Observer who deletes the
        // ground they are standing on has found a bug rather than a play.
        if self.protected_body_cells().contains(&cell) {
            self.events.push(Event::Inert);
            return;
        }
        if !self.retracted.insert(cell) {
            self.events.push(Event::Inert);
            return;
        }
        let doomed: Vec<u32> = self
            .snapshot
            .pieces
            .iter()
            .filter(|piece| piece.source_cell == cell)
            .map(|piece| piece.id.0)
            .collect();
        // Removed, not merely disabled. Disabling leaves the collider in the
        // broad phase until the next step, and the navigation rebuild happens
        // in this one — so the graph kept its edges across the hole, and the
        // Observer walked confidently into a cell that no longer had a floor.
        for id in doomed {
            if let Some(handle) = self.structural.remove(&id) {
                self.physics.colliders.remove(
                    handle,
                    &mut self.physics.islands,
                    &mut self.physics.bodies,
                    false,
                );
            }
        }
        self.geometry_generation += 1;
        self.rebuild_navigation_near(&BTreeSet::from([cell]));
        self.events.push(Event::Retracted(cell));
    }

    /// The cells the live floor has no floor in — the holes the tool needs.
    #[must_use]
    pub fn holes(&self) -> Vec<HexCoord> {
        (0..crate::site::config().cols)
            .flat_map(|q| (0..crate::site::config().rows).map(move |r| HexCoord { q, r, level: 0 }))
            .filter(|cell| {
                self.retracted.contains(cell)
                    || self
                        .world
                        .placements
                        .get(cell)
                        .is_none_or(|placement| placement.space == HexSpace::Void)
            })
            .collect()
    }

    /// Doorways that currently open onto a hole.
    #[must_use]
    pub fn open_thresholds(&self) -> Vec<(HexCoord, HexFace)> {
        let grid = crate::site::config().grid();
        let holes: BTreeSet<HexCoord> = self.holes().into_iter().collect();
        let mut found = Vec::new();
        for (cell, placement) in &self.world.placements {
            if placement.space == HexSpace::Void || self.retracted.contains(cell) {
                continue;
            }
            for face in HexFace::LATERAL {
                if placement.is_open(face)
                    && grid
                        .neighbor(*cell, face)
                        .is_some_and(|n| holes.contains(&n))
                {
                    found.push((*cell, face));
                }
            }
        }
        found
    }

    pub fn step(&mut self, command: Command) {
        self.events.clear();
        if self.outcome != Outcome::Playing {
            return;
        }
        self.tick += 1;
        self.cooldown = self.cooldown.saturating_sub(1);
        self.plumb_cooldown = self.plumb_cooldown.saturating_sub(1);
        self.expire_plumbs();
        // Hazards commit before movement so every query this tick sees the same
        // support the player is about to be resolved against.
        if let Some((left, _)) = &self.telegraph {
            if *left <= 1 {
                self.commit();
            } else if let Some((left, _)) = &mut self.telegraph {
                *left -= 1;
            }
        }
        let before = self.gravity.remaining;
        let report = self.gravity.step(
            &self.physics.query(self.player_handle), &mut self.player, command.movement,
            &self.player_config, (Vec3::ZERO, Vec3::splat(1000.)),
        );
        if before > 60 && self.gravity.remaining <= 60 { self.events.push(Event::GravityWarning); }
        if before > 0 && self.gravity.remaining == 0 { self.events.push(Event::GravityReleased); }
        self.physics.bodies[self.player_handle].set_next_kinematic_position(self.gravity.frame.pose(self.player.position));
        if report.recovered || self.player.position.y < VOID_Y {
            if self.mode == Mode::Practice {
                self.player.reset();
                self.gravity = ObserverGravity::default();
                self.physics.bodies[self.player_handle].set_position(self.gravity.frame.pose(self.player.position), true);
                self.physics.bodies[self.player_handle]
                    .set_translation(rv(self.player.position), true);
            } else {
                self.finish(Outcome::Fell);
                return;
            }
        }
        match command.action {
            Action::Push | Action::Pull => self.fire(command.action),
            Action::Plumb => self.fire_plumb(),
            Action::Arm => self.arm(),
            Action::SelfPlumb => self.self_plumb(),
            Action::Release => { self.gravity.release(); self.events.push(Event::GravityReleased); },
            Action::Interact => self.interact(),
            Action::None => {}
        }
        self.think();
        self.physics.step();
        self.resolve_contacts();
        if self.mode == Mode::Encounter {
            let feet = self.player.position - self.gravity.frame.up() * self.player_config.half_height;
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
            let up = self.actors[&id].lash.map_or(Vec3::Y, |plumb| plumb.up());
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
                            && gv(manifold.data.normal).dot(up) * sign > 0.5
                    })
                })
                && velocity.dot(up).abs() < 1.5;
            let contact_visible = self.line_clear(position, self.player.position);
            let over = self.site.cell_containing(position);
            let actor = self.actors.get_mut(&id).expect("live actor");
            actor.grounded = grounded;
            if grounded && actor.kind == Kind::Minor && actor.lash.is_some() {
                let rotation = Quat::from_rotation_arc(Vec3::Y, up);
                let body = &mut self.physics.bodies[actor.body];
                body.set_rotation(Rotation::from_xyzw(rotation.x,rotation.y,rotation.z,rotation.w), true);
                body.set_angvel(Vector::ZERO, true);
                body.set_enabled_rotations(false,false,false,true);
            }
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
                && actor.lash.is_none()
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
        // Muster near the fight, not at the far end of the floor.
        //
        // This used to take the cells *furthest* from the Observer, which on a
        // seven-cell arena meant twenty metres and on this floor means ninety:
        // a wave spent fifty seconds walking before anything happened, which is
        // dead air for a player and was fatal for the recorded director, whose
        // charge and patience both ran out first. Far enough not to appear on
        // top of somebody, near enough to be a wave.
        let feet = self.player.position - self.gravity.frame.up() * self.player_config.half_height;
        let ideal = (MUSTER_MIN + MUSTER_MAX) * 0.5;
        let mut choices: Vec<Vec3> = self
            .site
            .muster
            .iter()
            .copied()
            .filter(|at| {
                let distance = at.distance(feet);
                (MUSTER_MIN..=MUSTER_MAX).contains(&distance)
            })
            .collect();
        if choices.is_empty() {
            choices = self
                .site
                .muster
                .iter()
                .copied()
                .filter(|at| at.distance(feet) > MUSTER_MIN)
                .collect();
        }
        if choices.is_empty() {
            choices = self.site.muster.clone();
        }
        choices.sort_by(|a, b| {
            (a.distance(feet) - ideal)
                .abs()
                .total_cmp(&(b.distance(feet) - ideal).abs())
        });
        // A floor has fewer standable cells than a late wave has minors, so
        // muster points cycle rather than capping the wave.
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
        let target = self.player.position - self.gravity.frame.up() * self.player_config.half_height;
        let goal = self.nearest_waypoint(target);
        let toward_goal = goal.map(|node| self.breadth_first(node));

        let ids: Vec<_> = self.actors.keys().copied().collect();
        for id in ids {
            if !self.actors[&id].alive || self.actors[&id].kind != Kind::Minor {
                continue;
            }
            let actor = &self.actors[&id];
            let behavior = if actor.lash.is_some() {
                Behavior::Plumbed
            } else if actor.stagger > 0 {
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

    /// Whether a body could stand here *now*.
    ///
    /// Waypoints outlive the floor they were sampled on: retraction takes the
    /// geometry away and leaves the graph's nodes hanging in the air over the
    /// hole. Anything choosing somewhere to walk has to ask.
    #[must_use]
    pub fn standable(&self, feet: Vec3) -> bool {
        self.supported(feet)
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

    /// Recompute only the navigation a bounded change could have altered.
    ///
    /// A whole-graph rebuild costs about 95 ms on this floor, which is a
    /// visible hitch, and relayout makes it happen often. Walkability can only
    /// have changed near the cells that changed, so only those waypoints are
    /// re-linked.
    fn rebuild_navigation_near(&mut self, changed: &BTreeSet<HexCoord>) {
        let reach = observed_hex::ACROSS_CORNERS * 0.5 + NAV_LINK_RANGE;
        let centres: Vec<Vec3> = changed
            .iter()
            .map(|cell| Vec3::from_array(hex_origin(*cell)))
            .collect();
        let affected: Vec<usize> = (0..self.nav.len())
            .filter(|index| {
                let at = self.nav[*index];
                centres
                    .iter()
                    .any(|centre| centre.with_y(at.y).distance(at) < reach)
            })
            .collect();
        // Unlink them in both directions first, so a link that has stopped
        // being walkable actually disappears.
        for &index in &affected {
            for other in std::mem::take(&mut self.edges[index]) {
                self.edges[other].retain(|candidate| *candidate != index);
            }
        }
        for &index in &affected {
            for other in 0..self.nav.len() {
                if index == other || self.edges[index].contains(&other) {
                    continue;
                }
                if (self.nav[index].y - self.nav[other].y).abs() <= 0.42
                    && self.nav[index].distance(self.nav[other]) < NAV_LINK_RANGE
                    && self.walkable(self.nav[index], self.nav[other])
                {
                    self.edges[index].push(other);
                    self.edges[other].push(index);
                }
            }
        }
    }

    /// Test hook: retract without walking to the control.
    #[cfg(test)]
    pub fn retract_warning_for_tests(&mut self) {
        self.retract();
    }

    /// Test hook: attach a plumb directly.
    #[cfg(test)]
    pub fn attach_for_tests(&mut self, id: ActorId, plumb: Plumb) {
        self.attach(id, plumb);
    }

    /// Test hook: the bounded rebuild one relayout causes.
    #[cfg(test)]
    pub fn rebuild_navigation_for_profiling(&mut self, changed: &BTreeSet<HexCoord>) {
        self.rebuild_navigation_near(changed);
    }

    fn rebuild_navigation(&mut self) {
        self.edges = vec![Vec::new(); self.nav.len()];
        for a in 0..self.nav.len() {
            for b in a + 1..self.nav.len() {
                if (self.nav[a].y - self.nav[b].y).abs() <= 0.42
                    && self.nav[a].distance(self.nav[b]) < NAV_LINK_RANGE
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

    /// A horizontal walking direction from `from` toward `to`, routed through
    /// the navigation graph rather than straight through the architecture.
    ///
    /// Solves its own breadth-first search, so this is for one caller a tick —
    /// the recorded director — not for the minors, which share one solve.
    #[must_use]
    pub fn direction_toward(&self, from: Vec3, to: Vec3) -> Vec3 {
        let Some(goal) = self.nearest_waypoint(to) else {
            return (to - from).with_y(0.).normalize_or_zero();
        };
        let parents = self.breadth_first(goal);
        self.steer(from, to, Some(&parents))
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
            u64::from(self.telegraph.as_ref().map_or(u32::MAX, |(left, _)| *left)),
            u64::from(self.geometry_generation),
            self.retracted.len() as u64,
            self.world.generation as u64,
            u64::from(self.cooldown),
            u64::from(self.plumb_cooldown),
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
            self.config.plumb_strength,
            self.armed.x,
            self.armed.y,
            self.armed.z,
            self.player.position.x,
            self.player.position.y,
            self.player.position.z,
            self.player.yaw,
            self.player.pitch,
        ] {
            add(u64::from(value.to_bits()));
        }
        for value in self.gravity.frame.rotation.to_array().into_iter().chain(self.gravity.previous.rotation.to_array()).chain(self.player.velocity.to_array()) { add(u64::from(value.to_bits())); }
        for value in [self.gravity.remaining, self.gravity.transition, u32::from(self.gravity.returning)] { add(u64::from(value)); }
        for (id, actor) in &self.actors {
            add(u64::from(id.0));
            add(u64::from(actor.alive));
            add(u64::from(actor.stagger));
            add(actor.behavior as u64);
            add(u64::from(actor.grounded));
            add(actor.lash.map_or(0, |plumb| u64::from(plumb.ticks)));
            for value in actor
                .lash
                .map_or([0., 0., 0.], |plumb| plumb.direction.to_array())
            {
                add(u64::from(value.to_bits()));
            }
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

/// A thing an Observer can operate at the tile.
///
/// An enum rather than the label itself. These used to be matched as strings,
/// and renaming one prompt updated the side that advertises it and not the side
/// that acts on it: the control read "Retract the tile ahead", the handler
/// still waited for "Retract this tile", and pressing E did nothing at all for
/// as long as it took to find.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Generator,
    Decoherence,
    Demolition,
}

impl Device {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Generator => "Toggle generator",
            Self::Decoherence => "Decohere a pocket",
            Self::Demolition => "Retract the tile ahead",
        }
    }
}

/// How near an Observer a wave may muster, and how far.
const MUSTER_MIN: f32 = 12.0;
const MUSTER_MAX: f32 = 34.0;

/// How far apart two waypoints may be and still be linked.
const NAV_LINK_RANGE: f32 = 8.0;

/// How far an Observer's look protects a cell from being rewritten.
const OBSERVATION_RANGE: f32 = 26.0;
/// Roughly a 100-degree cone: `cos(50 degrees)`.
const OBSERVATION_COSINE: f32 = 0.64;
/// Two seconds of warning before a pocket re-collapses.
const DECOHERENCE_WARNING: u32 = 120;

/// Marks a collider as part of the projected architecture rather than an actor.
/// Set high so actor identities keep their small, stable values.
const STRUCTURAL_TAG: u128 = 1 << 100;

#[cfg(test)]
mod tests;
