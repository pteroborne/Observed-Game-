use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_match::hybrid::{HybridMatch, LocalAction};
use observed_match::maze::{GRID_H, GRID_W, TILE_SIZE};

use crate::GameState;
use crate::screens::{GameCam, MatchRuntime};

/// Rooms (and labels) the diagnostic tour photographs from above, after the
/// overview shot. 0=start, 1=a decor room, 3=control, 8=exit.
const TOUR_ROOMS: [(usize, &str); 4] = [(0, "start"), (1, "decor"), (3, "control"), (8, "exit")];

#[derive(Resource)]
pub(super) struct TourCapture {
    pub(super) phase: u8,
    pub(super) shot: usize,
    pub(super) next_at: f32,
}

impl TourCapture {
    pub(super) fn new() -> Self {
        Self {
            phase: 0,
            shot: 0,
            next_at: 0.0,
        }
    }
}

fn tour_vantage(shot: usize, game: &HybridMatch) -> Transform {
    if shot == 0 {
        // Bird's-eye over the whole facility.
        Transform::from_xyz(0.0, 135.0, 0.1).looking_at(Vec3::ZERO, Vec3::NEG_Z)
    } else {
        let (rx, ry) = game.rooms[TOUR_ROOMS[shot - 1].0].center_tile();
        let cx = (rx as f32 - GRID_W as f32 * 0.5 + 0.5) * TILE_SIZE;
        let cz = (ry as f32 - GRID_H as f32 * 0.5 + 0.5) * TILE_SIZE;
        // Oblique overhead of one room so its props read against the geometry.
        Transform::from_xyz(cx, 16.0, cz + 15.0).looking_at(Vec3::new(cx, 1.0, cz), Vec3::Y)
    }
}

/// Diagnostic: enter the Match, advance a few rounds, freeze, then photograph the
/// facility from an overview and several rooms to inspect asset placement.
pub(super) fn capture_tour_progress(
    time: Res<Time>,
    mut tour: ResMut<TourCapture>,
    mut runtime: Option<ResMut<MatchRuntime>>,
    mut next: ResMut<NextState<GameState>>,
    mut cam: Query<&mut Transform, With<GameCam>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match tour.phase {
        0 => {
            next.set(GameState::Match);
            tour.phase = 1;
        }
        1 => {
            if let Some(rt) = runtime.as_mut() {
                for _ in 0..5 {
                    if rt.live.finished() {
                        break;
                    }
                    let action = if rt.live.local_active() {
                        LocalAction::Advance
                    } else {
                        LocalAction::Wait
                    };
                    rt.live.force_round(action);
                    for _ in 0..400 {
                        if rt.live.in_sync() {
                            break;
                        }
                        rt.live.pump();
                    }
                }
                rt.done = true;
                tour.phase = 2;
                tour.next_at = elapsed + 1.2;
            }
        }
        2 => {
            let Some(rt) = runtime.as_ref() else {
                return;
            };
            let total = 1 + TOUR_ROOMS.len();
            if let Ok(mut transform) = cam.single_mut() {
                *transform = tour_vantage(tour.shot, rt.live.host_match());
            }
            if elapsed >= tour.next_at {
                let path = if tour.shot == 0 {
                    "docs/evidence/tour_0_overview.png".to_string()
                } else {
                    format!(
                        "docs/evidence/tour_{}_{}.png",
                        tour.shot,
                        TOUR_ROOMS[tour.shot - 1].1
                    )
                };
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
                tour.shot += 1;
                tour.next_at = elapsed + 0.7;
                if tour.shot >= total {
                    tour.phase = 3;
                    tour.next_at = elapsed + 0.7;
                }
            }
        }
        _ => {
            if elapsed >= tour.next_at {
                exit.write(AppExit::Success);
            }
        }
    }
}
