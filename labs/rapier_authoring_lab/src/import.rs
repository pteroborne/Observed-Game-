//! Pure TrenchBroom-data projection into stable semantics and convex collision hulls.

use bevy::math::Vec3;
use bevy_trenchbroom::{
    brush::{Brush, ConvexHull},
    config::TrenchBroomConfig,
    qmap::{QuakeMapEntities, QuakeMapEntity},
};
use observed_core::{PortId, RoomId};

use crate::map_source;

#[derive(Clone, Debug, PartialEq)]
pub struct ConvexBrush {
    pub points: Vec<[f32; 3]>,
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    Room,
    Corridor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomInfo {
    pub id: RoomId,
    pub kind: RoomKind,
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortInfo {
    pub id: PortId,
    pub a: RoomId,
    pub b: RoomId,
    pub position: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorState {
    Open,
    Closed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DoorInfo {
    pub id: u32,
    pub port: PortId,
    pub state: DoorState,
    pub hull: ConvexBrush,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Facility {
    pub structural: Vec<ConvexBrush>,
    pub rooms: Vec<RoomInfo>,
    pub ports: Vec<PortInfo>,
    pub doors: Vec<DoorInfo>,
    pub spawn_ground: Vec3,
}

fn prop(entity: &QuakeMapEntity, key: &str) -> Option<String> {
    entity.properties.get(key).cloned()
}

fn prop_u32(entity: &QuakeMapEntity, key: &str) -> Option<u32> {
    prop(entity, key)?.parse().ok()
}

fn point(entity: &QuakeMapEntity, key: &str, config: &TrenchBroomConfig) -> Option<Vec3> {
    let values = prop(entity, key)?
        .split_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == 3).then(|| config.to_bevy_space(Vec3::from_slice(&values)))
}

pub fn convex_brush(brush: &Brush) -> Option<ConvexBrush> {
    let mut points = Vec::<[f32; 3]>::new();
    for (vertex, _) in brush.calculate_vertices() {
        let point = vertex.as_vec3().to_array();
        if !points.iter().any(|other| {
            other[0].to_bits() == point[0].to_bits()
                && other[1].to_bits() == point[1].to_bits()
                && other[2].to_bits() == point[2].to_bits()
        }) {
            points.push(point);
        }
    }
    if points.len() < 4 {
        return None;
    }
    points.sort_by_key(|p| (p[0].to_bits(), p[1].to_bits(), p[2].to_bits()));
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for point in &points {
        let point = Vec3::from_array(*point);
        min = min.min(point);
        max = max.max(point);
    }
    Some(ConvexBrush { points, min, max })
}

pub fn project(entities: &QuakeMapEntities, config: &TrenchBroomConfig) -> Facility {
    let mut facility = Facility {
        structural: Vec::new(),
        rooms: Vec::new(),
        ports: Vec::new(),
        doors: Vec::new(),
        spawn_ground: Vec3::ZERO,
    };

    for entity in entities.iter() {
        match entity.classname().unwrap_or("") {
            "worldspawn" => {
                facility
                    .structural
                    .extend(entity.brushes.iter().filter_map(convex_brush));
            }
            "info_player_start" => {
                facility.spawn_ground = point(entity, "origin", config).unwrap_or(Vec3::ZERO);
                facility.spawn_ground.y = 0.0;
            }
            "observed_room" => {
                let a = point(entity, "mins", config).unwrap_or(Vec3::ZERO);
                let b = point(entity, "maxs", config).unwrap_or(Vec3::ZERO);
                facility.rooms.push(RoomInfo {
                    id: RoomId(prop_u32(entity, "id").unwrap_or(0)),
                    kind: if prop(entity, "kind").as_deref() == Some("corridor") {
                        RoomKind::Corridor
                    } else {
                        RoomKind::Room
                    },
                    min: a.min(b),
                    max: a.max(b),
                });
            }
            "observed_port" => facility.ports.push(PortInfo {
                id: PortId(prop_u32(entity, "id").unwrap_or(0)),
                a: RoomId(prop_u32(entity, "room_a").unwrap_or(0)),
                b: RoomId(prop_u32(entity, "room_b").unwrap_or(0)),
                position: point(entity, "origin", config).unwrap_or(Vec3::ZERO),
            }),
            "observed_door" => {
                if let Some(hull) = entity.brushes.first().and_then(convex_brush) {
                    facility.doors.push(DoorInfo {
                        id: prop_u32(entity, "id").unwrap_or(0),
                        port: PortId(prop_u32(entity, "port").unwrap_or(0)),
                        state: if prop(entity, "state").as_deref() == Some("closed") {
                            DoorState::Closed
                        } else {
                            DoorState::Open
                        },
                        hull,
                    });
                }
            }
            _ => {}
        }
    }
    facility.rooms.sort_by_key(|room| room.id);
    facility.ports.sort_by_key(|port| port.id);
    facility.doors.sort_by_key(|door| door.id);
    facility
}

pub fn parse_course() -> Facility {
    let config = TrenchBroomConfig::new("observed2_rapier_authoring_lab");
    let parsed = quake_map::parse(&mut std::io::Cursor::new(map_source::course_map()))
        .expect("typed Rapier authoring course must parse");
    let entities = QuakeMapEntities::from_quake_map(parsed, &config);
    project(&entities, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_semantics_without_editor_entities_becoming_identity() {
        let facility = parse_course();
        assert_eq!(facility.rooms.len(), 3);
        assert_eq!(facility.ports.len(), 2);
        assert_eq!(facility.ports[1].id, PortId(2));
        assert_eq!(facility.ports[1].a, RoomId(2));
        assert_eq!(facility.ports[1].b, RoomId(3));
        assert_eq!(facility.doors[0].state, DoorState::Closed);
    }

    #[test]
    fn ramp_remains_a_non_box_convex_brush() {
        let facility = parse_course();
        let summary: Vec<_> = facility
            .structural
            .iter()
            .map(|brush| (brush.points.len(), brush.min, brush.max))
            .collect();
        let ramp = facility
            .structural
            .iter()
            .find(|brush| brush.points.len() == 6)
            .unwrap_or_else(|| panic!("triangular-prism ramp missing: {summary:#?}"));
        assert!(ramp.max.y - ramp.min.y > 2.0);
        assert!(ramp.max.z - ramp.min.z > 4.0);
    }

    #[test]
    fn every_imported_brush_builds_a_real_rapier_convex_hull() {
        use rapier3d::prelude::{ColliderBuilder, Vector};

        let facility = parse_course();
        for brush in facility
            .structural
            .iter()
            .chain(facility.doors.iter().map(|door| &door.hull))
        {
            let points: Vec<Vector> = brush.points.iter().map(|p| Vector::from(*p)).collect();
            assert!(ColliderBuilder::convex_hull(&points).is_some());
        }
    }
}
