//! One deterministic evidence frame when `OBSERVED2_CAPTURE` is set.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use observed_mechanics::tiles::TileShape;

use crate::model::{LabState, Scenario};

#[derive(Resource)]
struct CaptureRun {
    path: String,
    elapsed: f32,
    phase: u8,
}

pub fn configure(app: &mut App) {
    let Ok(path) = std::env::var("OBSERVED2_CAPTURE") else {
        return;
    };
    app.insert_resource(CaptureRun {
        path,
        elapsed: 0.0,
        phase: 0,
    })
    .add_systems(Update, capture);
}

fn capture(
    mut commands: Commands,
    time: Res<Time>,
    run: Option<ResMut<CaptureRun>>,
    mut state: ResMut<LabState>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut run) = run else {
        return;
    };
    run.elapsed += time.delta_secs();
    if run.phase == 0 && run.elapsed >= 0.15 {
        let scenario = match std::env::var("OBSERVED2_CAPTURE_SCENARIO").as_deref() {
            Ok("corner") => Scenario::CornerTurn,
            Ok("safe") => Scenario::SafeSite,
            Ok("preserve") => Scenario::PreserveRoute,
            Ok("free") => Scenario::FreePlay,
            _ => Scenario::ThroughLine,
        };
        state.change_scenario(scenario);
        let wanted = match scenario {
            Scenario::ThroughLine => TileShape::Corridor,
            Scenario::CornerTurn => TileShape::Bend,
            Scenario::SafeSite => TileShape::DeadEnd,
            Scenario::PreserveRoute => state
                .cards
                .iter()
                .find(|card| Some(card.shape) != state.danger_play.map(|play| play.shape))
                .map_or(TileShape::Corridor, |card| card.shape),
            Scenario::FreePlay => TileShape::Junction,
        };
        if let Some(card) = state
            .cards
            .iter()
            .find(|card| card.shape == wanted)
            .copied()
        {
            state.select(card.id);
            if scenario == Scenario::CornerTurn {
                state.rotate(true);
                state.rotate(true);
            }
            let target = state.goal_cell.or_else(|| {
                state.state.board.cells().find(|&cell| {
                    state
                        .preview_at(cell)
                        .is_some_and(|preview| preview.is_valid())
                })
            });
            if let Some(target) = target {
                state.aim(target, false);
                if std::env::var_os("OBSERVED2_CAPTURE_COMPLETE").is_some() {
                    state.commit();
                }
            }
        }
        run.phase = 1;
    } else if run.phase == 1 && run.elapsed >= 0.85 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(run.path.clone()));
        run.phase = 2;
    } else if run.phase == 2 && run.elapsed >= 1.55 {
        exit.write(AppExit::Success);
        run.phase = 3;
    }
}
