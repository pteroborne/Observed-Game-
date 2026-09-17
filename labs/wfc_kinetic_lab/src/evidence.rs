//! One command tick per rendered frame; screenshots never skip simulation state.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::demo;
use crate::model::{ActorId, Mode};
use crate::runtime::Runtime;

/// What a capture run is recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode2 {
    /// One still of the opening view.
    Still,
    /// The five staged scenes.
    Scenes,
    /// One continuous encounter, played by the director.
    Loop,
}

#[derive(Resource)]
pub struct Capture {
    pub destination: String,
    pub sequence: bool,
    pub mode: Mode2,
    pub frame: u32,
    pub target: Option<ActorId>,
    pub report: String,
    /// The frame the encounter resolved on, so a recording ends shortly after
    /// its outcome rather than holding on a finished world.
    pub settled: Option<u32>,
}

pub fn advance(
    mut commands: Commands,
    capture: Option<ResMut<Capture>>,
    mut runtime: ResMut<Runtime>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut capture) = capture else {
        return;
    };
    capture.frame += 1;
    if capture.frame == 1 {
        runtime.reset(Mode::Practice);
        runtime.paused = false;
        capture.report.push_str(&format!(
            "seed {} (requested {})\nplan {}\ncells {} voids {} ledges {} hulls {}\nretracting ({}, {})\n",
            runtime.site.seed,
            runtime.site.requested_seed,
            runtime.site.plan(),
            runtime.site.cells.len(),
            runtime.site.voids.len(),
            runtime.site.ledges.len(),
            runtime.site.colliders().len(),
            runtime.site.retracting.q,
            runtime.site.retracting.r,
        ));
    }
    // Let the facility finish streaming in before anything is recorded.
    if capture.frame < 40 {
        return;
    }
    let frame = capture.frame - 40;
    if capture.mode == Mode2::Loop {
        if frame == 0 {
            runtime.reset(Mode::Encounter);
            runtime.paused = false;
            capture
                .report
                .push_str("\nGAMEPLAY LOOP / one encounter, played by the director\n");
        }
        // An encounter that has ended is not footage. Hold briefly on the
        // result, then stop.
        if runtime.world.outcome != crate::model::Outcome::Playing && capture.settled.is_none() {
            capture.settled = Some(frame);
        }
        let ends_at = capture
            .settled
            .map_or(demo::LOOP_TICKS, |settled| settled + 150);
        if frame >= ends_at {
            if frame == ends_at {
                capture.report.push_str(&format!(
                    "ended {:?} after {} ticks, removed {}, wave {}\nfinal digest {:016x}\n",
                    runtime.world.outcome,
                    runtime.world.tick,
                    runtime.world.kills,
                    runtime.world.wave,
                    runtime.world.digest(),
                ));
                std::fs::write(
                    format!("{}/verification.txt", capture.destination),
                    &capture.report,
                )
                .expect("write capture verification");
            }
            if frame > demo::LOOP_TICKS + 30 {
                exit.write(AppExit::Success);
            }
            return;
        }
        let input = demo::loop_command(&runtime.world, frame);
        runtime.movement = input.movement;
        runtime.pending.push_back(input.action);
        runtime.tick();
        for event in &runtime.world.events {
            // Refusals are the director missing its window; they are noise in a
            // continuous recording, unlike in a staged scene where they are the
            // point.
            if !matches!(event, crate::model::Event::Refused(_)) {
                capture
                    .report
                    .push_str(&format!("tick {}: {event:?}\n", runtime.world.tick));
            }
        }
        // Every second tick: at 30 fps that plays back in real time, and a
        // video is not paying the palette cost that keeps the staged GIF at 15.
        if frame.is_multiple_of(2) {
            let path = format!("{}/frame_{:04}.png", capture.destination, frame / 2);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        return;
    }
    if capture.sequence {
        let total = demo::SCENE_TICKS * demo::SCENES.len() as u32;
        if frame >= total {
            if frame == total {
                std::fs::write(
                    format!("{}/verification.txt", capture.destination),
                    &capture.report,
                )
                .expect("write capture verification");
            }
            if frame > total + 30 {
                exit.write(AppExit::Success);
            }
            return;
        }
        let scene = (frame / demo::SCENE_TICKS) as usize;
        let tick = frame % demo::SCENE_TICKS;
        if tick == 0 {
            let staged = demo::stage(scene, &runtime.site);
            runtime.reset(Mode::Practice);
            runtime.world = staged.world;
            runtime.previous_player = runtime.world.player.position;
            runtime.paused = false;
            capture.target = staged.target;
            capture
                .report
                .push_str(&format!("\n{}\n", demo::SCENES[scene]));
        }
        let input = demo::command(scene, tick, &runtime.world, capture.target);
        runtime.movement = input.movement;
        runtime.pending.push_back(input.action);
        runtime.tick();
        for event in &runtime.world.events {
            capture
                .report
                .push_str(&format!("tick {tick}: {event:?}\n"));
        }
        if tick == demo::SCENE_TICKS - 1 {
            capture.report.push_str(&format!(
                "final digest {:016x}, removed {}, player {:?}\n",
                runtime.world.digest(),
                runtime.world.kills,
                runtime.world.player.position
            ));
        }
        if frame.is_multiple_of(4) {
            let path = format!("{}/frame_{:04}.png", capture.destination, frame / 4);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
    } else {
        if frame < 60 {
            runtime.tick();
        }
        if frame == 65 {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(capture.destination.clone()));
        }
        if frame > 95 {
            exit.write(AppExit::Success);
        }
    }
}
