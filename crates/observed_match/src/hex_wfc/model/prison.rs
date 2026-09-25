//! The prison: a lobby in the facility, and a maze in another space.
//!
//! In a match that sends catches to prison, a Guardian's catch does not return a body to
//! safe ground. The body wakes in its team's prison maze, a sealed space with its own
//! lattice, geometry and colliders, and walks out of it into the prison lobby, a room on
//! the facility's ground floor. A teammate who reaches the lobby and holds it for
//! [`LOBBY_HOLD_TICKS`] brings every jailed member of the team out at once.
//!
//! One maze per team. It is carved fresh when a catch finds the team's prison empty, and
//! a teammate caught while it is occupied joins whoever is already inside. Guardians do
//! not enter the lobby (`guardian.rs`), so it cannot be camped.
//!
//! See `docs/architect_ascent_design.md`, section 3.

use std::collections::{BTreeMap, BTreeSet};

use glam::{Vec2, Vec3};
use observed_content::ArchitectureRegister;
use observed_core::{PlayerId, TeamId};
use observed_facility::hex_wfc::maze::braided_maze;
use observed_facility::hex_wfc::{HexCoord, HexWfcWorld};
use observed_hex::hex_origin;
use observed_traversal::FpsBody;
use observed_traversal::rapier_controller::{RapierTraversalScene, step_character_with_settings};
use player_input::PlayerIntent;

use super::super::geometry::HexWfcGeometrySnapshot;
use super::movement::horizontal_cell;
use super::{
    FIXED_DT, FLOOR_SLAB_TOP, HexBodyPlace, HexMatchEvent, HexMatchEventKind, HexWfcMatch,
    clear_spawn_position,
};

/// How long a teammate must stand in the lobby to break the team out: three seconds,
/// long enough to be seen and contested, short enough to be worth the walk.
pub const LOBBY_HOLD_TICKS: u16 = 180;
/// The maze's lattice. Its shortest way out is sixteen to twenty-four halls, about thirty
/// seconds walked straight, so a lone escape with its wrong turns takes a minute or so.
pub const MAZE_COLS: u16 = 9;
pub const MAZE_ROWS: u16 = 7;
/// Every prison hall, whatever district the facility above is in.
pub const MAZE_REGISTER: ArchitectureRegister = ArchitectureRegister::FacetMonument;

#[derive(Clone, Debug)]
pub struct HexPrison {
    /// The lobby room's cells.
    pub lobby: BTreeSet<HexCoord>,
    /// Where a released body appears.
    pub lobby_anchor: HexCoord,
    pub mazes: BTreeMap<TeamId, HexPrisonMaze>,
    /// How long a teammate has stood in the lobby while the team had someone jailed.
    pub lobby_hold: BTreeMap<TeamId, u16>,
}

/// One team's maze: a world of its own, drawn and collided like the facility.
#[derive(Clone, Debug)]
pub struct HexPrisonMaze {
    pub world: HexWfcWorld,
    pub geometry: HexWfcGeometrySnapshot,
    physics: RapierTraversalScene,
}

impl HexPrisonMaze {
    /// Where a caught body wakes.
    #[must_use]
    pub const fn entry(&self) -> HexCoord {
        self.world.config.spawn()
    }

    /// The hall that leads out to the lobby.
    #[must_use]
    pub const fn exit(&self) -> HexCoord {
        self.world.config.exit()
    }
}

impl HexPrison {
    /// The lobby is the ground-floor tile nearest the facility's centre that a body can
    /// walk to from the spawn; if that tile is part of a stamped room, the whole room.
    ///
    /// A tile rather than a room of its own, because the solver does not put a room at the
    /// centre of the ground floor: at production scale the ground floor holds the start
    /// room at the spawn corner and one other, so "the room nearest the centre" was a
    /// corner, and often the start room itself.
    fn new(world: &HexWfcWorld) -> Self {
        let centre = Vec3::from_array(hex_origin(HexCoord {
            q: world.config.cols / 2,
            r: world.config.rows / 2,
            level: 0,
        }));
        let from_centre = |cell: &HexCoord| {
            let at = Vec3::from_array(hex_origin(*cell));
            Vec2::new(at.x - centre.x, at.z - centre.z).length()
        };
        let spawn = world.config.spawn();
        let mut ground: Vec<HexCoord> = world
            .placements
            .iter()
            .filter(|(cell, placement)| cell.level == 0 && placement.space.built())
            .map(|(&cell, _)| cell)
            .collect();
        ground.sort_by(|a, b| {
            from_centre(a)
                .partial_cmp(&from_centre(b))
                .expect("distances are finite")
                .then(a.cmp(b))
        });
        let tile = ground
            .into_iter()
            .find(|&cell| world.route_between(spawn, cell).is_some())
            .unwrap_or(spawn);
        let room = world
            .blueprints
            .iter()
            .find(|blueprint| blueprint.cells.contains(&tile));
        let lobby = room.map_or_else(
            || BTreeSet::from([tile]),
            |room| room.cells.iter().copied().collect(),
        );
        Self {
            lobby,
            lobby_anchor: tile,
            mazes: BTreeMap::new(),
            lobby_hold: BTreeMap::new(),
        }
    }
}

impl HexWfcMatch {
    /// From here a Guardian's catch sends a body to its team's prison maze, and a fall
    /// through the whole facility loses it. Must be called before tick zero.
    pub fn send_catches_to_prison(&mut self) {
        assert_eq!(self.tick, 0, "a match has a prison from tick zero or never");
        self.prison = Some(HexPrison::new(&self.facility));
    }

    /// Which space a player's body is in.
    #[must_use]
    pub fn body_place(&self, player: PlayerId) -> Option<HexBodyPlace> {
        self.players.get(&player).map(|state| state.place)
    }

    /// Put a caught body in its team's maze, carving a fresh one if nobody is inside.
    pub(crate) fn jail(&mut self, id: PlayerId) {
        let team = self.players[&id].team;
        let inside = self
            .players
            .values()
            .filter(|player| player.team == team && player.place == HexBodyPlace::Prison)
            .count();
        let fresh = inside == 0;
        let seed = self.seed ^ u64::from(team.0).wrapping_mul(0xA24B_AED4_963E_E407) ^ self.tick;
        let content = self.content.clone();
        let prison = self.prison.as_mut().expect("only a prison match jails");
        if fresh {
            let world = braided_maze(seed, MAZE_COLS, MAZE_ROWS, MAZE_REGISTER);
            let geometry = HexWfcGeometrySnapshot::project_with_rooms(
                &world,
                content.cells(),
                content.rooms(),
            )
            .expect("every prison hall is one the corpus is required to build");
            let physics = geometry.rapier_scene();
            prison.mazes.insert(
                team,
                HexPrisonMaze {
                    world,
                    geometry,
                    physics,
                },
            );
        }
        let maze = &prison.mazes[&team];
        let entry = maze.entry();
        let position = spawn_in(
            &maze.physics,
            entry,
            &content.traversal_config(),
            u8::try_from(inside).unwrap_or(u8::MAX),
        );
        self.place_body(id, HexBodyPlace::Prison, entry, position);
        self.push_prison_event(HexMatchEventKind::PlayerJailed, id, entry);
    }

    /// Bring a jailed body out into the lobby.
    fn release(&mut self, id: PlayerId, kind: HexMatchEventKind) {
        let prison = self.prison.as_ref().expect("only a prison match releases");
        let anchor = prison.lobby_anchor;
        let crowd = self
            .players
            .values()
            .filter(|player| player.in_facility() && prison.lobby.contains(&player.cell))
            .count();
        let position = spawn_in(
            &self.physics,
            anchor,
            &self.content.traversal_config(),
            u8::try_from(crowd).unwrap_or(u8::MAX),
        );
        self.place_body(id, HexBodyPlace::Facility, anchor, position);
        self.push_prison_event(kind, id, anchor);
    }

    /// A body fell through the whole facility. It stays where it is, out of play.
    pub(super) fn lose(&mut self, id: PlayerId) {
        let cell = self.players[&id].cell;
        self.players.get_mut(&id).expect("player").place = HexBodyPlace::Void;
        self.push_prison_event(HexMatchEventKind::PlayerLost, id, cell);
    }

    /// Walk a jailed body through its maze, against the maze's own colliders.
    pub(super) fn move_jailed(&mut self, id: PlayerId, intent: PlayerIntent) {
        let team = self.players[&id].team;
        let profile = self.content.traversal_profile();
        let config = profile.controller();
        let prison = self.prison.as_ref().expect("a jailed body has a prison");
        let maze = &prison.mazes[&team];
        let body = self.bodies.get_mut(&id).expect("body");
        step_character_with_settings(
            &maze.physics,
            body,
            intent,
            &config,
            profile.rapier(),
            FIXED_DT,
        );
        let body = *body;
        // A maze is sealed, but a body that somehow leaves it wakes at the entry again.
        if !body.position.is_finite() || body.position.y < -4.0 {
            let entry = maze.entry();
            let position = spawn_in(&maze.physics, entry, &self.content.traversal_config(), 0);
            self.place_body(id, HexBodyPlace::Prison, entry, position);
            return;
        }
        let cell = horizontal_cell(maze.world.config, body.position, 0)
            .filter(|cell| maze.world.placements[cell].space.built());
        let player = self.players.get_mut(&id).expect("player");
        if let Some(cell) = cell {
            player.cell = cell;
        }
        player.position = body.position;
        player.yaw = body.yaw;
        player.pitch = body.pitch;
    }

    /// Catches become jailings; the maze's way out and the lobby hold become releases.
    pub(super) fn step_prison(&mut self) {
        if self.prison.is_none() {
            return;
        }
        let caught: Vec<PlayerId> = self
            .recent_events
            .iter()
            .filter(|event| event.kind == HexMatchEventKind::GuardianCatch)
            .filter_map(|event| event.player)
            .collect();
        for id in caught {
            self.jail(id);
        }

        let prison = self.prison.as_ref().expect("checked above");
        let out: Vec<PlayerId> = self
            .players
            .values()
            .filter(|player| {
                player.place == HexBodyPlace::Prison
                    && player.cell == prison.mazes[&player.team].exit()
            })
            .map(|player| player.id)
            .collect();
        for id in out {
            self.release(id, HexMatchEventKind::PlayerReleased);
        }

        for team in self.teams.keys().copied().collect::<Vec<_>>() {
            let prison = self.prison.as_ref().expect("checked above");
            let jailed: Vec<PlayerId> = self
                .players
                .values()
                .filter(|player| player.team == team && player.place == HexBodyPlace::Prison)
                .map(|player| player.id)
                .collect();
            let held = self.players.values().any(|player| {
                player.team == team && player.in_facility() && prison.lobby.contains(&player.cell)
            });
            let prison = self.prison.as_mut().expect("checked above");
            let hold = prison.lobby_hold.entry(team).or_default();
            if !held || jailed.is_empty() {
                *hold = 0;
                continue;
            }
            *hold += 1;
            if *hold < LOBBY_HOLD_TICKS {
                continue;
            }
            *hold = 0;
            for id in jailed {
                self.release(id, HexMatchEventKind::Jailbreak);
            }
        }
    }

    fn place_body(&mut self, id: PlayerId, place: HexBodyPlace, cell: HexCoord, position: Vec3) {
        let player = self.players.get_mut(&id).expect("player");
        player.place = place;
        player.cell = cell;
        player.position = position;
        let body = FpsBody::spawned(position, player.yaw);
        self.bodies.insert(id, body);
        self.stranded_ticks.insert(id, 0);
        self.stuck_ticks.insert(id, 0);
        self.progress_anchor.insert(id, position);
    }

    fn push_prison_event(&mut self, kind: HexMatchEventKind, player: PlayerId, cell: HexCoord) {
        self.recent_events.push(HexMatchEvent {
            tick: self.tick,
            kind,
            player: Some(player),
            cell: Some(cell),
        });
    }
}

/// A clear standing place in `cell`, spread by `index` so bodies do not stack.
fn spawn_in(
    physics: &RapierTraversalScene,
    cell: HexCoord,
    config: &observed_traversal::FpsConfig,
    index: u8,
) -> Vec3 {
    let origin = Vec3::from_array(hex_origin(cell));
    clear_spawn_position(physics, origin, config, index)
        .unwrap_or(origin + Vec3::Y * (FLOOR_SLAB_TOP + config.half_height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex_wfc::HexMatchContent;

    fn production_content() -> std::sync::Arc<HexMatchContent> {
        std::sync::Arc::new(HexMatchContent::from_runtime_catalog(
            crate::hex_wfc::test_catalog().clone(),
        ))
    }

    /// The compatibility corpus the other tests use is not what ships. Every maze must
    /// build from the committed catalogue, in the prison's own register.
    #[test]
    fn every_maze_builds_from_the_committed_catalogue() {
        let content = production_content();
        for seed in 0..24 {
            let world = braided_maze(seed, MAZE_COLS, MAZE_ROWS, MAZE_REGISTER);
            HexWfcGeometrySnapshot::project_with_rooms(&world, content.cells(), content.rooms())
                .unwrap_or_else(|error| panic!("seed {seed}: {error:?}"));
        }
    }

    /// The small test facilities have rooms everywhere; a production one does not. At
    /// production scale the lobby still sits at the ground floor's centre, and a body can
    /// walk to it from the spawn.
    #[test]
    fn at_production_scale_the_lobby_is_the_ground_floors_centre() {
        let content = production_content();
        for seed in [1u64, 11] {
            let config = crate::hex_wfc::HexMatchConfig {
                wfc: observed_facility::hex_wfc::HexWfcConfig::arc_default(),
                ..crate::hex_wfc::HexMatchConfig::default()
            };
            let mut game = HexWfcMatch::new_with_content(seed, config, content.clone())
                .expect("the production facility solves");
            game.send_catches_to_prison();
            let lobby = game.prison.as_ref().expect("just made").lobby_anchor;
            let centre = HexCoord {
                q: game.facility.config.cols / 2,
                r: game.facility.config.rows / 2,
                level: 0,
            };
            assert_eq!(lobby.level, 0, "seed {seed}");
            assert!(
                observed_hex::travel_distance(lobby, centre) <= 2,
                "seed {seed}: the lobby {lobby:?} is far from the centre"
            );
            assert!(
                game.facility
                    .route_between(game.facility.config.spawn(), lobby)
                    .is_some(),
                "seed {seed}: nobody can walk to the lobby"
            );
        }
    }
}
