//! Evidence capture: the `OBSERVED2_CAPTURE*` screenshot systems used to regenerate
//! the docs evidence. Each is opt-in via an environment variable (see [`configure`]),
//! drives the game into the Match, frames a deterministic shot, saves it, and exits.
//! None of this runs in normal play.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_core::RoomId;
use observed_match::hybrid::{HybridMatch, LocalAction};
use observed_match::maze::{GRID_H, GRID_W, TILE_SIZE};

use crate::flow::{self, Career};
use crate::{GameState, hallway, items, keystones, screens, teleport};

/// Wire up whichever capture system the environment requests (at most one). Called from
/// [`crate::run`] after the game plugin is added; a no-op in normal play.
pub fn configure(app: &mut App) {
    if std::env::var("OBSERVED2_CAPTURE_TOUR").is_ok() {
        app.insert_resource(TourCapture {
            phase: 0,
            shot: 0,
            next_at: 0.0,
        })
        .add_systems(
            Update,
            capture_tour_progress.after(screens::present_match_camera),
        );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_MATCH") {
        app.insert_resource(MatchCaptureRequest { path, phase: 0 })
            .add_systems(
                Update,
                capture_match_progress.after(screens::present_match_camera),
            );
    } else if let Ok((path, into_maze)) = std::env::var("OBSERVED2_CAPTURE_MAZE")
        .map(|p| (p, true))
        .or_else(|_| std::env::var("OBSERVED2_CAPTURE_ROOM").map(|p| (p, false)))
    {
        app.insert_resource(MazeCaptureRequest {
            path,
            phase: 0,
            next_at: 0.0,
            into_maze,
        })
        .add_systems(
            Update,
            capture_maze_progress.after(screens::present_match_camera),
        );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_KEYSTONE") {
        app.insert_resource(KeystoneCaptureRequest { path, phase: 0 })
            .add_systems(
                Update,
                capture_keystone_progress.after(screens::present_match_camera),
            );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE_RIVALS") {
        app.insert_resource(RivalCaptureRequest {
            path,
            phase: 0,
            next_at: 0.0,
        })
        .add_systems(
            Update,
            capture_rivals_progress.after(screens::present_match_camera),
        );
    } else if let Ok((path, from_hallway)) = std::env::var("OBSERVED2_CAPTURE_DOORWAY_HALL")
        .map(|p| (p, true))
        .or_else(|_| std::env::var("OBSERVED2_CAPTURE_DOORWAY").map(|p| (p, false)))
    {
        app.insert_resource(DoorwayCaptureRequest {
            path,
            phase: 0,
            next_at: 0.0,
            from_hallway,
        })
        .add_systems(
            Update,
            capture_doorway_progress.after(screens::present_match_camera),
        );
    } else if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }
}

/// Evidence capture: stand in the start room facing the spine forward door so it slides
/// open and reveals the previewed hallway beyond. Set `OBSERVED2_CAPTURE_DOORWAY=<path>`.
#[derive(Resource)]
struct DoorwayCaptureRequest {
    path: String,
    phase: u8,
    next_at: f32,
    /// Shoot a hallway looking at its exit (room preview) instead of a room looking at
    /// its forward door (hallway preview).
    from_hallway: bool,
}

#[allow(clippy::too_many_arguments)]
fn capture_doorway_progress(
    time: Res<Time>,
    mut request: ResMut<DoorwayCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<screens::MatchRuntime>>,
    tp: Option<ResMut<screens::TeleportState>>,
    keys: Option<Res<keystones::KeystoneState>>,
    item_state: Option<Res<items::ItemsState>>,
    mut leaves: Query<(&screens::DoorLeaf, &mut Transform)>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let (Some(mut rt), Some(mut tp), Some(keys), Some(item_state)) =
                (runtime, tp, keys, item_state)
            {
                rt.done = true; // freeze the sim so the body holds position
                rt.live.host.match_state.reroute_feedback_ticks = 0;
                // Optionally drop into a spine hallway so we frame its exit → room preview.
                if request.from_hallway {
                    let (from, to) = {
                        let game = rt.live.host_match();
                        let from = game.local_room();
                        let to = game.local_target().unwrap_or(RoomId(from.0 + 1));
                        (from, to)
                    };
                    let variation = hallway::variation_for(
                        from,
                        to,
                        flow::MATCH_SEED,
                        rt.live.host_match().reroute_commits,
                    );
                    screens::debug_place_into(
                        &mut tp,
                        &rt,
                        teleport::Place::Hallway {
                            from,
                            to,
                            variation,
                        },
                        from,
                        &keys,
                        &item_state,
                    );
                }
                let aim = if request.from_hallway {
                    tp.geom
                        .gaps
                        .iter()
                        .find(|g| g.kind == teleport::GapKind::Exit)
                        .copied()
                } else {
                    tp.geom.forward_gap().copied()
                };
                if let Some(gap) = aim {
                    let n = gap.normal;
                    // Stand back from the door, looking through (+normal) and tilted down
                    // a touch so the lit floor beyond frames up.
                    let back = if request.from_hallway { 3.0 } else { 1.6 };
                    let stand = gap.center - n * back;
                    let hh = tp.config.half_height;
                    tp.body.position = Vec3::new(stand.x, hh, stand.y);
                    tp.body.yaw = n.x.atan2(-n.y);
                    tp.body.pitch = -0.14;
                }
                // Force every leaf fully open so the preview behind is unobstructed.
                for (leaf, mut transform) in &mut leaves {
                    transform.translation.y = leaf.open_y;
                }
                request.phase = 2;
                request.next_at = elapsed + 0.4;
            }
        }
        2 if elapsed >= request.next_at => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(request.path.clone()));
            request.phase = 3;
            request.next_at = elapsed + 1.0;
        }
        3 if elapsed >= request.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

/// Evidence capture: drop into a keystone room first-person so the gold keystone item +
/// the LOCKED exit HUD read. Set `OBSERVED2_CAPTURE_KEYSTONE=<path>`.
#[derive(Resource)]
struct KeystoneCaptureRequest {
    path: String,
    phase: u8,
}

#[allow(clippy::too_many_arguments)]
fn capture_keystone_progress(
    time: Res<Time>,
    mut request: ResMut<KeystoneCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<screens::MatchRuntime>>,
    tp: Option<ResMut<screens::TeleportState>>,
    keys: Option<Res<keystones::KeystoneState>>,
    item_state: Option<Res<items::ItemsState>>,
    mut cam: Query<&mut Transform, With<screens::GameCam>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let (Some(mut rt), Some(mut tp), Some(keys), Some(item_state)) =
                (runtime, tp, keys, item_state)
            {
                rt.done = true;
                if let Some(&room) = keys.rooms.first() {
                    screens::debug_place_into(
                        &mut tp,
                        &rt,
                        teleport::Place::Room(room),
                        room,
                        &keys,
                        &item_state,
                    );
                    // Stand back from the centre keystone so it stays visible (and out of
                    // pickup range) for the shot.
                    tp.body.position = Vec3::new(0.0, tp.config.half_height, 5.0);
                }
                request.phase = 2;
            }
        }
        2 if elapsed >= 0.8 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(request.path.clone()));
            request.phase = 3;
        }
        3 if elapsed >= 1.8 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
    // Frame the keystone at the room centre each frame (overriding the first-person
    // `present_match_camera`, which we run after).
    if request.phase >= 2
        && let Ok(mut transform) = cam.single_mut()
    {
        *transform =
            Transform::from_xyz(0.0, 1.7, 5.2).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
    }
}

/// Evidence capture: freeze the match at round 0 (every team still clumped at the
/// entrance) so the rival avatars walking the room you share read, then shoot it from an
/// oblique vantage. Set `OBSERVED2_CAPTURE_RIVALS=<path>`.
#[derive(Resource)]
struct RivalCaptureRequest {
    path: String,
    phase: u8,
    next_at: f32,
}

#[allow(clippy::too_many_arguments)]
fn capture_rivals_progress(
    time: Res<Time>,
    mut request: ResMut<RivalCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    mut runtime: Option<ResMut<screens::MatchRuntime>>,
    mut cam: Query<&mut Transform, With<screens::GameCam>>,
    geometry: Query<(Entity, &Name), With<screens::PlaceGeometry>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let Some(rt) = runtime.as_mut() {
                // Freeze at round 0 so all four teams stay clumped at the entrance and the
                // three rivals keep pacing the room. Let them walk a beat into the frame.
                rt.done = true;
                request.phase = 2;
                request.next_at = elapsed + 1.2;
            }
        }
        2 if elapsed >= request.next_at => {
            // Drop the ceiling so the high oblique shot looks down into the room cleanly.
            for (entity, name) in &geometry {
                if name.as_str() == "Place ceiling" {
                    commands.entity(entity).despawn();
                }
            }
            request.phase = 3;
            request.next_at = elapsed + 0.4;
        }
        3 if elapsed >= request.next_at => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(request.path.clone()));
            request.phase = 4;
            request.next_at = elapsed + 1.0;
        }
        4 if elapsed >= request.next_at => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
    // High oblique vantage (above the 3.4 m walls, looking down into the room) so the
    // pacing rival figures + neon room both read, clearing the near wall (overrides the
    // first-person `present_match_camera`, which we run after).
    if request.phase >= 2
        && let Ok(mut transform) = cam.single_mut()
    {
        *transform =
            Transform::from_xyz(0.0, 9.0, 9.0).looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y);
    }
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut career: ResMut<Career>,
    mut next: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Give the career some history so the main-menu banner reads a real level,
        // then jump to the main menu for the cohesive title shot.
        for _ in 0..4 {
            career.record(flow::play_match());
            career.award();
        }
        next.set(GameState::MainMenu);
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.8 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.6 {
        exit.write(AppExit::Success);
    }
}

#[derive(Resource)]
struct MatchCaptureRequest {
    path: String,
    phase: u8,
}

/// Capture the in-game networked match: enter Match, drive the lockstep transport a
/// few rounds down the spine, freeze, and shoot the real first-person neon-noir view.
fn capture_match_progress(
    time: Res<Time>,
    mut request: ResMut<MatchCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    mut runtime: Option<ResMut<screens::MatchRuntime>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        next.set(GameState::Match);
        request.phase = 1;
    } else if request.phase == 1 {
        // Once the Match screen's runtime exists, resolve a few rounds (replicating
        // each over the network) and freeze, so the first-person view shows the maze
        // partway through the match.
        if let Some(runtime) = runtime.as_mut() {
            for _ in 0..5 {
                if runtime.live.finished() {
                    break;
                }
                let action = if runtime.live.local_active() {
                    LocalAction::Advance
                } else {
                    LocalAction::Wait
                };
                runtime.live.force_round(action);
                for _ in 0..400 {
                    if runtime.live.in_sync() {
                        break;
                    }
                    runtime.live.pump();
                }
            }
            runtime.done = true;
            // Clear the reroute flash so the frozen capture isn't stuck behind the
            // full-screen ROUTE SHIFT overlay.
            runtime.live.host.match_state.reroute_feedback_ticks = 0;
            request.phase = 2;
        }
    } else if request.phase == 2 && elapsed >= 2.5 {
        // The forced advances walked the player several rooms down the spine, and
        // `present_match_camera` already places the eye — so just shoot the real
        // first-person view of the re-skinned neon-noir maze.
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 3;
    } else if request.phase == 3 && elapsed >= 3.5 {
        exit.write(AppExit::Success);
    }
}

/// Rooms (and labels) the diagnostic tour photographs from above, after the
/// overview shot. 0=start, 1=a decor room, 3=control, 8=exit.
const TOUR_ROOMS: [(usize, &str); 4] = [(0, "start"), (1, "decor"), (3, "control"), (8, "exit")];

#[derive(Resource)]
struct TourCapture {
    phase: u8,
    shot: usize,
    next_at: f32,
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
fn capture_tour_progress(
    time: Res<Time>,
    mut tour: ResMut<TourCapture>,
    mut runtime: Option<ResMut<screens::MatchRuntime>>,
    mut next: ResMut<NextState<GameState>>,
    mut cam: Query<&mut Transform, With<screens::GameCam>>,
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

/// Evidence capture: drop into a labyrinth hallway (or stay in the start room), drop the
/// ceiling, and shoot it straight down so the generated geometry reads as a plan. Set
/// `OBSERVED2_CAPTURE_MAZE=<path>` (a maze hallway) or `OBSERVED2_CAPTURE_ROOM=<path>`
/// (a polygon room).
#[derive(Resource)]
struct MazeCaptureRequest {
    path: String,
    phase: u8,
    next_at: f32,
    /// True → drop into a maze hallway; false → photograph the start polygon room.
    into_maze: bool,
}

#[allow(clippy::too_many_arguments)]
fn capture_maze_progress(
    time: Res<Time>,
    mut request: ResMut<MazeCaptureRequest>,
    mut next: ResMut<NextState<GameState>>,
    runtime: Option<ResMut<screens::MatchRuntime>>,
    tp: Option<ResMut<screens::TeleportState>>,
    keys: Option<Res<keystones::KeystoneState>>,
    item_state: Option<Res<items::ItemsState>>,
    mut cam: Query<&mut Transform, With<screens::GameCam>>,
    geometry: Query<(Entity, &Name), With<screens::PlaceGeometry>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    match request.phase {
        0 => {
            next.set(GameState::Match);
            request.phase = 1;
        }
        1 => {
            if let (Some(mut rt), Some(mut tp), Some(keys), Some(item_state)) =
                (runtime, tp, keys, item_state)
            {
                rt.done = true; // freeze the sim so the body stays put
                if request.into_maze {
                    let (from, to) = {
                        let game = rt.live.host_match();
                        let from = game.local_room();
                        let to = game.local_target().unwrap_or(RoomId(from.0 + 1));
                        (from, to)
                    };
                    // The deep 6×7 labyrinth reads best from above.
                    let variation = hallway::TEMPLATES
                        .iter()
                        .position(|t| t.grid == Some((6, 7)))
                        .unwrap_or(0);
                    screens::debug_place_into(
                        &mut tp,
                        &rt,
                        teleport::Place::Hallway {
                            from,
                            to,
                            variation,
                        },
                        from,
                        &keys,
                        &item_state,
                    );
                }
                if !request.into_maze {
                    // Walk the spine until the current room is a many-sided polygon, so
                    // the evidence shows an angled room rather than a plain rectangle.
                    for _ in 0..12 {
                        let room = rt.live.host_match().local_room();
                        screens::debug_place_into(
                            &mut tp,
                            &rt,
                            teleport::Place::Room(room),
                            room,
                            &keys,
                            &item_state,
                        );
                        if tp.geom.poly.as_ref().map_or(0, |p| p.len()) >= 6 {
                            break;
                        }
                        let act = if rt.live.local_active() {
                            LocalAction::Advance
                        } else {
                            LocalAction::Wait
                        };
                        rt.live.force_round(act);
                        for _ in 0..400 {
                            if rt.live.in_sync() {
                                break;
                            }
                            rt.live.pump();
                        }
                    }
                }
                request.phase = 2;
                request.next_at = elapsed + 0.6;
            }
        }
        2 => {
            if elapsed >= request.next_at {
                // Drop the ceiling so the labyrinth is visible from straight above.
                for (entity, name) in &geometry {
                    if name.as_str() == "Place ceiling" {
                        commands.entity(entity).despawn();
                    }
                }
                request.phase = 3;
                request.next_at = elapsed + 0.4;
            }
        }
        3 => {
            if elapsed >= request.next_at {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(request.path.clone()));
                request.phase = 4;
                request.next_at = elapsed + 1.0;
            }
        }
        _ => {
            if elapsed >= request.next_at {
                exit.write(AppExit::Success);
            }
        }
    }
    // Once we've dropped in, hold the camera straight down over the maze every frame
    // (overriding the first-person `present_match_camera`, which we run after).
    if request.phase >= 2
        && let Ok(mut transform) = cam.single_mut()
    {
        *transform = Transform::from_xyz(0.0, 42.0, 0.1).looking_at(Vec3::ZERO, Vec3::NEG_Z);
    }
}
