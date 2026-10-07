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
        if (origin - focus.position).length() > radius + 16.0 {
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
            p.distance(focus.position) <= 48.0
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
        // Walkable portals do not describe sight across windows, balconies,
        // open air, or stacked atria. Conservatively prepare exposed owners
        // in the view cone, even without a traversable connection to them.
        let exposed_view = cell.footprint.iter().any(|&at| {
            let centre = Vec3::from_array(hex_origin(at)) + Vec3::Y * 4.0;
            let delta = centre - focus.position;
            if delta.length() > radius + 16.0 {
                return false;
            }
            let forward = Vec3::new(
                focus.yaw.sin() * focus.pitch.cos(),
                focus.pitch.sin(),
                -focus.yaw.cos() * focus.pitch.cos(),
            );
            let in_view = delta.dot(forward) >= -16.0;
            in_view
                && HexFace::ALL.into_iter().any(|face| {
                    grid.neighbor(at, face).is_none_or(|next| {
                        world
                            .placements
                            .get(&next)
                            .is_none_or(|p| p.space == observed_facility::hex_wfc::HexSpace::Air)
                    })
                })
        });
        if near || next_to_visible || exposed_view {
            visible.insert(owner);
        }
    }
    visible
}

pub(super) fn spawn_priority(
    footprint: &[HexCoord],
    focus: &observed_match::hex_wfc::HexPlayerState,
    camera: Option<&Transform>,
) -> (bool, bool, u32) {
    let intended = Vec3::new(
        focus.yaw.sin() * focus.pitch.cos(),
        focus.pitch.sin(),
        -focus.yaw.cos() * focus.pitch.cos(),
    );
    let in_view = footprint.iter().any(|&cell| {
        let centre = Vec3::from_array(hex_origin(cell)) + Vec3::Y * 4.0;
        let delta = centre - focus.position;
        delta.dot(intended) + 16.0 >= delta.length() * 0.5
            || camera.is_some_and(|camera| {
                let delta = centre - camera.translation;
                delta.dot(camera.rotation * Vec3::NEG_Z) + 16.0 >= delta.length() * 0.5
            })
    });
    let distance = footprint
        .iter()
        .map(|&cell| {
            (Vec3::from_array(hex_origin(cell)) + Vec3::Y * 4.0 - focus.position).length_squared()
        })
        .fold(f32::INFINITY, f32::min);
    (
        !footprint.contains(&focus.cell),
        !in_view,
        (distance * 100.0) as u32,
    )
}

/// Drawing is independent of walkable portal reach. Keep a generous camera
/// cone, plus the nearby shadow neighbourhood, including the current
/// rendered gaze while a spectator's camera eases towards the body intent.
pub(super) fn draw_resident(
    footprint: &[HexCoord],
    focus: &observed_match::hex_wfc::HexPlayerState,
    camera: Option<&Transform>,
) -> bool {
    let intended = Vec3::new(
        focus.yaw.sin() * focus.pitch.cos(),
        focus.pitch.sin(),
        -focus.yaw.cos() * focus.pitch.cos(),
    );
    footprint.iter().any(|&cell| {
        if cell == focus.cell {
            return true;
        }
        let centre = Vec3::from_array(hex_origin(cell)) + Vec3::Y * 4.0;
        let delta = centre - focus.position;
        delta.length_squared() <= 48.0 * 48.0
            || delta.dot(intended) + 16.0 >= delta.length() * 0.42
            || camera.is_some_and(|camera| {
                let delta = centre - camera.translation;
                delta.dot(camera.rotation * Vec3::NEG_Z) + 16.0 >= delta.length() * 0.42
            })
    })
}

#[derive(PartialEq)]
struct WindowKey {
    generation: u32,
    cell: HexCoord,
    pose: [i32; 5],
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
                (p.position.y / 2.0).floor() as i32,
                (p.yaw / 0.15).floor() as i32,
                (p.pitch / 0.15).floor() as i32,
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
