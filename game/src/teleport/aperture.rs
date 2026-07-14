//! Pure boundary-wall planning shared by rendering, collision, and validation.
//!
//! A place boundary is a closed polygon in XZ. Door gaps become rectangular
//! apertures in an edge-local `(along, height)` plane. Partitioning that plane
//! once avoids the historical family of bugs where one consumer recognized only
//! a gap at an edge midpoint while another projected arbitrary or elevated gaps.

use bevy::math::Vec2;

use super::{DoorGap, GapKind};

const EDGE_TOLERANCE: f32 = 0.08;
const NORMAL_ALIGNMENT_MIN: f32 = 0.7;
const MIN_PANEL_SIZE: f32 = 0.01;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThresholdClosure {
    Closed,
    Locked,
    Collapsed,
}

impl ThresholdClosure {
    pub fn for_kind(kind: GapKind) -> Option<Self> {
        match kind {
            GapKind::Side | GapKind::OneWayEntry => Some(Self::Closed),
            GapKind::LockedExit => Some(Self::Locked),
            GapKind::Collapsed => Some(Self::Collapsed),
            GapKind::Forward | GapKind::Entry | GapKind::Exit | GapKind::OneWayExit => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PlannedAperture {
    pub gap_index: usize,
    pub edge_index: usize,
    pub start: Vec2,
    pub end: Vec2,
    pub y_min: f32,
    pub y_max: f32,
    pub closure: Option<ThresholdClosure>,
}

#[derive(Clone, Copy, Debug)]
pub struct WallPanel {
    pub edge_index: usize,
    pub start: Vec2,
    pub end: Vec2,
    pub y_min: f32,
    pub y_max: f32,
}

#[derive(Clone, Debug, Default)]
pub struct AperturePlan {
    pub wall_panels: Vec<WallPanel>,
    pub apertures: Vec<PlannedAperture>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AperturePlanError {
    InvalidBoundary,
    InvalidGap { gap_index: usize },
    GapNotOnBoundary { gap_index: usize },
    OverlappingGaps { first: usize, second: usize },
}

#[derive(Clone, Copy)]
struct EdgeGap {
    gap_index: usize,
    along_min: f32,
    along_max: f32,
    y_min: f32,
    y_max: f32,
    closure: Option<ThresholdClosure>,
}

/// Partition a place boundary into solid wall panels and threshold apertures.
///
/// Every gap, including a sealed/locked/collapsed one, cuts a real architectural
/// opening. Non-passable gaps carry a closure that collision and presentation add
/// back explicitly. `opening_height` is the clear height above each gap's floor.
pub fn plan_boundary(
    boundary: &[Vec2],
    gaps: &[DoorGap],
    total_height: f32,
    opening_height: f32,
) -> Result<AperturePlan, AperturePlanError> {
    if boundary.len() < 3
        || !total_height.is_finite()
        || !opening_height.is_finite()
        || total_height <= 0.0
        || opening_height <= 0.0
    {
        return Err(AperturePlanError::InvalidBoundary);
    }

    let mut edge_gaps = vec![Vec::<EdgeGap>::new(); boundary.len()];
    for (gap_index, gap) in gaps.iter().enumerate() {
        if !gap.center.is_finite()
            || !gap.normal.is_finite()
            || !gap.width.is_finite()
            || !gap.floor_y.is_finite()
            || gap.width <= 0.0
            || gap.floor_y < -EDGE_TOLERANCE
            || gap.floor_y >= total_height
        {
            return Err(AperturePlanError::InvalidGap { gap_index });
        }

        let mut owner = None;
        for edge_index in 0..boundary.len() {
            let start = boundary[edge_index];
            let end = boundary[(edge_index + 1) % boundary.len()];
            let edge = end - start;
            let length = edge.length();
            if length <= MIN_PANEL_SIZE {
                continue;
            }
            let tangent = edge / length;
            let outward = super::geom::outward_normal(start, end);
            let relative = gap.center - start;
            let along = relative.dot(tangent);
            let off_edge = (relative - tangent * along).length();
            if off_edge <= EDGE_TOLERANCE
                && gap.normal.dot(outward) >= NORMAL_ALIGNMENT_MIN
                && along >= -EDGE_TOLERANCE
                && along <= length + EDGE_TOLERANCE
            {
                owner = Some((edge_index, along.clamp(0.0, length), length));
                break;
            }
        }
        let Some((edge_index, along, edge_length)) = owner else {
            return Err(AperturePlanError::GapNotOnBoundary { gap_index });
        };
        let along_min = (along - gap.width * 0.5).clamp(0.0, edge_length);
        let along_max = (along + gap.width * 0.5).clamp(0.0, edge_length);
        let y_min = gap.floor_y.max(0.0);
        let y_max = (gap.floor_y + opening_height).min(total_height);
        if along_max - along_min <= MIN_PANEL_SIZE || y_max - y_min <= MIN_PANEL_SIZE {
            return Err(AperturePlanError::InvalidGap { gap_index });
        }
        edge_gaps[edge_index].push(EdgeGap {
            gap_index,
            along_min,
            along_max,
            y_min,
            y_max,
            closure: ThresholdClosure::for_kind(gap.kind),
        });
    }

    let mut plan = AperturePlan::default();
    for (edge_index, owned) in edge_gaps.iter_mut().enumerate() {
        let start = boundary[edge_index];
        let end = boundary[(edge_index + 1) % boundary.len()];
        let edge = end - start;
        let edge_length = edge.length();
        if edge_length <= MIN_PANEL_SIZE {
            continue;
        }
        let tangent = edge / edge_length;
        owned.sort_by(|a, b| {
            a.along_min
                .total_cmp(&b.along_min)
                .then(a.y_min.total_cmp(&b.y_min))
                .then(a.gap_index.cmp(&b.gap_index))
        });
        for (index, a) in owned.iter().enumerate() {
            for b in owned.iter().skip(index + 1) {
                let overlap_x = a.along_min < b.along_max - MIN_PANEL_SIZE
                    && b.along_min < a.along_max - MIN_PANEL_SIZE;
                let overlap_y =
                    a.y_min < b.y_max - MIN_PANEL_SIZE && b.y_min < a.y_max - MIN_PANEL_SIZE;
                if overlap_x && overlap_y {
                    return Err(AperturePlanError::OverlappingGaps {
                        first: a.gap_index,
                        second: b.gap_index,
                    });
                }
            }
        }

        for gap in owned.iter() {
            plan.apertures.push(PlannedAperture {
                gap_index: gap.gap_index,
                edge_index,
                start: start + tangent * gap.along_min,
                end: start + tangent * gap.along_max,
                y_min: gap.y_min,
                y_max: gap.y_max,
                closure: gap.closure,
            });
        }

        let mut xs = vec![0.0, edge_length];
        let mut ys = vec![0.0, total_height];
        for gap in owned.iter() {
            xs.extend([gap.along_min, gap.along_max]);
            ys.extend([gap.y_min, gap.y_max]);
        }
        sort_dedup(&mut xs);
        sort_dedup(&mut ys);

        for y_window in ys.windows(2) {
            let y_min = y_window[0];
            let y_max = y_window[1];
            if y_max - y_min <= MIN_PANEL_SIZE {
                continue;
            }
            let y_mid = (y_min + y_max) * 0.5;
            let mut run_start: Option<f32> = None;
            for x_window in xs.windows(2) {
                let x_min = x_window[0];
                let x_max = x_window[1];
                if x_max - x_min <= MIN_PANEL_SIZE {
                    continue;
                }
                let x_mid = (x_min + x_max) * 0.5;
                let inside_aperture = owned.iter().any(|gap| {
                    x_mid > gap.along_min + MIN_PANEL_SIZE
                        && x_mid < gap.along_max - MIN_PANEL_SIZE
                        && y_mid > gap.y_min + MIN_PANEL_SIZE
                        && y_mid < gap.y_max - MIN_PANEL_SIZE
                });
                match (inside_aperture, run_start) {
                    (false, None) => run_start = Some(x_min),
                    (true, Some(run)) => {
                        push_panel(
                            &mut plan.wall_panels,
                            WallPanel {
                                edge_index,
                                start: start + tangent * run,
                                end: start + tangent * x_min,
                                y_min,
                                y_max,
                            },
                        );
                        run_start = None;
                    }
                    _ => {}
                }
            }
            if let Some(run) = run_start {
                push_panel(
                    &mut plan.wall_panels,
                    WallPanel {
                        edge_index,
                        start: start + tangent * run,
                        end: start + tangent * edge_length,
                        y_min,
                        y_max,
                    },
                );
            }
        }
    }
    plan.apertures.sort_by_key(|aperture| aperture.gap_index);
    Ok(plan)
}

fn push_panel(panels: &mut Vec<WallPanel>, panel: WallPanel) {
    if panel.end.distance(panel.start) > MIN_PANEL_SIZE
        && panel.y_max - panel.y_min > MIN_PANEL_SIZE
    {
        panels.push(panel);
    }
}

fn sort_dedup(values: &mut Vec<f32>) {
    values.sort_by(|a, b| a.total_cmp(b));
    values.dedup_by(|a, b| (*a - *b).abs() <= MIN_PANEL_SIZE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use observed_core::{CorridorId, RoomId, ThresholdSlotId};

    use crate::teleport::{HallThreshold, RoomThreshold, ThresholdLink, ThresholdLocalSide};

    fn gap(center: Vec2, floor_y: f32, kind: GapKind, slot: u16) -> DoorGap {
        DoorGap {
            center,
            normal: Vec2::Y,
            width: 2.0,
            target: RoomId(slot as u32 + 1),
            kind,
            threshold: ThresholdLink {
                room: RoomThreshold {
                    room: RoomId(0),
                    slot: ThresholdSlotId(slot),
                },
                hall: HallThreshold {
                    corridor: CorridorId(0),
                    slot: ThresholdSlotId(slot),
                },
                local_side: ThresholdLocalSide::Room,
            },
            floor_y,
        }
    }

    fn square() -> Vec<Vec2> {
        vec![
            Vec2::new(-5.0, -5.0),
            Vec2::new(5.0, -5.0),
            Vec2::new(5.0, 5.0),
            Vec2::new(-5.0, 5.0),
        ]
    }

    #[test]
    fn off_center_and_elevated_apertures_partition_one_wall() {
        let gaps = [
            gap(Vec2::new(-2.5, 5.0), 0.0, GapKind::Exit, 0),
            gap(Vec2::new(2.5, 5.0), 3.5, GapKind::Exit, 1),
        ];
        let plan = plan_boundary(&square(), &gaps, 7.0, 3.0).unwrap();
        assert_eq!(plan.apertures.len(), 2);
        assert!(plan.apertures.iter().all(|opening| opening.edge_index == 2));
        assert!(plan.wall_panels.iter().all(|panel| {
            let center = (panel.start + panel.end) * 0.5;
            let y = (panel.y_min + panel.y_max) * 0.5;
            !gaps.iter().any(|gap| {
                center.distance(gap.center) < gap.width * 0.5
                    && y > gap.floor_y
                    && y < gap.floor_y + 3.0
            })
        }));
    }

    #[test]
    fn closed_threshold_is_an_opening_with_an_explicit_closure() {
        let plan = plan_boundary(
            &square(),
            &[gap(Vec2::new(0.0, 5.0), 0.0, GapKind::Collapsed, 0)],
            3.4,
            3.4,
        )
        .unwrap();
        assert_eq!(plan.apertures.len(), 1);
        assert_eq!(plan.apertures[0].closure, Some(ThresholdClosure::Collapsed));
    }

    #[test]
    fn overlapping_thresholds_fail_closed() {
        let result = plan_boundary(
            &square(),
            &[
                gap(Vec2::new(-0.4, 5.0), 0.0, GapKind::Exit, 0),
                gap(Vec2::new(0.4, 5.0), 0.0, GapKind::Exit, 1),
            ],
            3.4,
            3.4,
        );
        assert!(matches!(
            result,
            Err(AperturePlanError::OverlappingGaps { .. })
        ));
    }
}
