//! Native playback evidence from real rule/physics ticks with staged adverse events.
use super::{Shot, shot};
use crate::{GameState, sim::replay::ReplayTape};
use bevy::prelude::*;
use observed_core::PlayerId;
use observed_match::{
    ascent::{
        facility::{AscentRules, architect_seats},
        session::{ASCENT_INPUT_VERSION, InputFrame},
        sim::MatchOutcome,
    },
    hex_wfc::{HexBotDriver, HexInputFrame, HexMatchConfig, HexReleasedKind},
};

#[derive(Resource)]
struct RecordedEvidence(ReplayTape);

pub(super) fn sweep() -> Vec<Shot> {
    if std::env::var_os("OBSERVED2_CAPTURE_REPLAY_VIDEO").is_some() {
        return vec![Shot {
            completion: Some(100),
            ..shot("00_playback", GameState::Replay)
        }];
    }
    [
        "00_follow",
        "01_team",
        "02_floor",
        "03_eyes",
        "04_prison",
        "05_card_event",
        "06_rogue",
        "07_rewind",
        "08_guardians",
    ]
    .into_iter()
    .enumerate()
    .map(|(case, label)| Shot {
        completion: Some(100 + case),
        ..shot(label, GameState::Replay)
    })
    .collect()
}

pub(super) fn stage(world: &mut World) {
    if !world.contains_resource::<RecordedEvidence>() {
        world.insert_resource(RecordedEvidence(fixture()));
    }
    let tape = world.resource::<RecordedEvidence>().0.clone();
    world.insert_resource(tape);
    world.resource_mut::<crate::lan::LanRuntime>().leave();
}

pub(super) fn pose(world: &mut World, case: usize) {
    use crate::screens::replay::{ReplayPlayback, scene::CameraView};
    let tape = world.resource::<ReplayTape>();
    let cursor = match case {
        4 => tape
            .scene_frames
            .iter()
            .find(|f| f.bodies[0].place == observed_match::hex_wfc::HexBodyPlace::Prison)
            .map_or(0, |f| f.sample),
        5 => tape
            .markers
            .iter()
            .find(|m| m.label.contains("played"))
            .map_or(0, |m| m.sample),
        6 => tape.len().saturating_sub(1),
        7 => 0,
        _ => tape
            .scene_frames
            .iter()
            .find(|f| f.tick >= 120)
            .map_or(0, |f| f.sample),
    };
    let event_focus = if case == 8 {
        tape.scene_frames
            .iter()
            .find(|f| f.sample == cursor)
            .and_then(|f| f.guardians.first())
            .map(|g| g.cell)
    } else {
        (case == 5)
            .then(|| {
                tape.markers
                    .iter()
                    .find(|m| m.sample == cursor && m.cell.is_some())
                    .and_then(|m| m.cell)
            })
            .flatten()
    };
    let mut playback = world.resource_mut::<ReplayPlayback>();
    playback.cursor = cursor as f32;
    playback.playing = false;
    if std::env::var_os("OBSERVED2_CAPTURE_REPLAY_VIDEO").is_some() {
        playback.cursor = 0.0;
    }
    playback.event_focus = event_focus;
    playback.view = match case {
        1 => CameraView::Team,
        2 | 6 => CameraView::Floor,
        3 => CameraView::Eyes,
        _ => CameraView::Follow,
    };
}

/// The bodies walk on their deterministic bot inputs. Catch/corruption are staged
/// through physical APIs at known ticks; the rules resolve the actual final outcome.
fn fixture() -> ReplayTape {
    let prepared = crate::hex_wfc::launch::prepare(crate::hex_wfc::launch::HexLaunchSpec {
        requested_seed: 0xF011_FAC1_1177,
        config: HexMatchConfig {
            teams: 2,
            members_per_team: 2,
            guardian: false,
            ..default()
        },
        seed_policy: crate::hex_wfc::launch::HexSeedPolicy::Nearby,
    })
    .expect("replay evidence seed prepares");
    let mut game = prepared.match_state;
    let seats = architect_seats(&game, None);
    let mut rules =
        AscentRules::new(&mut game, prepared.selected_seed, seats).expect("Ascent rules");
    rules.voice(game.players.keys().copied());
    let mut driver = HexBotDriver::new();
    let mut tape = ReplayTape::new_hex_wfc_for_player(&game, PlayerId(0));
    tape.ascent_result = Some(crate::flow::AscentResult {
        outcome: MatchOutcome::Running,
        winner: None,
        local_team: observed_core::TeamId(0),
        role: crate::flow::AscentResultRole::Observer,
        loyal: 4,
        jailed: 0,
        corrupted: 0,
        rogue_by_capture: false,
    });
    tape.record_ascent(&game, &rules);
    for tick in 1..=480 {
        if rules.rules().outcome != MatchOutcome::Running {
            break;
        }
        if tick == 60 {
            let cell = game.guardian.cell;
            game.release_guardian(700, HexReleasedKind::Major, cell);
            game.release_guardian(701, HexReleasedKind::Minor, cell);
        }
        if tick == 180 {
            game.jail(PlayerId(0));
        }
        if tick == 300 {
            game.drop_into_void(PlayerId(2));
        }
        if tick == 420 {
            for id in [PlayerId(1), PlayerId(3)] {
                game.jail(id);
            }
        }
        let commands = game
            .players
            .keys()
            .map(|&id| (id, rules.bot_body_command(&game, &mut driver, id)))
            .collect();
        let seat_commands = if tick == 1 {
            use observed_match::ascent::{
                facility::ROGUE_SEAT,
                session::SeatCommand,
                sim::{ArchitectCommand, CardKind, TileShape},
            };
            let card = rules
                .stage_card(ROGUE_SEAT, CardKind::Tile(TileShape::Corridor))
                .expect("staged tile card");
            let play = rules
                .rules()
                .mutable_targets()
                .into_iter()
                .flat_map(|target| {
                    (0..6).map(move |rotation| ArchitectCommand::Play {
                        card,
                        target,
                        rotation,
                    })
                })
                .find(|&command| {
                    rules
                        .session()
                        .architect_refusal(ROGUE_SEAT, command)
                        .is_none()
                })
                .expect("a legal recorded rewrite");
            std::collections::BTreeMap::from([(ROGUE_SEAT, SeatCommand::Architect(play))])
        } else {
            default()
        };
        rules
            .step(
                &mut game,
                &HexInputFrame {
                    tick,
                    commands,
                    ..default()
                },
                &InputFrame {
                    version: ASCENT_INPUT_VERSION,
                    tick,
                    commands: seat_commands,
                },
            )
            .expect("valid fixture tick");
        tape.record_ascent(&game, &rules);
    }
    tape
}

/// Opt-in native sequence; each GPU readback completes before the next is requested.
#[derive(Resource)]
pub(crate) struct ReplayVideo {
    dir: std::path::PathBuf,
    frame: u32,
    pending: bool,
    prepared: bool,
    next_at: f32,
}
impl ReplayVideo {
    pub(crate) fn new(dir: String) -> Self {
        let dir = std::path::PathBuf::from(dir).join("frames");
        std::fs::create_dir_all(&dir).expect("video frame directory");
        Self {
            dir,
            frame: 0,
            pending: false,
            prepared: false,
            next_at: 0.0,
        }
    }
}
pub(crate) fn capture_video(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut video: ResMut<ReplayVideo>,
    mut request: ResMut<super::FrontendCaptureRequest>,
    tape: Option<Res<ReplayTape>>,
    mut playback: Option<ResMut<crate::screens::replay::ReplayPlayback>>,
    mut commands: Commands,
) {
    if *state.get() != GameState::Replay || video.frame >= 70 {
        return;
    }
    // Keep the screenshot sweep alive until all readbacks finish. Capture evenly
    // spaced recorded times instead of letting GPU latency change playback speed.
    request.next_at = time.elapsed_secs() + 0.5;
    let (Some(tape), Some(playback)) = (tape, playback.as_deref_mut()) else {
        return;
    };
    if video.pending || time.elapsed_secs() < video.next_at {
        return;
    }
    if !video.prepared {
        let last_tick = tape.samples.last().map_or(0, |s| s.live_round);
        let tick = last_tick as f32 * video.frame as f32 / 69.0;
        let i = tape
            .samples
            .partition_point(|s| s.live_round as f32 <= tick)
            .saturating_sub(1);
        let a = tape.samples[i].live_round;
        let b = tape.samples.get(i + 1).map_or(a, |s| s.live_round);
        playback.cursor =
            i as f32 + ((tick - a as f32) / b.saturating_sub(a).max(1) as f32).clamp(0.0, 1.0);
        playback.playing = false;
        video.prepared = true;
        video.next_at = time.elapsed_secs() + 0.15;
        return;
    }
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
    let path = video.dir.join(format!("playback_{:03}.png", video.frame));
    video.pending = true;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            |_: On<ScreenshotCaptured>, time: Res<Time>, mut video: ResMut<ReplayVideo>| {
                video.frame += 1;
                video.pending = false;
                video.prepared = false;
                video.next_at = time.elapsed_secs() + 0.07;
            },
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn recorded_world_retains_motion_cards_prison_corruption_and_revisions() {
        let tape = fixture();
        assert!(tape.scene_frames.len() > 50);
        assert!(tape.scene_frames.windows(2).all(|f| f[0].tick < f[1].tick));
        let first = &tape.scene_frames[0];
        assert!(
            tape.scene_frames
                .iter()
                .any(|f| f.bodies[0].position != first.bodies[0].position)
        );
        assert!(tape.scene_frames.iter().any(|f| f.bodies[0].place
            == observed_match::hex_wfc::HexBodyPlace::Prison
            && !f.prisons.is_empty()));
        assert!(tape.scene_frames.iter().any(|f| {
            f.bodies
                .iter()
                .any(|b| b.place == observed_match::hex_wfc::HexBodyPlace::Void)
        }));
        assert!(tape.scene_frames.iter().any(|f| f.guardians.len() >= 2));
        assert!(
            tape.markers
                .iter()
                .any(|m| m.label.contains("played") && m.cell.is_some())
        );
        assert_eq!(
            tape.ascent_result.unwrap().outcome,
            MatchOutcome::RogueVictory
        );
        assert!(
            tape.scene_frames
                .windows(2)
                .any(|f| Arc::ptr_eq(&f[0].facility, &f[1].facility))
        );
        assert!(
            tape.scene_frames
                .windows(2)
                .any(|f| !Arc::ptr_eq(&f[0].facility, &f[1].facility))
        );
        let changed = tape
            .scene_frames
            .windows(2)
            .find(|f| f[0].facility.generation != f[1].facility.generation)
            .unwrap();
        assert!(
            changed[0].facility.pieces.iter().any(|a| changed[1]
                .facility
                .pieces
                .iter()
                .any(|b| Arc::ptr_eq(a, b)))
        );
        let before = tape.clone();
        for cursor in [tape.len() as f32 - 1.0, 0.0, 20.5, 2.0] {
            let (frame, _, _) = crate::screens::replay::scene::frames(&tape, cursor).unwrap();
            assert_eq!(frame.sample, cursor.floor() as usize);
        }
        assert_eq!(tape, before);
    }
}
