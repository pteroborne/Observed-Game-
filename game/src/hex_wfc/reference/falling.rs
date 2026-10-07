//! Native proof of continuous low/tall-roof entry with the production controller.
use crate::{GameState, hex_wfc::sim::HexWfcRuntime};
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
use observed_hex::{HexCoord, HexFace, hex_origin};

#[derive(Resource)]
struct Capture {
    directory: std::path::PathBuf,
    frames: u32,
    scene: u8,
    at: Option<(HexCoord, Vec3)>,
    before: Option<Vec3>,
}

pub(super) fn install(app: &mut App) {
    let Some(directory) =
        std::env::var_os("OBSERVED2_CAPTURE_ROOF_FALLS").map(std::path::PathBuf::from)
    else {
        return;
    };
    std::fs::create_dir_all(&directory).expect("fall evidence directory");
    app.insert_resource(Capture {
        directory,
        frames: 0,
        scene: 0,
        at: None,
        before: None,
    })
    .add_systems(
        Update,
        drive
            .after(super::controls)
            .run_if(in_state(GameState::HexWfc)),
    );
}

fn drive(
    mut commands: Commands,
    mut runtime: ResMut<HexWfcRuntime>,
    mut capture: ResMut<Capture>,
    mut exit: MessageWriter<AppExit>,
) {
    capture.frames += 1;
    if capture.scene == 0 && capture.at.is_none() && capture.frames < 240 {
        return;
    }
    if capture.at.is_none() {
        let world = &runtime.match_state.facility;
        let grid = world.config.grid();
        let target = world
            .placements
            .keys()
            .filter(|at| {
                if capture.scene == 0 {
                    at.level == 0
                } else {
                    at.level > 0
                }
            })
            .find_map(|&at| {
                if grid.neighbor(at, HexFace::Up).is_some_and(|next| {
                    world.placements.get(&next).is_some_and(|p| p.space.built())
                }) {
                    return None;
                }
                let feet = runtime.match_state.roof_landing_point(at)?;
                let height = feet.y - hex_origin(at)[1];
                ((capture.scene == 0 && height < 4.0) || (capture.scene == 1 && height > 7.0))
                    .then_some((at, feet))
            });
        let Some(target) = target else {
            exit.write(AppExit::error());
            return;
        };
        capture.at = Some(target);
        capture.frames = 0;
    }
    let (cell, feet) = capture.at.expect("staged fall");
    let local = runtime.local_player;
    if capture.frames <= 90 {
        if !runtime.match_state.stage_body_facing(
            local,
            cell,
            feet,
            feet + Vec3::new(4.0, 1.6, 0.0),
        ) {
            exit.write(AppExit::error());
            return;
        }
        capture.before = Some(runtime.local().position);
    }
    let name = if capture.scene == 0 { "low" } else { "tall" };
    if [90, 115, 240].contains(&capture.frames) {
        let path = capture
            .directory
            .join(format!("{name}-{}.png", capture.frames));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if capture.frames == 240 {
        let report = serde_json::json!({"cell":format!("{cell:?}"),"before":capture.before.map(|v|v.to_array()),"after":runtime.local().position.to_array(),"after_cell":format!("{:?}",runtime.local().cell),"continuous_controller":true});
        std::fs::write(
            capture.directory.join(format!("{name}.json")),
            serde_json::to_string_pretty(&report).unwrap(),
        )
        .expect("fall report");
    }
    if capture.frames > 265 {
        if capture.scene == 1 {
            exit.write(AppExit::Success);
        } else {
            capture.scene = 1;
            capture.at = None;
            capture.frames = 0;
        }
    }
}
