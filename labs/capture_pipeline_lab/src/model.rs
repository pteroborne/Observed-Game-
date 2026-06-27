use std::path::{Path, PathBuf};

pub const FIXED_DT_SECONDS: f32 = 1.0 / 10.0;
pub const LOOP_TICKS: u32 = 28;
pub const STILL_TICK: u32 = 14;
pub const SEQUENCE_TICKS: [u32; 6] = [0, 4, 8, 12, 16, 20];
pub const DEFAULT_CAPTURE_BASE_DIR: &str = "docs/evidence/capture_pipeline_lab";

const DOOR_START_TICK: u32 = 4;
const DOOR_END_TICK: u32 = 16;
const FLASH_START_TICK: u32 = 8;
const FLASH_PEAK_TICK: u32 = 14;
const FLASH_END_TICK: u32 = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapturePass {
    Still,
    Sequence,
}

impl CapturePass {
    pub const ALL: [CapturePass; 2] = [CapturePass::Still, CapturePass::Sequence];

    pub fn label(self) -> &'static str {
        match self {
            CapturePass::Still => "still",
            CapturePass::Sequence => "sequence",
        }
    }

    pub fn frame_count(self) -> usize {
        match self {
            CapturePass::Still => 1,
            CapturePass::Sequence => SEQUENCE_TICKS.len(),
        }
    }

    pub fn output_dir(self, base_dir: impl AsRef<Path>) -> PathBuf {
        base_dir.as_ref().join(self.label())
    }

    pub fn snapshot_for_frame(self, frame_index: usize) -> Option<CaptureSnapshot> {
        match self {
            CapturePass::Still => (frame_index == 0).then(|| snapshot_at(STILL_TICK)),
            CapturePass::Sequence => SEQUENCE_TICKS.get(frame_index).copied().map(snapshot_at),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureSnapshot {
    pub tick: u32,
    pub seconds: f32,
    pub door_open_fraction: f32,
    pub reroute_flash_alpha: f32,
    pub route_shift_fraction: f32,
    pub phase_label: &'static str,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CaptureTimeline {
    tick: u32,
}

impl CaptureTimeline {
    pub fn tick(self) -> u32 {
        self.tick
    }

    pub fn reset(&mut self) {
        self.tick = 0;
    }

    pub fn advance_fixed_tick(&mut self) {
        self.tick = if self.tick >= LOOP_TICKS {
            0
        } else {
            self.tick + 1
        };
    }

    pub fn snapshot(self) -> CaptureSnapshot {
        snapshot_at(self.tick)
    }
}

pub fn snapshot_at(tick: u32) -> CaptureSnapshot {
    let door_open_fraction = ramp(tick, DOOR_START_TICK, DOOR_END_TICK);
    let reroute_flash_alpha = triangle(tick, FLASH_START_TICK, FLASH_PEAK_TICK, FLASH_END_TICK);
    let route_shift_fraction = ramp(tick, FLASH_START_TICK, FLASH_END_TICK);
    let phase_label = if tick < DOOR_START_TICK {
        "closed"
    } else if tick < DOOR_END_TICK {
        "door opening"
    } else if tick <= FLASH_END_TICK {
        "reroute flash"
    } else {
        "settled"
    };

    CaptureSnapshot {
        tick,
        seconds: tick as f32 * FIXED_DT_SECONDS,
        door_open_fraction,
        reroute_flash_alpha,
        route_shift_fraction,
        phase_label,
    }
}

pub fn output_path_is_lab_scoped(base_dir: impl AsRef<Path>, path: impl AsRef<Path>) -> bool {
    path.as_ref().starts_with(base_dir)
}

fn ramp(tick: u32, start: u32, end: u32) -> f32 {
    if tick <= start {
        0.0
    } else if tick >= end {
        1.0
    } else {
        (tick - start) as f32 / (end - start) as f32
    }
}

fn triangle(tick: u32, start: u32, peak: u32, end: u32) -> f32 {
    if tick <= start || tick >= end {
        0.0
    } else if tick <= peak {
        (tick - start) as f32 / (peak - start) as f32
    } else {
        (end - tick) as f32 / (end - peak) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_samples_the_door_transition_and_flash() {
        let frames: Vec<CaptureSnapshot> = (0..CapturePass::Sequence.frame_count())
            .map(|index| CapturePass::Sequence.snapshot_for_frame(index).unwrap())
            .collect();

        assert_eq!(frames.first().unwrap().door_open_fraction, 0.0);
        assert_eq!(frames.last().unwrap().door_open_fraction, 1.0);
        assert!(
            frames.iter().any(|frame| frame.reroute_flash_alpha > 0.5),
            "the sequence includes the reroute flash peak"
        );
    }

    #[test]
    fn capture_sampling_does_not_advance_the_timeline() {
        let mut timeline = CaptureTimeline::default();
        for _ in 0..7 {
            timeline.advance_fixed_tick();
        }
        let before = timeline;

        for index in 0..CapturePass::Sequence.frame_count() {
            let _ = CapturePass::Sequence.snapshot_for_frame(index).unwrap();
        }

        assert_eq!(timeline, before);
    }

    #[test]
    fn output_directories_are_scoped_under_the_lab_evidence_root() {
        let root = Path::new(DEFAULT_CAPTURE_BASE_DIR);
        for pass in CapturePass::ALL {
            let dir = pass.output_dir(root);
            assert!(output_path_is_lab_scoped(root, &dir));
            assert!(dir.ends_with(pass.label()));
        }
    }

    #[test]
    fn timeline_loops_without_wall_clock_state() {
        let mut timeline = CaptureTimeline::default();
        for _ in 0..=LOOP_TICKS {
            timeline.advance_fixed_tick();
        }
        assert_eq!(timeline.tick(), 0);
        assert_eq!(timeline.snapshot().phase_label, "closed");
    }
}
