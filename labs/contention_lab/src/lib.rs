//! Phase 38 — **Contested Observation**: team attribution over the shared
//! decoherence graph.
//!
//! The technical question: does shared, team-attributed observation with anchors create
//! real competitive interaction while preserving determinism and solvability?
//!
//! This lab visualizes a `ContentionWorld` (a team-aware extension of `ObservationWorld`)
//! built from a 3×3 lattice with four single-member teams. Members and anchors freeze
//! rooms for all teams (shared, objective observation); each team maintains its own
//! knowledge ledger of observed doorway links (fog of war over truth, not geometry).
//! A solvability guard ensures no member is ever stranded from the exit, even as
//! teams strategically position members and anchors to rewire the unobserved graph.

pub mod experiment;
mod lab;

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};

pub use lab::ContentionRuntime;
pub use observed_observation::contention::ContentionWorld;

pub struct ContentionLabPlugin;

impl Plugin for ContentionLabPlugin {
    fn build(&self, app: &mut App) {
        // Build the authored 3x3 lattice into a ContentionWorld with four teams
        let edges = make_authored_edges();
        let world = ContentionWorld::new(
            observed_observation::ROOM_COUNT,
            &edges,
            &[
                (observed_core::TeamId(0), observed_core::RoomId(0)),
                (observed_core::TeamId(1), observed_core::RoomId(2)),
                (observed_core::TeamId(2), observed_core::RoomId(6)),
                (observed_core::TeamId(3), observed_core::RoomId(4)),
            ],
            observed_core::RoomId(8), // exit
            0x00C0_FFEE_DEAD_BEEF,
        );

        app.insert_resource(world)
            .init_resource::<ContentionRuntime>()
            .add_systems(Startup, (setup_camera, lab::setup_lab))
            .add_systems(
                Update,
                (
                    lab::handle_input,
                    lab::perform_reset,
                    lab::present_members,
                    lab::draw_debug,
                    lab::update_debug_text,
                )
                    .chain(),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: 1.55,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 40.0, 1000.0),
        Name::new("Contention Lab Camera"),
    ));
}

pub fn run() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.010, 0.018, 0.030)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Observed 2 — Contention Lab".to_string(),
                resolution: WindowResolution::new(1440, 900),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(ContentionLabPlugin);

    if let Ok(path) = std::env::var("OBSERVED2_CAPTURE") {
        app.insert_resource(CaptureRequest { path, phase: 0 })
            .add_systems(Update, capture_progress);
    }

    app.run();
}

#[derive(Resource)]
struct CaptureRequest {
    path: String,
    phase: u8,
}

fn capture_progress(
    time: Res<Time>,
    mut request: ResMut<CaptureRequest>,
    mut world: ResMut<ContentionWorld>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed = time.elapsed_secs();
    if request.phase == 0 {
        // Let the world stabilize and demonstrate some interaction
        world.record_observations();
        for _ in 0..4 {
            world.decohere();
            world.record_observations();
        }
        request.phase = 1;
    } else if request.phase == 1 && elapsed >= 0.6 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(request.path.clone()));
        request.phase = 2;
    } else if request.phase == 2 && elapsed >= 1.4 {
        exit.write(AppExit::Success);
    }
}

/// Rebuild the 3×3 lattice edges locally to avoid depending on private `authored_edges()`.
/// The lattice is: 0 - 1 - 2
///                 |   |   |
///                 3 - 4 - 5
///                 |   |   |
///                 6 - 7 - 8
fn make_authored_edges() -> Vec<(
    observed_core::RoomId,
    observed_core::Direction,
    observed_core::RoomId,
    observed_core::Direction,
)> {
    use observed_core::{Direction, RoomId};
    use observed_observation::{COLS, ROWS};

    let mut edges = Vec::new();
    for r in 0..ROWS {
        for c in 0..COLS {
            let room = RoomId(r * COLS + c);
            if c + 1 < COLS {
                edges.push((
                    room,
                    Direction::East,
                    RoomId(r * COLS + c + 1),
                    Direction::West,
                ));
            }
            if r + 1 < ROWS {
                edges.push((
                    room,
                    Direction::South,
                    RoomId((r + 1) * COLS + c),
                    Direction::North,
                ));
            }
        }
    }
    edges
}
