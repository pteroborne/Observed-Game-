//! Evidence capture.
//!
//! * `OBSERVED2_CAPTURE=<dir>` shoots one still per authored vantage, writes a
//!   `manifest.json`, and exits.
//! * `OBSERVED2_CAPTURE_WALK=<dir>` walks the tour on the production controller and
//!   saves a numbered frame sequence (`walk_0000.png`, ...) at [`WALK_FPS`], then
//!   exits. Encode it as MP4 with FFmpeg (`-framerate 30`).
//!
//! The walk is frame-locked: each captured frame advances the simulation by exactly
//! `1 / WALK_FPS` of fixed steps, whatever the renderer's real frame rate. The
//! recording is therefore smooth and real-time on playback, and it is the same walk
//! the headless tour test takes, step for step.
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::composition::Vista;
use crate::view::Lab;
use observed_facility::hex_wfc::exposure::Form;

/// Long enough for pipelines to compile and shadow cascades to settle.
const WARM_UP: f32 = 5.0;
const SETTLE: f32 = 1.6;
/// Frames per second of simulated time in a walk recording.
pub const WALK_FPS: u32 = 30;
/// A second of stillness at each end of a walk, so the video does not start or stop
/// mid-stride.
const HOLD_FRAMES: u32 = WALK_FPS;
const MAX_FRAMES: u32 = 60 * WALK_FPS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Stills,
    Walk,
}

#[derive(Resource)]
pub struct Capture {
    dir: String,
    kind: Kind,
    started: bool,
    index: usize,
    frame: u32,
    /// Frames shot since the tour finished.
    tail: u32,
    timer: f32,
    finished: bool,
}

impl Capture {
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let (dir, kind) = if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE") {
            (dir, Kind::Stills)
        } else if let Ok(dir) = std::env::var("OBSERVED2_CAPTURE_WALK") {
            (dir, Kind::Walk)
        } else {
            return None;
        };
        std::fs::create_dir_all(&dir).expect("capture dir must be creatable");
        Some(Self {
            dir,
            kind,
            started: false,
            index: 0,
            frame: 0,
            tail: 0,
            timer: 0.0,
            finished: false,
        })
    }
}

impl Capture {
    /// Stills are shot at the lab's full window; a walk is hundreds of frames, so it
    /// is shot at 720p, which is what the evidence video is encoded at anyway.
    #[must_use]
    pub const fn window(&self) -> (u32, u32) {
        match self.kind {
            Kind::Stills => (1600, 900),
            Kind::Walk => (1280, 720),
        }
    }
}

fn shoot(commands: &mut Commands, path: String) {
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

pub fn progress(
    time: Res<Time>,
    mut run: ResMut<Capture>,
    mut lab: ResMut<Lab>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if run.finished {
        return;
    }
    run.timer += time.delta_secs();
    if !run.started {
        if run.kind == Kind::Stills
            && let Some(first) = Vista::vantages().first()
        {
            lab.take(first);
        }
        if run.timer < WARM_UP {
            return;
        }
        run.started = true;
        run.timer = 0.0;
        if run.kind == Kind::Walk {
            lab.start_tour();
            lab.manual_clock = true;
        }
        return;
    }
    match run.kind {
        Kind::Stills => stills(&mut run, &mut lab, &mut commands, &mut exit),
        Kind::Walk => walk(&mut run, &mut lab, &mut commands, &mut exit),
    }
}

fn stills(
    run: &mut Capture,
    lab: &mut Lab,
    commands: &mut Commands,
    exit: &mut MessageWriter<AppExit>,
) {
    let vantages = Vista::vantages();
    // Each vantage is two beats: settle then shoot, then give the save a moment.
    if let Some(vantage) = vantages.get(run.index) {
        lab.take(vantage);
        if run.timer >= SETTLE && run.frame == 0 {
            shoot(
                commands,
                format!(
                    "{}/vista_{:02}_{}.png",
                    run.dir,
                    run.index + 1,
                    vantage.slug
                ),
            );
            run.frame = 1;
        } else if run.timer >= SETTLE + 0.6 {
            run.index += 1;
            run.frame = 0;
            run.timer = 0.0;
        }
        return;
    }
    let count = |pred: fn(&Form) -> bool| lab.exposures.iter().filter(|e| pred(&e.form)).count();
    let manifest = serde_json::json!({
        "lab": "vista_lab",
        "vantages": vantages
            .iter()
            .enumerate()
            .map(|(i, v)| serde_json::json!({
                "file": format!("vista_{:02}_{}.png", i + 1, v.slug),
                "title": v.title,
                "standing": v.standing,
            }))
            .collect::<Vec<_>>(),
        "air_cells": lab.vista.air_cells,
        "built_cells": lab.exposures.len(),
        "sheer_faces": lab.exposures.iter().map(|e| e.sheer_count()).sum::<u32>(),
        "hanging_cells": lab.exposures.iter().filter(|e| e.overhang.hangs()).count(),
        "spans": count(|f| matches!(f, Form::Span { .. })),
        "flights": count(|f| matches!(f, Form::Flight { .. })),
        "pieces": lab.build.pieces.len(),
        "colliders": lab.build.pieces.iter().filter(|p| p.collides).count(),
    });
    std::fs::write(
        format!("{}/manifest.json", run.dir),
        serde_json::to_string_pretty(&manifest).expect("manifest serializes"),
    )
    .expect("manifest must be writable");
    run.finished = true;
    exit.write(AppExit::Success);
}

fn walk(
    run: &mut Capture,
    lab: &mut Lab,
    commands: &mut Commands,
    exit: &mut MessageWriter<AppExit>,
) {
    if run.frame >= MAX_FRAMES || run.tail > HOLD_FRAMES {
        run.finished = true;
        exit.write(AppExit::Success);
        return;
    }
    // Hold still for the opening second, then advance exactly one frame of fixed
    // steps per captured frame.
    if run.frame >= HOLD_FRAMES {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = (1.0 / (WALK_FPS as f32 * crate::walk::STEP)).round() as u32;
        for _ in 0..steps {
            if lab.touring {
                lab.walker.step_tour();
                lab.touring = !lab.walker.finished();
            }
        }
        if !lab.touring {
            run.tail += 1;
        }
    }
    shoot(commands, format!("{}/walk_{:04}.png", run.dir, run.frame));
    run.frame += 1;
}
