//! The two things on the board that are not the player: the Guardian, and a
//! rival squad racing for the same exit.
//!
//! Both act in the resolve step of a turn, after the player has committed and
//! before the facility shifts. That ordering is deliberate — a rival that moved
//! *after* the relayout would appear to walk through walls that had just closed.

use observed_core::{PlayerId, TeamId};
use observed_facility::hex_wfc::HexWfcWorld;
use observed_facility::map_spec::RoomRole;
use observed_hex::{HexCoord, travel_distance};
use observed_match::hex_wfc::HexGuardianStatus;

use super::unit::Unit;
use super::vision::{exits, is_solid_space};

/// The Guardian, reduced to what a turn-based board needs: a cell, a status,
/// and a quarry.
///
/// [`HexGuardianStatus`] is the shipped enum rather than a local copy, so the
/// three states a player has to learn are the same three in both games.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Guardian {
    pub cell: HexCoord,
    pub status: HexGuardianStatus,
    pub target: Option<PlayerId>,
}

impl Guardian {
    /// Start it in its control room, or at the far corner from the spawn when the
    /// solve placed no such room.
    #[must_use]
    pub fn new(world: &HexWfcWorld) -> Self {
        let home = world
            .blueprints
            .iter()
            .find(|blueprint| blueprint.role == RoomRole::GuardianControl)
            .map(|blueprint| blueprint.anchor)
            .unwrap_or_else(|| world.config.exit());
        Self {
            cell: home,
            status: HexGuardianStatus::Active,
            target: None,
        }
    }

    /// Close `steps` cells on the nearest reachable unit.
    ///
    /// Observation stops it. That is the same contract the shipped Guardian has
    /// and it is the reason the mechanic is not simply a timer: the thing that
    /// protects your route from the facility also protects you from the hunter,
    /// so a turn's observation budget is spent against both at once.
    pub fn step(
        &mut self,
        world: &HexWfcWorld,
        units: &[Unit],
        steps: u8,
        observed: &std::collections::BTreeSet<HexCoord>,
        anchored: &std::collections::BTreeSet<HexCoord>,
    ) {
        self.status = if anchored.contains(&self.cell) {
            HexGuardianStatus::FrozenByAnchor
        } else if observed.contains(&self.cell) {
            HexGuardianStatus::FrozenByPlayer
        } else {
            HexGuardianStatus::Active
        };
        if self.status != HexGuardianStatus::Active || steps == 0 {
            return;
        }
        let Some(quarry) = nearest_unit(world, self.cell, units) else {
            self.target = None;
            return;
        };
        self.target = Some(quarry.id);
        for _ in 0..steps {
            if self.cell == quarry.cell {
                break;
            }
            let Some(next) = advance_toward(world, self.cell, quarry.cell) else {
                break;
            };
            self.cell = next;
        }
    }
}

/// The unit with the cheapest route from `from`, ignoring escaped units.
#[must_use]
pub fn nearest_unit(world: &HexWfcWorld, from: HexCoord, units: &[Unit]) -> Option<Unit> {
    units
        .iter()
        .filter(|unit| !unit.escaped)
        .min_by_key(|unit| {
            world
                .route_between_cells(from, unit.cell)
                .map_or(u32::MAX, |route| route.cost_millis)
        })
        .copied()
}

/// One cell along the route from `from` to `goal`, or the neighbour that gets
/// closest when no route exists.
///
/// The fallback matters more than it looks: mid-match the facility rewires, and
/// a hunter or a rival that simply stopped when its route broke would read as
/// broken itself. This is the same `Seek`-else-`Explore` split the shipped
/// objective bot makes (`model/bot.rs`), at one move per turn instead of per
/// tick.
#[must_use]
pub fn advance_toward(world: &HexWfcWorld, from: HexCoord, goal: HexCoord) -> Option<HexCoord> {
    if let Some(route) = world.route_between(from, goal)
        && let Some(&next) = route.get(1)
    {
        return Some(next);
    }
    exits(world, from)
        .into_iter()
        .map(|(_, cell)| cell)
        .filter(|&cell| is_solid_space(world, cell))
        .min_by_key(|&cell| travel_distance(cell, goal))
        .filter(|&cell| travel_distance(cell, goal) < travel_distance(from, goal))
}

/// Where a rival unit wants to be next, given what its squad still owes.
///
/// Deliberately one pure function returning one cell, mirroring the Guardian
/// rather than introducing a planner: the rival exists to apply race pressure so
/// the player's route choices have a clock on them, and a smarter opponent would
/// change what is being measured.
#[must_use]
pub fn rival_goal(world: &HexWfcWorld, unit: &Unit, authorized: bool) -> HexCoord {
    if authorized {
        return world.config.exit();
    }
    world
        .blueprints
        .iter()
        .filter(|blueprint| blueprint.role == RoomRole::Keystone)
        .map(|blueprint| blueprint.anchor)
        .min_by_key(|&cell| travel_distance(unit.cell, cell))
        .unwrap_or_else(|| world.config.exit())
}

/// The rival squad's team identity, for readers that would otherwise reach for
/// a literal.
#[must_use]
pub const fn rival_team() -> TeamId {
    super::unit::RIVAL_TEAM
}
