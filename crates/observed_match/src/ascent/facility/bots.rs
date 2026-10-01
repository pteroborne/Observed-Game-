//! First-person Observer bot priorities for the Ascent economy.
//!
//! The existing hex bot driver owns physical routing. This layer only chooses
//! when restoring a floor or refilling the kinetic tool takes precedence over
//! its usual objective, then emits the same body command a human could send.

use glam::Vec3;
use observed_core::PlayerId;

use super::{AscentRules, AtFixture, Fixture, FixtureKind};
use crate::ascent::economy::{KINETIC_SHOT_COST, MAX_CHARGE};
use crate::ascent::sim::{ObserverState, PowerPolicy};
use crate::hex_wfc::{HexBotDriver, HexPlayerCommand, HexWfcMatch};

impl AscentRules {
    /// Choose and drive a bot body through the same input frame as a player.
    /// A dark floor's reachable generator comes first. When charge cannot pay
    /// for a shot, seek a reachable powered station and remain there until full.
    /// Reachable means without leaving the floor: these errands are judged by the floor
    /// the body stands on, so one that climbed away would be dropped halfway. If no
    /// fixture can be reached, continue the normal physical objective.
    #[must_use]
    pub fn bot_body_command(
        &self,
        physical: &HexWfcMatch,
        driver: &mut HexBotDriver,
        player: PlayerId,
    ) -> HexPlayerCommand {
        let (Some(body), Some(id)) = (physical.players.get(&player), self.observer_for(player))
        else {
            return driver.command(physical, player);
        };
        if !body.in_facility() || self.session.sim.observers[&id].state != ObserverState::Active {
            return driver.command(physical, player);
        }
        let rules = &self.session.sim;
        let floor = body.cell.level;

        if rules.power_policy == PowerPolicy::Restorable
            && !rules.economy.is_powered(floor)
            && let Some(generator) = self.fixtures.iter().find(|fixture| {
                fixture.kind == FixtureKind::Generator
                    && fixture.cell.level == floor
                    && driver
                        .floor_route_len_to(physical, player, fixture.cell)
                        .is_some()
            })
        {
            if matches!(
                self.at_fixture(physical, player),
                Some((_, AtFixture::Generator { operable: true, .. }))
            ) {
                driver.clear_player(player);
                let mut command = HexPlayerCommand::default();
                command.actions.interact = true;
                return command;
            }
            return toward(physical, driver, player, body.position, generator);
        }

        if matches!(
            self.at_fixture(physical, player),
            Some((_, AtFixture::Station { powered: true, charge })) if charge < MAX_CHARGE
        ) {
            driver.clear_player(player);
            return HexPlayerCommand::default();
        }
        if rules.economy.charge(id) < KINETIC_SHOT_COST {
            let station = self
                .fixtures
                .iter()
                .filter(|fixture| {
                    fixture.kind == FixtureKind::Station
                        && fixture.cell.level == floor
                        && rules.economy.is_powered(floor)
                })
                .filter_map(|fixture| {
                    let length = driver.floor_route_len_to(physical, player, fixture.cell)?;
                    Some((length, fixture.cell, fixture))
                })
                .min_by_key(|(length, cell, _)| (*length, *cell));
            if let Some((_, _, station)) = station {
                return toward(physical, driver, player, body.position, station);
            }
        }

        driver.command(physical, player)
    }
}

fn toward(
    physical: &HexWfcMatch,
    driver: &mut HexBotDriver,
    player: PlayerId,
    position: Vec3,
    fixture: &Fixture,
) -> HexPlayerCommand {
    // Both fixtures are on the body's floor. Matching its current centre height
    // gives the route follower a reachable physical point inside fixture reach.
    driver.command_to(
        physical,
        player,
        fixture.cell,
        Vec3::new(fixture.floor.x, position.y, fixture.floor.z),
    )
}
