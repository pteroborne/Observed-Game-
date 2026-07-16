use super::{CellCoord, DEFAULT_PULSE_TICKS, FullWfcConfig, FullWfcError, ModuleFace};

#[derive(Clone, Copy, Debug)]
pub(super) struct AttemptRange {
    pub start: u32,
    pub budget: u32,
}

pub(super) fn validate_config(config: FullWfcConfig) -> Result<(), FullWfcError> {
    if config.cols < 2
        || config.rows < 2
        || config.levels < 2
        || config.min_rooms < 2
        || config.min_rooms > config.max_rooms
        || config.max_rooms >= config.cell_count()
        || config.retry_budget == 0
        || config.pulse_ticks == 0
        || config.min_room_distance == 0
    {
        return Err(FullWfcError::InvalidConfig);
    }
    Ok(())
}

impl Default for FullWfcConfig {
    fn default() -> Self {
        Self {
            cols: 8,
            rows: 5,
            levels: 3,
            min_rooms: 24,
            max_rooms: 32,
            retry_budget: 64,
            pulse_ticks: DEFAULT_PULSE_TICKS,
            min_room_distance: 1,
        }
    }
}

impl FullWfcConfig {
    pub fn liminal_large() -> Self {
        Self {
            cols: 20,
            rows: 12,
            levels: 5,
            min_rooms: 24,
            max_rooms: 36,
            retry_budget: 128,
            pulse_ticks: DEFAULT_PULSE_TICKS,
            min_room_distance: 4,
        }
    }

    pub fn cell_count(self) -> usize {
        usize::from(self.cols) * usize::from(self.rows) * usize::from(self.levels)
    }

    pub fn spawn(self) -> CellCoord {
        CellCoord::new(0, 0, 0)
    }

    pub fn exit(self) -> CellCoord {
        CellCoord::new(self.cols - 1, self.rows - 1, self.levels - 1)
    }

    pub(super) fn contains(self, coord: CellCoord) -> bool {
        coord.x < self.cols && coord.z < self.rows && coord.level < self.levels
    }

    pub(super) fn index(self, coord: CellCoord) -> usize {
        usize::from(coord.level) * usize::from(self.cols) * usize::from(self.rows)
            + usize::from(coord.z) * usize::from(self.cols)
            + usize::from(coord.x)
    }

    pub(super) fn coord(self, index: usize) -> CellCoord {
        let plane = usize::from(self.cols) * usize::from(self.rows);
        let level = index / plane;
        let within = index % plane;
        CellCoord::new(
            (within % usize::from(self.cols)) as u16,
            (within / usize::from(self.cols)) as u16,
            level as u8,
        )
    }

    pub fn neighbor(self, coord: CellCoord, face: ModuleFace) -> Option<CellCoord> {
        let (dx, dz, dl) = face.delta();
        let next = CellCoord::new(
            u16::try_from(i32::from(coord.x) + i32::from(dx)).ok()?,
            u16::try_from(i32::from(coord.z) + i32::from(dz)).ok()?,
            u8::try_from(i16::from(coord.level) + i16::from(dl)).ok()?,
        );
        self.contains(next).then_some(next)
    }
}
