//! What a squad must do before the exit will take it.
//!
//! The shipped match keys these off authored *tile sockets* — a keystone sits at
//! a measured point inside a room's geometry. This variant has no tile geometry,
//! so it keys off **room role** instead: standing anywhere in a `Keystone` room
//! and interacting takes the keystone. That is the simplification the lab is
//! for, and it is honest about what it drops — sub-room positioning — while
//! keeping the thing being tuned, which is how many objectives a route must
//! detour through.

use std::collections::{BTreeMap, BTreeSet};

use observed_core::TeamId;
use observed_facility::hex_wfc::HexWfcWorld;
use observed_facility::map_spec::RoomRole;
use observed_hex::HexCoord;

/// Hops a monitor room surveys around itself. Matches the shipped
/// `try_survey_monitor`, which calls `survey_local(.., 2)`.
pub const MONITOR_SURVEY_HOPS: usize = 2;

/// Per-squad objective progress.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TeamObjectives {
    pub keystones: u8,
    /// Consecutive turns this squad has held a station with two units.
    pub station_turns: u8,
    pub station_complete: bool,
    /// Rooms this squad has already surveyed, so a monitor pays once.
    pub surveyed: BTreeSet<u64>,
}

/// Which rooms still have something in them, tracked by stable generation key so
/// a relayout that moves a room does not resurrect a collected keystone.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObjectiveState {
    pub available_keystones: BTreeSet<u64>,
    pub teams: BTreeMap<TeamId, TeamObjectives>,
}

impl ObjectiveState {
    #[must_use]
    pub fn new(world: &HexWfcWorld, teams: impl IntoIterator<Item = TeamId>) -> Self {
        Self {
            available_keystones: rooms_with_role(world, RoomRole::Keystone)
                .map(|blueprint| blueprint.0)
                .collect(),
            teams: teams
                .into_iter()
                .map(|team| (team, TeamObjectives::default()))
                .collect(),
        }
    }

    #[must_use]
    pub fn team(&self, team: TeamId) -> &TeamObjectives {
        self.teams.get(&team).unwrap_or(&EMPTY)
    }

    pub(super) fn team_mut(&mut self, team: TeamId) -> &mut TeamObjectives {
        self.teams.entry(team).or_default()
    }
}

static EMPTY: TeamObjectives = TeamObjectives {
    keystones: 0,
    station_turns: 0,
    station_complete: false,
    surveyed: BTreeSet::new(),
};

/// `(generation_key, role, cells)` for every stamped room with `role`.
pub fn rooms_with_role(
    world: &HexWfcWorld,
    role: RoomRole,
) -> impl Iterator<Item = (u64, RoomRole, &[HexCoord])> {
    world
        .blueprints
        .iter()
        .filter(move |blueprint| blueprint.role == role)
        .map(|blueprint| {
            (
                blueprint.generation_key(),
                blueprint.role,
                blueprint.cells.as_slice(),
            )
        })
}

/// The room containing `cell`, if any.
#[must_use]
pub fn room_at(world: &HexWfcWorld, cell: HexCoord) -> Option<(u64, RoomRole, HexCoord)> {
    world
        .blueprints
        .iter()
        .find(|blueprint| blueprint.cells.contains(&cell))
        .map(|blueprint| (blueprint.generation_key(), blueprint.role, blueprint.anchor))
}

/// Cells holding objectives this squad has not finished with.
///
/// Fed to the relayout as `objective_cells`, where the facility guarantees a
/// surviving route to each. Without it the solver is free to wall off the last
/// keystone and make a match unwinnable — legally, since it only ever promises
/// a route to the *exit*.
#[must_use]
pub fn outstanding_cells(
    world: &HexWfcWorld,
    state: &ObjectiveState,
    team: TeamId,
    keystones_required: u8,
    stations_enabled: bool,
) -> BTreeSet<HexCoord> {
    let mut cells = BTreeSet::new();
    let progress = state.team(team);
    if progress.keystones < keystones_required {
        for (key, _, room_cells) in rooms_with_role(world, RoomRole::Keystone) {
            if state.available_keystones.contains(&key) {
                cells.extend(room_cells.iter().copied());
            }
        }
    }
    if stations_enabled && !progress.station_complete {
        for (_, _, room_cells) in rooms_with_role(world, RoomRole::DualStation) {
            cells.extend(room_cells.iter().copied());
        }
    }
    cells
}
