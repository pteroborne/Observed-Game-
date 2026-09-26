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
//! 6. and answered at the desk (`architect-answered`).
//!
//! The only staging is choosing the play and where it points; the play itself goes
//! through the desk, the rules and the physical facility like any other.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_match::ascent::sim::{ArchitectCommand, CardKind};

use super::ArchitectDesk;
use super::board::Board;
use super::feedback::BuildIn;
use crate::hex_wfc::{HexWfcCapture, HexWfcCaptureMode, sim::HexWfcRuntime};

/// Ticks the bots walk before the first still: time to map a floor.
const MAPPING_TICKS: u64 = 1_200;
const GIVE_UP_FRAMES: u16 = 12_000;

#[allow(clippy::too_many_arguments)]
pub(in crate::hex_wfc) fn capture(
    mut commands: Commands,
    request: Option<ResMut<HexWfcCapture>>,
    runtime: Option<Res<HexWfcRuntime>>,
    desk: Option<ResMut<ArchitectDesk>>,
    board: Option<Res<Board>>,
    mut windows: Query<&mut Window>,
    building_in: Query<&BuildIn>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut request) = request else {
        return;
    };
    if request.mode != HexWfcCaptureMode::Architect {
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
    // The board frames itself before anything is pointed at.
    let (Some(runtime), Some(mut desk), Some(_)) = (runtime, desk, board) else {
        return;
    };
    let Some(ascent) = runtime.ascent.as_ref() else {
        return;
    };
    let tick = runtime.match_state.tick;
    let path = std::path::PathBuf::from(&request.path);
    let shoot = |commands: &mut Commands, name: &str| {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.join(name)));
    };
    match request.stills {
        0 if tick >= MAPPING_TICKS => {
            shoot(&mut commands, "architect-board-1280x800.png");
            request.stills = 1;
        }
        1 => {
            // Pick up the first card with a play the rules would take on a known cell,
            // on any floor, and look at that floor: a tile if one can be played, so the
            // still shows its ghost.
            let Some(knowledge) = ascent.rules().team_knowledge.get(&desk.team) else {
                return;
            };
            let Some(hand) = ascent.session().hands.get(&desk.team) else {
                return;
            };
            let mut cards: Vec<_> = hand.deck.hand.iter().enumerate().collect();
            cards.sort_by_key(|(_, card)| !matches!(card.kind, CardKind::Tile(_)));
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
            shoot(&mut commands, "architect-play-1280x800.png");
            // Confirm it as PLAY does.
            if let (Some(index), Some(target)) = (desk.selected, desk.aimed)
                && let Some(card) = ascent
                    .session()
                    .hands
                    .get(&desk.team)
                    .and_then(|hand| hand.deck.hand.get(index))
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
            shoot(&mut commands, "architect-building-in-1280x800.png");
            request.last_shot_tick = tick;
            request.stills = 4;
        }
        4 if tick >= request.last_shot_tick + 90 => {
            shoot(&mut commands, "architect-built-1280x800.png");
            let played = ascent.rules().command_log.len();
            info!("architect capture: the rules have logged {played} plays");
            request.last_shot_tick = tick;
            request.stills = 5;
        }
        // A teammate's request, as the desk shows it, and once it has been answered.
        5 => {
            let asked = super::requests::oldest_unanswered(ascent.session(), desk.team);
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
            shoot(&mut commands, "architect-request-1280x800.png");
            super::requests::answer_oldest(&mut desk, &runtime);
            request.last_shot_tick = tick;
            request.stills = 7;
        }
        7 if tick >= request.last_shot_tick + 20 => {
            shoot(&mut commands, "architect-answered-1280x800.png");
            request.last_shot_tick = tick;
            request.stills = 8;
        }
        8 if tick >= request.last_shot_tick + 20 => {
            info!("architect capture complete");
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
