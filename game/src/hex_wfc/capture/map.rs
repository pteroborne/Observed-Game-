//! Capture the survivor map after exploration or completion. A frame override
//! supports quick renderer checks without waiting through a whole bot playthrough.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum MapBeat {
    Wait,
    Open,
    Shot,
    Exit,
}

pub(super) struct MapCapture {
    opened_at: Option<u16>,
    frame_limit: u16,
}
impl Default for MapCapture {
    fn default() -> Self {
        Self {
            opened_at: None,
            frame_limit: std::env::var("OBSERVED2_CAPTURE_HEX_WFC_MAP_FRAME")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(7_200),
        }
    }
}
impl MapCapture {
    pub(super) fn beat(&mut self, frame: u16, finished: bool) -> MapBeat {
        if self.opened_at.is_none() && (frame >= self.frame_limit || finished) {
            self.opened_at = Some(frame);
            return MapBeat::Open;
        }
        match self.opened_at.map(|start| frame.saturating_sub(start)) {
            Some(60) => MapBeat::Shot,
            Some(130) => MapBeat::Exit,
            _ => MapBeat::Wait,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{MapBeat, MapCapture};
    #[test]
    fn completion_and_a_frame_limit_each_capture_the_map_and_exit() {
        for (limit, finished) in [(7_200, true), (101, false)] {
            let mut capture = MapCapture {
                opened_at: None,
                frame_limit: limit,
            };
            assert_eq!(capture.beat(100, false), MapBeat::Wait);
            assert_eq!(capture.beat(101, finished), MapBeat::Open);
            assert_eq!(capture.beat(161, finished), MapBeat::Shot);
            assert_eq!(capture.beat(231, finished), MapBeat::Exit);
        }
    }
}
