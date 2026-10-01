//! Floor power and recharge on the real facility.
//!
//! The rules own power and charge (`ascent::economy`): which floors have power, where
//! each floor's generator and deployed recharge stations are, and every Observer's pool. On a lab
//! board an Observer at a fixture is an Observer on its cell. A first-person cell is
//! fourteen metres across, so here each fixture stands at a point on its cell's floor,
//! and a body works it by standing within [`FIXTURE_REACH`] of that point.
//!
//! - Every floor's generator is sited once, before tick zero, on a cell a
//!   body can stand in: never a stair or ramp cell, never the prison, and the generator
//!   in a room wherever the floor has one a body can reach (design section 5: "every
//!   playable floor has exactly one generator room"). The generator is fixed
//!   structure. Architects deploy stations from their mixed hand onto standable
//!   built cells; a tile rewrite or retraction removes the equipment.
//! - A body within reach of its floor's generator that presses interact toggles the
//!   floor's power, through the Observer command a lab Observer uses, so the power policy
//!   and every refusal are the rules' own.
//! - A body within reach of a powered station draws [`RECHARGE_PER_BEAT`] each beat. A
//!   station on a dark floor supplies nothing.
//!
//! The lab's teleport pads are not sited: the first-person match has plates of its own.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;
use observed_core::PlayerId;
use observed_facility::hex_wfc::{HexSpace, HexWfcWorld};
use observed_hex::HexCoord;

use super::AscentRules;
use crate::ascent::economy::{EconomyState, MAX_CHARGE, RECHARGE_PER_BEAT};
use crate::ascent::sim::{
    ACTOR_BEAT_TICKS, Observer, ObserverAction, ObserverCommand, ObserverId, ObserverState,
    linked_vertically,
};
use crate::hex_wfc::{HexInputFrame, HexWfcMatch};

/// How far from a fixture's point a body may stand and still work it, metres across the
/// floor. A fixture stands at most four metres from its cell's centre and a cell's walls
/// are seven, so a body in reach is always in the fixture's cell.
pub const FIXTURE_REACH: f32 = 2.2;

/// How far above a fixture's floor a body's centre may be and still reach it: a body
/// standing on that floor, not one on the floor above.
const FIXTURE_HEIGHT: f32 = 2.5;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FixtureKind {
    /// The floor's generator: its power, switched in person.
    Generator,
    /// The floor's recharge station, which fills the kinetic tool while the floor has
    /// power.
    Station,
}

/// A generator or recharge station, where it stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fixture {
    pub kind: FixtureKind,
    pub cell: HexCoord,
    /// The point on the cell's floor it stands on.
    pub floor: Vec3,
}

/// A fixture a body is standing at, and what it would do for it now: what the prompt
/// says.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtFixture {
    /// Whether the floor has power, and whether the rules would let this body switch it.
    Generator { powered: bool, operable: bool },
    /// Whether the station supplies charge, and the body's charge.
    Station { powered: bool, charge: u32 },
}

/// Site every floor's generator in `world`, which `physical` builds. The lab
/// economy's pre-sited stations are removed here: first-person Architects must
/// play station cards. Returns the economy and generator fixtures.
pub(super) fn site(
    world: &HexWfcWorld,
    observers: &BTreeMap<ObserverId, Observer>,
    prison_core: &BTreeSet<HexCoord>,
    physical: &HexWfcMatch,
) -> (EconomyState, Vec<Fixture>) {
    // A physics query each, so asked lazily and remembered.
    let points: RefCell<BTreeMap<HexCoord, Option<Vec3>>> = RefCell::default();
    let point = |cell: HexCoord| {
        *points
            .borrow_mut()
            .entry(cell)
            .or_insert_with(|| physical.standing_point(cell))
    };
    let allowed = |cell: HexCoord| {
        world
            .placements
            .get(&cell)
            .is_some_and(|placement| placement.space.built())
            && !prison_core.contains(&cell)
            && !linked_vertically(world, cell)
            && point(cell).is_some()
    };
    let room = |cell: HexCoord| {
        world
            .placements
            .get(&cell)
            .is_some_and(|placement| placement.space == HexSpace::Room)
    };
    let mut economy = EconomyState::sited(world, observers, prison_core, allowed, room);
    economy.pads.clear();
    economy.stations.clear();
    let fixtures = economy
        .generators
        .values()
        .filter_map(|&cell| {
            Some(Fixture {
                kind: FixtureKind::Generator,
                cell,
                floor: point(cell)?,
            })
        })
        .collect();
    (economy, fixtures)
}

/// Collider-verified places where a played station can stand. Kept outside the
/// economy until a card is actually played.
pub(super) fn station_points(
    physical: &HexWfcMatch,
    rules: &crate::ascent::sim::ArchitectLab,
) -> BTreeMap<HexCoord, Vec3> {
    rules
        .world
        .placements
        .iter()
        .filter_map(|(&cell, _)| station_point(physical, rules, cell).map(|point| (cell, point)))
        .collect()
}

fn station_point(
    physical: &HexWfcMatch,
    rules: &crate::ascent::sim::ArchitectLab,
    cell: HexCoord,
) -> Option<Vec3> {
    if !rules.world.placements.get(&cell)?.space.built()
        || rules.prison_core.contains(&cell)
        || rules.economy.is_at_generator(cell)
        || rules.linked_vertically(cell)
    {
        return None;
    }
    physical.standing_point(cell)
}

impl AscentRules {
    pub(super) fn refresh_station_sites(
        &mut self,
        physical: &HexWfcMatch,
        changed: impl IntoIterator<Item = HexCoord>,
    ) {
        for cell in changed {
            self.station_points.remove(&cell);
            self.session.sim.station_sites.remove(&cell);
            if let Some(point) = station_point(physical, &self.session.sim, cell) {
                self.station_points.insert(cell, point);
                self.session.sim.station_sites.insert(cell);
            }
        }
    }

    pub(super) fn refresh_station_fixtures(&mut self) {
        let stations = &self.session.sim.economy.stations;
        self.fixtures.retain(|fixture| {
            fixture.kind == FixtureKind::Generator || stations.contains(&fixture.cell)
        });
        for &cell in stations {
            if self.fixtures.iter().any(|fixture| fixture.cell == cell) {
                continue;
            }
            let floor = self.station_points[&cell];
            self.fixtures.push(Fixture {
                kind: FixtureKind::Station,
                cell,
                floor,
            });
        }
        self.fixtures
            .sort_by_key(|fixture| (fixture.kind, fixture.cell));
    }
    /// Every floor's generator and recharge station, where each stands.
    #[must_use]
    pub fn fixtures(&self) -> &[Fixture] {
        &self.fixtures
    }

    /// The fixture `player`'s body stands within reach of, and what it would do now.
    #[must_use]
    pub fn at_fixture(
        &self,
        physical: &HexWfcMatch,
        player: PlayerId,
    ) -> Option<(Fixture, AtFixture)> {
        let id = self.observer_for(player)?;
        let fixture = *self.fixture_in_reach(physical, player)?;
        let rules = &self.session.sim;
        let powered = rules.economy.is_powered(fixture.cell.level);
        let at = match fixture.kind {
            FixtureKind::Generator => AtFixture::Generator {
                powered,
                operable: rules.observer_refusal(id, TOGGLE).is_none(),
            },
            FixtureKind::Station => AtFixture::Station {
                powered,
                charge: rules.economy.charge(id),
            },
        };
        Some((fixture, at))
    }

    /// Whether `player`'s body stands within reach of its floor's generator.
    pub(super) fn at_generator(&self, physical: &HexWfcMatch, player: PlayerId) -> bool {
        self.fixture_in_reach(physical, player)
            .is_some_and(|fixture| fixture.kind == FixtureKind::Generator)
    }

    fn fixture_in_reach(&self, physical: &HexWfcMatch, player: PlayerId) -> Option<&Fixture> {
        let body = physical.players.get(&player)?;
        if !body.in_facility() {
            return None;
        }
        self.fixtures.iter().find(|fixture| {
            let rise = body.position.y - fixture.floor.y;
            let across = (body.position - fixture.floor).with_y(0.0).length();
            (0.0..=FIXTURE_HEIGHT).contains(&rise) && across <= FIXTURE_REACH
        })
    }

    /// Set `player`'s charge. For evidence captures, as `HexWfcMatch::jail` is: play only
    /// spends charge on shots and restores it at stations.
    pub fn stage_charge(&mut self, player: PlayerId, charge: u32) {
        if let Some(&id) = self.bodies.get(&player) {
            self.session.sim.economy.set_charge(id, charge);
        }
    }

    /// Deploy a station on `level` at the site nearest `near`, as a station card played
    /// there would, and return it. For evidence captures: in play only an Architect's card
    /// deploys one. `None` when the floor has no site.
    pub fn stage_station(&mut self, level: u8, near: Vec3) -> Option<Fixture> {
        let (&cell, _) = self
            .station_points
            .iter()
            .filter(|(cell, _)| cell.level == level)
            .min_by(|(a, at), (b, bt)| {
                at.distance_squared(near)
                    .total_cmp(&bt.distance_squared(near))
                    .then(a.cmp(b))
            })?;
        self.session.sim.economy.stations.insert(cell);
        self.refresh_station_fixtures();
        self.fixtures
            .iter()
            .copied()
            .find(|fixture| fixture.kind == FixtureKind::Station && fixture.cell == cell)
    }

    /// Set a floor's power. For evidence captures: play switches it only at the generator,
    /// or by a play that contests it.
    pub fn stage_power(&mut self, level: u8, powered: bool) {
        self.session.sim.economy.set_powered(level, powered);
        self.session.sim.refresh_observation();
    }

    /// Every body that pressed interact within reach of its floor's generator switches
    /// the floor's power, as the rules allow.
    pub(super) fn operate_generators(&mut self, physical: &HexWfcMatch, bodies: &HexInputFrame) {
        for (player, command) in &bodies.commands {
            if !command.actions.interact {
                continue;
            }
            let Some(&id) = self.bodies.get(player) else {
                continue;
            };
            if self
                .fixture_in_reach(physical, *player)
                .is_some_and(|fixture| fixture.kind == FixtureKind::Generator)
            {
                // Refused under the power policy, or by a body that is not active: the
                // rules' answer, and nothing changes.
                let _ = self.session.sim.submit_observer(id, TOGGLE);
            }
        }
    }

    /// On a beat, every active body within reach of a powered station draws its charge.
    pub(super) fn recharge_at_stations(&mut self, physical: &HexWfcMatch) {
        let rules = &self.session.sim;
        if !rules.tick.is_multiple_of(u64::from(ACTOR_BEAT_TICKS)) {
            return;
        }
        let drawing: Vec<_> = self
            .bodies
            .iter()
            .filter(|&(&player, id)| {
                rules
                    .observers
                    .get(id)
                    .is_some_and(|observer| observer.state == ObserverState::Active)
                    && rules.economy.charge(*id) < MAX_CHARGE
                    && self
                        .fixture_in_reach(physical, player)
                        .is_some_and(|fixture| {
                            fixture.kind == FixtureKind::Station
                                && rules.economy.is_at_powered_station(fixture.cell)
                        })
            })
            .map(|(_, &id)| id)
            .collect();
        for id in drawing {
            self.session
                .sim
                .economy
                .recharge_observer(id, RECHARGE_PER_BEAT);
        }
    }
}

const TOGGLE: ObserverCommand = ObserverCommand {
    facing: None,
    action: ObserverAction::ToggleGenerator,
};
