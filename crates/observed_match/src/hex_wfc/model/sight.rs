//! What a body actually sees: a fan of rays from its eye against the match's own
//! colliders, and the built cells those rays pass through or come to rest in.
//!
//! One answer to "what can this body see", for everything that asks. The Ascent rules ward
//! and know by it (`ascent::facility`), and the in-play map records it, so a team's two
//! maps agree. Walls, floors, ceilings and closed doors are colliders, so they stop sight
//! as they stop bodies; open air is seen through, so a body looking across an atrium or
//! down a stairwell sees the floors it opens onto.
//!
//! Fixed-tick and pure: the fan is laid out from the body's yaw and pitch, the rays are
//! cast against the stable colliders every peer built, and the result is refreshed on a
//! fixed cadence, so every peer sees the same cells on the same tick. It is derived from
//! positions and geometry, so snapshots leave it out and a late joiner replaying from tick
//! one rebuilds it.

use std::collections::BTreeMap;

use glam::Vec3;
use observed_core::PlayerId;
use observed_hex::{HexCoord, TILE_LEVEL_HEIGHT};

use super::HexWfcMatch;
use super::movement::containing_cell;

/// How far sight reaches, metres: four cells, the rules' sight range.
pub const SIGHT_REACH: f32 = 56.0;
/// Ticks between refreshes of a body's sight. The bodies are spread across them by id, so
/// one tick never casts every body's fan.
pub const SIGHT_REFRESH_TICKS: u64 = 6;
/// Half the fan's width either side of the look, radians: about the view's own.
const HALF_WIDTH: f32 = 0.78;
/// Rays across the fan.
const ACROSS: u16 = 11;
/// Pitches of the fan's rows, radians, about the look's.
const ROWS: [f32; 5] = [-0.45, -0.22, 0.0, 0.22, 0.45];
/// Metres between the points sampled along a ray.
const STEP: f32 = 1.5;
/// How far short of what stops a ray its last sample is taken, so the cell it is looking
/// into counts and the one behind the wall does not.
const SHY: f32 = 0.1;

/// The built cells a body sees, each with the nearest distance it was seen at, metres from
/// the eye. The body's own cell is always in it, at no distance.
pub type HexSight = BTreeMap<HexCoord, f32>;

impl HexWfcMatch {
    /// What `player`'s body sees, as of the last refresh; `None` for a body not walking
    /// the facility.
    #[must_use]
    pub fn sight(&self, player: PlayerId) -> Option<&HexSight> {
        self.sight.get(&player)
    }

    /// Every body's sight, as of the last refresh.
    #[must_use]
    pub fn sights(&self) -> &BTreeMap<PlayerId, HexSight> {
        &self.sight
    }

    /// Refresh each body's sight on its turn of the cadence, and any body that has none yet;
    /// drop the sight of a body that has left the facility.
    pub(super) fn refresh_sight(&mut self) {
        let tick = self.tick;
        let due =
            |player: PlayerId| (tick + u64::from(player.0)).is_multiple_of(SIGHT_REFRESH_TICKS);
        let walking: Vec<PlayerId> = self
            .players
            .values()
            .filter(|player| player.in_facility())
            .map(|player| player.id)
            .collect();
        self.sight.retain(|player, _| walking.contains(player));
        for player in walking {
            if !due(player) && self.sight.contains_key(&player) {
                continue;
            }
            let state = &self.players[&player];
            let (Some((eye, _)), cell) = (self.eye_and_look(player), state.cell) else {
                continue;
            };
            let sight = self.sample_sight(eye, state.yaw, state.pitch, cell);
            self.sight.insert(player, sight);
        }
    }

    /// The fan from `eye`, looking along `yaw` and `pitch`, of a body standing in `own`.
    #[must_use]
    pub fn sample_sight(&self, eye: Vec3, yaw: f32, pitch: f32, own: HexCoord) -> HexSight {
        let mut sight = HexSight::from([(own, 0.0)]);
        let top = f32::from(self.facility.config.levels.saturating_sub(1));
        for column in 0..ACROSS {
            let across = f32::from(column) / f32::from(ACROSS - 1) * 2.0 - 1.0;
            let yaw = yaw + across * HALF_WIDTH;
            let forward = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
            for row in ROWS {
                let pitch = (pitch + row).clamp(-1.5, 1.5);
                let direction = forward * pitch.cos() + Vec3::Y * pitch.sin();
                let stop = self
                    .physics
                    .ray_distance(eye, direction, SIGHT_REACH)
                    .unwrap_or(SIGHT_REACH);
                let last = (stop - SHY).max(0.0);
                let mut at = 0.0_f32;
                loop {
                    at = (at + STEP).min(last);
                    let point = eye + direction * at;
                    let level = (point.y / TILE_LEVEL_HEIGHT).floor().clamp(0.0, top) as u8;
                    if let Some(cell) = containing_cell(self.facility.config, point, level)
                        && self
                            .facility
                            .placements
                            .get(&cell)
                            .is_some_and(|placement| placement.space.built())
                    {
                        let nearest = sight.entry(cell).or_insert(at);
                        *nearest = nearest.min(at);
                    }
                    if at >= last {
                        break;
                    }
                }
            }
        }
        sight
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use observed_facility::hex_wfc::HexWfcConfig;
    use observed_hex::{HexFace, hex_origin};

    use super::*;
    use crate::hex_wfc::model::{HEX_INPUT_VERSION, HexInputFrame, HexMatchConfig};

    const BODY: PlayerId = PlayerId(0);

    fn game(seed: u64) -> HexWfcMatch {
        let config = HexMatchConfig {
            teams: 1,
            members_per_team: 1,
            guardian: false,
            wfc: HexWfcConfig {
                levels: 2,
                ..HexWfcConfig::default()
            },
        };
        let mut game = HexWfcMatch::new_with_content(
            seed,
            config,
            crate::hex_wfc::compatibility_test_content().clone(),
        )
        .expect("a two-level facility solves");
        game.send_catches_to_prison();
        game
    }

    /// Stand still until the next refresh of `BODY`, player 0, has run.
    fn until_refreshed(game: &mut HexWfcMatch) {
        loop {
            let frame = HexInputFrame {
                version: HEX_INPUT_VERSION,
                tick: game.tick + 1,
                commands: BTreeMap::new(),
            };
            game.step(&frame);
            if game.tick.is_multiple_of(SIGHT_REFRESH_TICKS) {
                return;
            }
        }
    }

    fn sight(game: &HexWfcMatch) -> HexSight {
        game.sight(BODY).expect("a body in the facility").clone()
    }

    #[test]
    fn a_body_sees_its_own_cell_and_only_built_cells_within_reach() {
        for seed in [3, 7, 11] {
            let game = game(seed);
            let seen = sight(&game);
            assert_eq!(
                seen.get(&game.players[&BODY].cell),
                Some(&0.0),
                "seed {seed}"
            );
            for (cell, &distance) in &seen {
                assert!(game.facility.placements[cell].space.built(), "seed {seed}");
                assert!((0.0..=SIGHT_REACH).contains(&distance), "seed {seed}");
            }
        }
    }

    /// Sight follows the look: turn round, and a cell seen well ahead is no longer seen.
    #[test]
    fn turning_away_loses_what_was_ahead() {
        let mut game = game(7);
        let own = game.players[&BODY].cell;
        let (ahead, _) = sight(&game)
            .into_iter()
            .filter(|&(cell, distance)| cell != own && distance > 6.0)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("something seen across the room");
        let centre = Vec3::from_array(hex_origin(ahead));
        let (eye, _) = game.eye_and_look(BODY).expect("a body");
        game.aim_body_for_tests(BODY, eye - (centre - eye).with_y(0.0));
        until_refreshed(&mut game);
        assert!(
            !sight(&game).contains_key(&ahead),
            "{ahead:?} seen behind the body"
        );
    }

    /// Colliders stop sight: somewhere near the spawn, closing a door hides the cell
    /// through it. If sight ignored colliders, no door could ever hide anything.
    #[test]
    fn a_closed_door_hides_the_cell_behind_it() {
        let game = game(7);
        let grid = game.facility.config.grid();
        let doorways: Vec<(HexCoord, HexFace, HexCoord)> = game
            .facility
            .placements
            .iter()
            .filter(|(cell, placement)| cell.level == 0 && placement.space.built())
            .flat_map(|(&cell, placement)| {
                HexFace::LATERAL
                    .into_iter()
                    .filter(move |&face| placement.is_open(face))
                    .filter_map(move |face| {
                        grid.neighbor(cell, face).map(|next| (cell, face, next))
                    })
            })
            .filter(|&(_, face, next)| {
                game.facility
                    .placements
                    .get(&next)
                    .is_some_and(|other| other.space.built() && other.is_open(face.opposite()))
            })
            .collect();
        let hidden = doorways.into_iter().take(40).any(|(cell, face, next)| {
            let Some(feet) = game.standing_point(cell) else {
                return false;
            };
            let mut open = game.clone();
            open.stand_body_for_tests(BODY, cell, feet);
            let (eye, _) = open.eye_and_look(BODY).expect("a body");
            let target = Vec3::from_array(hex_origin(next)).with_y(eye.y);
            open.aim_body_for_tests(BODY, target);
            let mut closed = open.clone();
            closed.set_doors([((cell, face), true)]);
            until_refreshed(&mut open);
            until_refreshed(&mut closed);
            sight(&open).contains_key(&next) && !sight(&closed).contains_key(&next)
        });
        assert!(hidden, "no closed door near the spawn hid anything");
    }

    /// Sight is refreshed on its cadence and is gone while the body is jailed.
    #[test]
    fn sight_refreshes_on_its_cadence_and_a_jailed_body_has_none() {
        let mut game = game(7);
        until_refreshed(&mut game);
        let before = sight(&game);
        let (eye, look) = game.eye_and_look(BODY).expect("a body");
        game.aim_body_for_tests(BODY, eye - look * 5.0);
        let frame = HexInputFrame {
            version: HEX_INPUT_VERSION,
            tick: game.tick + 1,
            commands: BTreeMap::new(),
        };
        game.step(&frame);
        assert_eq!(sight(&game), before, "refreshed between its ticks");
        game.jail(BODY);
        until_refreshed(&mut game);
        assert!(
            game.sight(BODY).is_none(),
            "a jailed body sees the facility"
        );
    }

    /// Two peers stepping the same turns see the same cells on the same ticks.
    #[test]
    fn every_peer_sees_the_same() {
        let mut a = game(11);
        let mut b = a.clone();
        for tick in 0..240u32 {
            let commands = BTreeMap::from([(
                BODY,
                crate::hex_wfc::model::HexPlayerCommand {
                    intent: player_input::PlayerIntent {
                        look: glam::Vec2::new(if tick % 90 < 45 { 4.0 } else { -2.0 }, 0.0),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )]);
            for game in [&mut a, &mut b] {
                let frame = HexInputFrame {
                    version: HEX_INPUT_VERSION,
                    tick: game.tick + 1,
                    commands: commands.clone(),
                };
                game.step(&frame);
            }
            assert_eq!(a.sights(), b.sights(), "tick {tick}");
        }
    }

    /// The Guardian is frozen only by what is in plain view. Over the neighbouring cells
    /// round the spawn, a body facing the Guardian from within range freezes it where some
    /// line from its eye reaches the Guardian's body, and not where a wall stands across
    /// every one: there must be at least one of each, and each must come out so.
    #[test]
    fn the_guardian_is_frozen_only_by_what_is_in_plain_view() {
        use crate::hex_wfc::model::HexGuardianStatus;

        let config = HexMatchConfig {
            teams: 1,
            members_per_team: 1,
            guardian: true,
            wfc: HexWfcConfig {
                levels: 2,
                ..HexWfcConfig::default()
            },
        };
        let base = HexWfcMatch::new_with_content(
            7,
            config,
            crate::hex_wfc::compatibility_test_content().clone(),
        )
        .expect("a two-level facility solves");
        let grid = base.facility.config.grid();
        let mut cells: Vec<HexCoord> = base
            .facility
            .placements
            .iter()
            .filter(|(cell, placement)| cell.level == 0 && placement.space.built())
            .map(|(&cell, _)| cell)
            .collect();
        cells.sort();
        let (mut blocked, mut clear) = (0, 0);
        'search: for from in cells {
            for face in HexFace::LATERAL {
                let Some(to) = grid.neighbor(from, face) else {
                    continue;
                };
                let open =
                    base.facility.placements[&from].is_open(face)
                        && base.facility.placements.get(&to).is_some_and(|other| {
                            other.space.built() && other.is_open(face.opposite())
                        });
                if !open {
                    continue;
                }
                let centre = Vec3::from_array(hex_origin(to)) + Vec3::Y * 0.9;
                for feet in base.standing_points(from) {
                    let mut game = base.clone();
                    game.stand_body_for_tests(BODY, from, feet);
                    game.aim_body_for_tests(BODY, centre);
                    let (eye, _) = game.eye_and_look(BODY).expect("a body");
                    if !(2.0..13.0).contains(&eye.distance(centre)) {
                        continue;
                    }
                    game.guardian.cell = to;
                    game.guardian.position = centre;
                    let seen = [-0.5, 0.3, 1.1]
                        .into_iter()
                        .any(|height| game.physics.line_is_clear(eye, centre + Vec3::Y * height));
                    if seen && clear >= 1 || !seen && blocked >= 1 {
                        continue;
                    }
                    let frame = HexInputFrame {
                        version: HEX_INPUT_VERSION,
                        tick: game.tick + 1,
                        commands: BTreeMap::new(),
                    };
                    game.step(&frame);
                    let frozen = game.guardian.status == HexGuardianStatus::FrozenByPlayer;
                    assert_eq!(frozen, seen, "{from:?} -> {to:?} from {feet}");
                    if seen {
                        clear += 1;
                    } else {
                        blocked += 1;
                    }
                    if clear >= 1 && blocked >= 1 {
                        break 'search;
                    }
                }
            }
        }
        assert!(
            clear >= 1 && blocked >= 1,
            "clear {clear}, blocked {blocked}"
        );
    }
}
