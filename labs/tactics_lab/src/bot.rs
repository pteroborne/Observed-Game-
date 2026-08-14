//! Deterministic debug bot for the spectator controller.
//!
//! The bot emits one ordinary [`TacticsAction`] at a time. It owns no Bevy
//! state, camera, timer, or entity, so an automated run exercises exactly the
//! same simulation boundary as a player click and can be replayed from the
//! normal action log.

use observed_core::PlayerId;
use observed_facility::map_spec::RoomRole;
use observed_hex::HexCoord;

use crate::sim::action::TacticsAction;
use crate::sim::objectives::rooms_with_role;
use crate::sim::unit::PLAYER_TEAM;
use crate::sim::{MatchStatus, TacticsGame};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BotDirective {
    Act {
        unit: PlayerId,
        action: TacticsAction,
    },
    EndTurn,
    Finished,
}

/// Choose one visible simulation beat. Stable IDs, stable room keys, and route
/// cost supply every tie-break, so identical matches emit identical directives.
#[must_use]
pub fn choose(game: &TacticsGame) -> BotDirective {
    if game.status != MatchStatus::Running {
        return BotDirective::Finished;
    }
    for unit in game
        .units
        .values()
        .filter(|unit| unit.team == PLAYER_TEAM && !unit.escaped)
    {
        let legal = game.legal_actions(unit.id);
        if legal.contains(&TacticsAction::Interact) {
            return BotDirective::Act {
                unit: unit.id,
                action: TacticsAction::Interact,
            };
        }
        if game.telegraph.as_ref().is_some_and(|telegraph| {
            telegraph.cells().contains(&unit.cell) && legal.contains(&TacticsAction::DeployAnchor)
        }) {
            return BotDirective::Act {
                unit: unit.id,
                action: TacticsAction::DeployAnchor,
            };
        }
        let target = objective_target(game, unit.cell);
        if unit.cell == target {
            continue;
        }
        let Some(next) = crate::sim::adversary::advance_toward(&game.world, unit.cell, target)
        else {
            continue;
        };
        let Some(action) = legal.into_iter().find(|action| {
            let TacticsAction::Move(face) = action else {
                return false;
            };
            crate::sim::vision::step_through(&game.world, unit.cell, *face) == Some(next)
        }) else {
            continue;
        };
        return BotDirective::Act {
            unit: unit.id,
            action,
        };
    }
    BotDirective::EndTurn
}

#[must_use]
pub fn describe(directive: BotDirective) -> String {
    match directive {
        BotDirective::Act { unit, action } => format!("U{} {action:?}", unit.0),
        BotDirective::EndTurn => String::from("end turn"),
        BotDirective::Finished => String::from("match complete"),
    }
}

fn objective_target(game: &TacticsGame, from: HexCoord) -> HexCoord {
    let progress = game.objectives.team(PLAYER_TEAM);
    if progress.keystones < game.keystones_required()
        && let Some(target) = rooms_with_role(&game.world, RoomRole::Keystone)
            .filter(|(key, _, _)| game.objectives.available_keystones.contains(key))
            .flat_map(|(_, _, cells)| cells.iter().copied())
            .min_by_key(|&cell| route_key(game, from, cell))
    {
        return target;
    }
    if game.settings.objectives.stations() && !progress.station_complete {
        // Every squad member chooses the same stable station room. That turns
        // the two-operator requirement into real coordination instead of each
        // unit independently picking its nearest station.
        if let Some((_, _, cells)) =
            rooms_with_role(&game.world, RoomRole::DualStation).min_by_key(|(key, _, _)| *key)
            && let Some(&target) = cells.first()
        {
            return target;
        }
    }
    // A monitor is optional, not an objective. Interact is still taken when a
    // route happens to cross one, which makes spectator runs exercise surveys.
    game.world.config.exit()
}

fn route_key(game: &TacticsGame, from: HexCoord, to: HexCoord) -> (usize, HexCoord) {
    (
        game.world
            .route_between(from, to)
            .map_or(usize::MAX, |route| route.len()),
        to,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{GuardianSetting, MatchSettings, Objectives, ShiftCadence};
    use crate::sim::objectives::room_at;

    fn apply(game: &mut TacticsGame, directive: BotDirective) {
        match directive {
            BotDirective::Act { unit, action } => game.apply(unit, action).expect("bot action"),
            BotDirective::EndTurn => {
                game.end_turn();
            }
            BotDirective::Finished => {}
        }
    }

    #[test]
    fn identical_matches_emit_identical_directives_and_digests() {
        let mut a = TacticsGame::new(MatchSettings::standard()).expect("solves");
        let mut b = a.clone();
        for _ in 0..80 {
            let directive = choose(&a);
            assert_eq!(directive, choose(&b));
            apply(&mut a, directive);
            apply(&mut b, directive);
            assert_eq!(a.digest(), b.digest());
        }
    }

    #[test]
    fn the_spectator_bot_can_finish_an_objective_match() {
        let settings = MatchSettings {
            objectives: Objectives::Keystones,
            shift: ShiftCadence::Off,
            guardian: GuardianSetting::Off,
            rival_team: false,
            ..MatchSettings::standard()
        };
        let mut game = TacticsGame::new(settings).expect("solves");
        for _ in 0..2_000 {
            let directive = choose(&game);
            apply(&mut game, directive);
            if game.status == MatchStatus::Escaped {
                return;
            }
        }
        panic!(
            "spectator bot did not finish: turn={} status={:?} progress={:?} units={:?} next={:?}",
            game.turn,
            game.status,
            game.objectives.team(PLAYER_TEAM),
            game.units,
            choose(&game)
        );
    }

    #[test]
    fn a_unit_already_holding_the_station_waits_for_its_teammates() {
        let settings = MatchSettings {
            objectives: Objectives::Full,
            guardian: GuardianSetting::Off,
            rival_team: false,
            ..MatchSettings::collapse()
        };
        let mut game = TacticsGame::new(settings).expect("solves");
        game.objectives
            .teams
            .get_mut(&PLAYER_TEAM)
            .expect("team")
            .keystones = game.keystones_required();
        let station = rooms_with_role(&game.world, RoomRole::DualStation)
            .next()
            .and_then(|(_, _, cells)| cells.first().copied())
            .expect("station");
        let first = game
            .units
            .values()
            .find(|unit| unit.team == PLAYER_TEAM)
            .map(|unit| unit.id)
            .expect("unit");
        game.units.get_mut(&first).expect("unit").cell = station;
        let directive = choose(&game);
        assert!(
            !matches!(directive, BotDirective::Act { unit, .. } if unit == first),
            "the holder should not walk away while the station needs its squad"
        );
        assert_eq!(
            room_at(&game.world, station).map(|(_, role, _)| role),
            Some(RoomRole::DualStation)
        );
    }
}
