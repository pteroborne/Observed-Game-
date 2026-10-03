//! Evidence for the Architect's seat: `OBSERVED2_CAPTURE_HEX_WFC_ARCHITECT=<dir>`.
//!
//! Launches Architect Ascent with the local player in their team's Architect seat, lets
//! the team's bots walk for a while so there is something mapped, then:
//!
//! 1. the board as it is (`architect-board`);
//! 2. a card picked up and pointed at a cell the rules would take it on
//!    (`architect-play`): the legal targets, the ghost, the verdict;
//! 3. the same play confirmed through the desk, building in as the rules build it
//!    (`architect-building-in`), and
//! 4. built (`architect-built`);
//! 5. a teammate's request, once a body the bot drives asks for help (`architect-request`),
//! 6. and answered at the desk (`architect-answered`);
//! 7. off the desk, through a teammate's eyes (`architect-eyes`).
//!
//! The only staging is choosing the play and where it points; the play itself goes
//! through the desk, the rules and the physical facility like any other.
//!
//! `OBSERVED2_CAPTURE_HEX_WFC_ROGUE=<dir>` is the same capture from the Rogue board: the
//! local player walks as a body until their body is dropped into true void (staged:
//! `HexWfcMatch::drop_into_void`), and takes a seat at the Rogue board as a corrupted
//! player does. Then the board, a play, building in and built (`rogue-*`); nobody asks the
//! Rogue for help, and the Rogue looks through nobody's eyes. Instead, once its clock is
//! ready, it sends the major Guardians to a cell a short walk from one of them through the
//! desk as the answer key does (`rogue-directed`), and the board a while later, the
//! Guardian on its way (`rogue-directed-walked`). Last, once its clock is ready again, it
//! installs a sensor through the desk where one sees a loyal body (`rogue-sensor`): the
//! board shows every sensor - the bot Rogue's too - and the cells each watches.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_match::ascent::sim::{ArchitectCommand, CardId, CardKind};

use super::ArchitectDesk;
use super::board::Board;
use super::feedback::BuildIn;
use crate::hex_wfc::{HexWfcCapture, HexWfcCaptureMode, sim::HexWfcRuntime};

/// Ticks the bots walk before the first still: time to map a floor.
const MAPPING_TICKS: u64 = 1_200;
/// Ticks before the Rogue capture's body is dropped into the void.
const FALL_TICK: u64 = 150;
const GIVE_UP_FRAMES: u16 = 12_000;

#[allow(clippy::too_many_arguments)]
pub(in crate::hex_wfc) fn capture(
    mut commands: Commands,
    request: Option<ResMut<HexWfcCapture>>,
    runtime: Option<ResMut<HexWfcRuntime>>,
    desk: Option<ResMut<ArchitectDesk>>,
    board: Option<Res<Board>>,
    mut windows: Query<&mut Window>,
    building_in: Query<&BuildIn>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut request) = request else {
        return;
    };
    let rogue = request.mode == HexWfcCaptureMode::Rogue;
    if request.mode != HexWfcCaptureMode::Architect && !rogue {
        return;
    }
    if request.frame == 12
        && let Ok(mut window) = windows.single_mut()
    {
        window.resolution.set(1280.0, 800.0);
    }
    if request.frame > GIVE_UP_FRAMES {
        error!("architect capture gave up at stage {}", request.stills);
        exit.write(AppExit::error());
        return;
    }
    // The Rogue board is taken, not given: the body falls first.
    if rogue
        && desk.is_none()
        && let Some(mut runtime) = runtime
    {
        if runtime.match_state.tick >= FALL_TICK {
            let local = runtime.local_player;
            runtime.match_state.drop_into_void(local);
        }
        return;
    }
    let (Some(mut runtime), Some(mut desk), Some(_)) = (runtime, desk, board) else {
        return;
    };
    let prefix = if rogue { "rogue" } else { "architect" };
    let mapping = if rogue { FALL_TICK + 90 } else { MAPPING_TICKS };
    let tick = runtime.match_state.tick;
    if request.stills == 0 && tick >= mapping {
        if let Some(ascent) = runtime.ascent.as_mut() {
            let _ = ascent.stage_card(desk.seat, CardKind::Cistern);
        }
    }
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let path = std::path::PathBuf::from(&request.path);
    let shoot = |commands: &mut Commands, name: &str| {
        let name = format!("{prefix}-{name}-1280x800.png");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.join(name)));
    };
    match request.stills {
        0 if tick >= mapping => {
            shoot(&mut commands, "board");
            request.stills = 1;
        }
        1 => {
            let Some(knowledge) = desk.knowledge(ascent.rules()) else {
                return;
            };
            let Some(hand) = desk.hand(ascent.session()) else {
                return;
            };
            let mut cards: Vec<_> = hand.deck.hand.iter().enumerate().collect();
            // A cistern when one is in hand, so the still shows the multi-tile room; a stair else; a tile else.
            cards.sort_by_key(|(_, card)| match card.kind {
                CardKind::Cistern => 0,
                CardKind::Stair => 1,
                CardKind::Tile(_) => 2,
                CardKind::Door => 3,
                CardKind::Station => 3,
                // The Rogue's orders build nothing to show.
                CardKind::Directive | CardKind::Sensor | CardKind::Surge => 4,
            });
            let found = cards.into_iter().find_map(|(index, card)| {
                knowledge.cells.keys().find_map(|&target| {
                    (0..6).find_map(|rotation| {
                        let play = ArchitectCommand::Play {
                            card: card.id,
                            target,
                            rotation,
                        };
                        ascent
                            .session()
                            .architect_refusal(desk.seat, play)
                            .is_none()
                            .then_some((index, target, rotation))
                    })
                })
            });
            let Some((index, target, rotation)) = found else {
                return;
            };
            desk.selected = Some(index);
            desk.rotation = rotation;
            desk.look_at(target.level);
            // Point at it. (Moving the real cursor needs it locked on Wayland; the desk
            // keeps the cell it was given while the cursor is off the window.)
            desk.hovered = Some(target);
            desk.click_cell(target);
            request.last_shot_tick = tick;
            request.stills = 2;
        }
        2 if tick >= request.last_shot_tick + 20 => {
            shoot(&mut commands, "play");
            // Confirm it as PLAY does.
            if let (Some(index), Some(target)) = (desk.selected, desk.aimed)
                && let Some(card) = desk
                    .hand(ascent.session())
                    .and_then(|hand| hand.deck.hand.get(index).copied())
            {
                let play = ArchitectCommand::Play {
                    card: card.id,
                    target,
                    rotation: desk.rotation,
                };
                let refusal = ascent.session().architect_refusal(desk.seat, play);
                desk.settle(play, refusal);
            }
            request.last_shot_tick = tick;
            request.stills = 3;
        }
        // Caught while it builds in, however the ticks fall against the frames.
        3 if building_in.iter().any(|room| room.age > 0.25)
            || tick >= request.last_shot_tick + 120 =>
        {
            shoot(&mut commands, "building-in");
            request.last_shot_tick = tick;
            request.stills = 4;
        }
        4 if tick >= request.last_shot_tick + 90 => {
            shoot(&mut commands, "built");
            let played = ascent.rules().command_log.len();
            info!("{prefix} capture: the rules have logged {played} plays");
            // Nobody asks the Rogue for help, and it looks through nobody's eyes: it leaves
            // once the still has had frames enough to be written.
            request.last_shot_tick = tick;
            request.stills = if rogue { 11 } else { 5 };
        }
        // A teammate's request, as the desk shows it, and once it has been answered.
        5 => {
            let asked = super::requests::oldest_unanswered(ascent.session(), &desk);
            if let Some(asked) = asked {
                desk.look_at(asked.target.level);
                request.last_shot_tick = tick;
                request.stills = 6;
            } else if tick >= request.last_shot_tick + 6_000 {
                warn!("architect capture: no teammate asked for help; no request stills");
                request.stills = 8;
            }
        }
        6 if tick >= request.last_shot_tick + 10 => {
            shoot(&mut commands, "request");
            super::requests::answer_oldest(&mut desk, &runtime);
            request.last_shot_tick = tick;
            request.stills = 7;
        }
        7 if tick >= request.last_shot_tick + 20 => {
            shoot(&mut commands, "answered");
            request.last_shot_tick = tick;
            request.stills = 8;
        }
        // Off the desk, through the eyes of a teammate other than the local body if there
        // is one, once the facility has streamed in around it.
        8 => {
            let eyes = super::eyes::eyes_for(&runtime, &desk);
            desk.eyes = eyes
                .iter()
                .copied()
                .find(|&eye| eye != runtime.local_player)
                .or_else(|| eyes.first().copied());
            request.last_shot_tick = tick;
            request.stills = 9;
        }
        9 if tick >= request.last_shot_tick + 150 => {
            shoot(&mut commands, "eyes");
            request.last_shot_tick = tick;
            request.stills = 10;
        }
        // The Rogue's directive, once its own clock is ready.
        11 if desk
            .hand(ascent.session())
            .is_some_and(|hand| hand.cooldown == 0) =>
        {
            let played = play_order(
                &mut runtime,
                &mut desk,
                CardKind::Directive,
                directive_target,
            );
            let Some(target) = played else {
                if tick >= request.last_shot_tick + 6_000 {
                    warn!("rogue capture: no major Guardian to direct; no directive stills");
                    request.last_shot_tick = tick + 300;
                    request.stills = 10;
                }
                return;
            };
            info!("rogue capture: Guardians directed to {target:?}");
            request.last_shot_tick = tick;
            request.stills = 12;
        }
        12 if tick >= request.last_shot_tick + 30 => {
            shoot(&mut commands, "directed");
            request.last_shot_tick = tick;
            request.stills = 13;
        }
        13 if ascent.rules().directed.is_none() || tick >= request.last_shot_tick + 420 => {
            shoot(&mut commands, "directed-walked");
            request.last_shot_tick = tick;
            request.stills = 14;
        }
        // A sensor, once the clock is ready again.
        14 if desk
            .hand(ascent.session())
            .is_some_and(|hand| hand.cooldown == 0) =>
        {
            let played = play_order(&mut runtime, &mut desk, CardKind::Sensor, sensor_target);
            let Some(target) = played else {
                if tick >= request.last_shot_tick + 3_000 {
                    warn!("rogue capture: nowhere to install a sensor; no sensor still");
                    request.last_shot_tick = tick + 300;
                    request.stills = 10;
                }
                return;
            };
            info!("rogue capture: a sensor installed at {target:?}");
            request.last_shot_tick = tick;
            request.stills = 15;
        }
        15 if tick >= request.last_shot_tick + 30 => {
            shoot(&mut commands, "sensor");
            // It leaves once the still has had frames enough to be written.
            request.last_shot_tick = tick + 300;
            request.stills = 10;
        }
        10 if tick >= request.last_shot_tick + 20 => {
            desk.eyes = None;
            info!("{prefix} capture complete");
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

/// Play one of the Rogue's orders through the desk as a player would - the card picked
/// up, aimed at the cell `pick` chooses, and confirmed - with a card of `kind` staged into
/// the hand (`AscentRules::stage_card`). Where it was aimed, if anywhere.
fn play_order(
    runtime: &mut HexWfcRuntime,
    desk: &mut ArchitectDesk,
    kind: CardKind,
    pick: fn(&HexWfcRuntime, &ArchitectDesk, CardId) -> Option<observed_hex::HexCoord>,
) -> Option<observed_hex::HexCoord> {
    let card = runtime.ascent.as_mut()?.stage_card(desk.seat, kind)?;
    let target = pick(runtime, desk, card)?;
    let index = desk
        .hand(runtime.ascent.as_ref()?.session())?
        .deck
        .hand
        .iter()
        .position(|held| held.id == card)?;
    desk.selected = Some(index);
    desk.rotation = 0;
    desk.look_at(target.level);
    desk.hovered = Some(target);
    desk.click_cell(target);
    super::input::confirm_play(desk, runtime);
    desk.pending.is_some().then_some(target)
}

/// The Rogue's order `card` on `target`.
const fn order(card: CardId, target: observed_hex::HexCoord) -> ArchitectCommand {
    ArchitectCommand::Play {
        card,
        target,
        rotation: 0,
    }
}

/// A cell the rules would take a directive to, four to six steps' walk from a major
/// Guardian the Rogue board sees: near enough that the still shows it walking there.
fn directive_target(
    runtime: &HexWfcRuntime,
    desk: &ArchitectDesk,
    card: CardId,
) -> Option<observed_hex::HexCoord> {
    let ascent = runtime.ascent.as_ref()?;
    let rules = ascent.rules();
    let knowledge = desk.knowledge(rules)?;
    let facility = &runtime.match_state.facility;
    knowledge
        .visible_guardians
        .iter()
        .filter_map(|id| rules.guardians.get(id))
        .filter(|guardian| guardian.kind == observed_match::ascent::sim::GuardianKind::Major)
        .find_map(|guardian| {
            knowledge
                .cells
                .keys()
                .filter(|cell| cell.level == guardian.cell.level)
                .filter(|&&target| {
                    ascent
                        .session()
                        .architect_refusal(desk.seat, order(card, target))
                        .is_none()
                })
                .find(|&&target| {
                    facility
                        .route_between_cells(guardian.cell, target)
                        .is_some_and(|route| (5..=7).contains(&route.cells.len()))
                })
                .copied()
        })
}

/// A cell the rules would take a sensor on whose sight holds a loyal body, or failing that
/// one beside a loyal body.
fn sensor_target(
    runtime: &HexWfcRuntime,
    desk: &ArchitectDesk,
    card: CardId,
) -> Option<observed_hex::HexCoord> {
    let ascent = runtime.ascent.as_ref()?;
    let rules = ascent.rules();
    let bodies: Vec<observed_hex::HexCoord> = rules
        .observers
        .values()
        .filter(|o| o.state == observed_match::ascent::sim::ObserverState::Active)
        .map(|o| o.cell)
        .collect();
    let legal = |target: observed_hex::HexCoord| {
        ascent
            .session()
            .architect_refusal(desk.seat, order(card, target))
            .is_none()
    };
    let near = |target: &observed_hex::HexCoord| {
        bodies.iter().any(|body| {
            body.level == target.level && observed_hex::travel_distance(*body, *target) <= 4
        })
    };
    let candidates: Vec<observed_hex::HexCoord> = rules
        .world
        .placements
        .keys()
        .copied()
        .filter(near)
        .filter(|&target| legal(target))
        .collect();
    candidates
        .iter()
        .copied()
        .find(|&target| {
            let sight = rules.sensor_sight(target);
            bodies.iter().any(|body| sight.contains(body))
        })
        .or_else(|| candidates.first().copied())
}
