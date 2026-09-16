//! One command tick per rendered frame; screenshots never skip simulation state.
use crate::{
    demo,
    model::{ActorId, Mode},
    runtime::Runtime,
};
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
#[derive(Resource)]
pub struct Capture {
    pub destination: String,
    pub sequence: bool,
    pub frame: u32,
    pub target: Option<ActorId>,
    pub report: String,
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
    }
    if capture.frame < 40 {
        return;
    }
    let frame = capture.frame - 40;
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
            let staged = demo::stage(scene);
            runtime.reset(Mode::Practice);
            runtime.world = staged.world;
            runtime.previous_player = runtime.world.player.position;
            runtime.paused = false;
            runtime.demo_label = Some(demo::SCENES[scene]);
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
