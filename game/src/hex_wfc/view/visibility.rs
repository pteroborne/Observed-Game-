//! Conservative portal windows for presentation only. The match's observation
//! and collision world remain complete, including everything behind closed doors.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use bevy::prelude::*;
use observed_facility::hex_wfc::{HexCoord, HexFace};
use observed_hex::{PortClass, face_edge, hex_origin, ports_compatible};

use super::shell::HexGeometryCatalog;
use crate::hex_wfc::sim::HexWfcRuntime;

fn angle(point: Vec2, focus: Vec3, yaw: f32) -> f32 {
    let d = point - Vec2::new(focus.x, focus.z);
    let a = d.x.atan2(-d.y) - yaw;
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

pub(super) fn warm_cells(
    runtime: &HexWfcRuntime,
    catalog: &HexGeometryCatalog,
    radius: f32,
) -> BTreeSet<HexCoord> {
    let world = &runtime.match_state.facility;
    let focus = runtime.viewed();
    let owners: BTreeMap<_, _> = catalog
        .cells
        .iter()
        .flat_map(|(&owner, cell)| cell.footprint.iter().map(move |&at| (at, owner)))
        .collect();
    let grid = world.config.grid();
    let closed: BTreeSet<_> = runtime
        .match_state
        .doors()
        .filter(|door| door.closed)
        .flat_map(|door| {
            [
                (door.cell, door.face),
                (
                    grid.neighbor(door.cell, door.face).unwrap_or(door.cell),
                    door.face.opposite(),
                ),
            ]
        })
        .collect();
    let mut visible = BTreeSet::new();
    let mut visited = BTreeMap::<HexCoord, (f32, f32)>::new();
    let mut queue = VecDeque::from([(focus.cell, -2.2_f32, 2.2_f32)]);
    while let Some((at, left, right)) = queue.pop_front() {
        let origin = Vec3::from_array(hex_origin(at));
        if (origin - focus.position).with_y(0.0).length() > radius + 8.0 {
            continue;
        }
        if visited
            .get(&at)
            .is_some_and(|&(lo, hi)| lo <= left && hi >= right)
        {
            continue;
        }
        visited
            .entry(at)
            .and_modify(|v| {
                v.0 = v.0.min(left);
                v.1 = v.1.max(right);
            })
            .or_insert((left, right));
        if let Some(&owner) = owners.get(&at) {
            visible.insert(owner);
        }
        let Some(placement) = world.placements.get(&at) else {
            continue;
        };
        for face in HexFace::ALL {
            let Some(next) = grid.neighbor(at, face) else {
                continue;
            };
            let Some(neighbor) = world.placements.get(&next).filter(|p| p.space.built()) else {
                continue;
            };
            let internal = owners
                .get(&at)
                .is_some_and(|owner| owners.get(&next) == Some(owner));
            let a = placement.ports().port(face);
            let b = neighbor.ports().port(face.opposite());
            if !internal
                && (a == PortClass::Sealed
                    || b == PortClass::Sealed
                    || !ports_compatible(a, b)
                    || closed.contains(&(at, face)))
            {
                continue;
            }
            let (mut lo, mut hi) = (left, right);
            if face.is_lateral() && !internal && origin.distance(focus.position) > 16.0 {
                let [a, b] = face_edge(face);
                let mut angles = [
                    angle(
                        Vec2::new(origin.x + a.0 as f32, origin.z + a.1 as f32),
                        focus.position,
                        focus.yaw,
                    ),
                    angle(
                        Vec2::new(origin.x + b.0 as f32, origin.z + b.1 as f32),
                        focus.position,
                        focus.yaw,
                    ),
                ];
                angles.sort_by(f32::total_cmp);
                if angles[1] - angles[0] < std::f32::consts::PI {
                    lo = lo.max(angles[0] - 0.10);
                    hi = hi.min(angles[1] + 0.10);
                }
            }
            if lo <= hi {
                queue.push_back((next, lo, hi));
            }
        }
    }
    // A shadow-support neighbourhood and one-cell corner prefetch prevent a
    // camera turn from revealing an unprepared adjacent room.
    let reached = visible.clone();
    for (&owner, cell) in &catalog.cells {
        let near = cell.footprint.iter().any(|&at| {
            let p = Vec3::from_array(hex_origin(at));
            at.level.abs_diff(focus.cell.level) <= 1 && p.distance(focus.position) <= 30.0
        });
        let next_to_visible = cell.footprint.iter().any(|&at| {
            HexFace::ALL
                .into_iter()
                .filter_map(|face| grid.neighbor(at, face))
                .any(|next| {
                    owners
                        .get(&next)
                        .is_some_and(|owner| reached.contains(owner))
                })
        });
        if near || next_to_visible {
            visible.insert(owner);
        }
    }
    visible
}

#[derive(PartialEq)]
struct WindowKey {
    generation: u32,
    cell: HexCoord,
    pose: [i32; 3],
    radius: u32,
    doors: Vec<(HexCoord, HexFace)>,
}
#[derive(Default)]
pub(super) struct Window {
    key: Option<WindowKey>,
    warm: BTreeSet<HexCoord>,
}
impl Window {
    pub(super) fn update(
        &mut self,
        runtime: &HexWfcRuntime,
        catalog: &HexGeometryCatalog,
        radius: f32,
    ) -> &BTreeSet<HexCoord> {
        let p = runtime.viewed();
        let key = WindowKey {
            generation: catalog.generation,
            cell: p.cell,
            pose: [
                (p.position.x / 2.0).floor() as i32,
                (p.position.z / 2.0).floor() as i32,
                (p.yaw / 0.15).floor() as i32,
            ],
            radius: radius.to_bits(),
            doors: runtime
                .match_state
                .doors()
                .filter(|d| d.closed)
                .map(|d| (d.cell, d.face))
                .collect(),
        };
        if self.key.as_ref() != Some(&key) {
            self.warm = warm_cells(runtime, catalog, radius);
            self.key = Some(key);
        }
        &self.warm
    }
}

#[cfg(test)]
mod tests;
