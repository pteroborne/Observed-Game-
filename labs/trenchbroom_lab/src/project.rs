//! The **projection**: imported TrenchBroom map data → the game's domain model.
//!
//! This is the architectural heart of Phase A1 and answers its question — *can an
//! authored 3D map become the room/corridor/door topology without editor entities
//! becoming the game model?* It is pure logic over `bevy_trenchbroom`'s parsed
//! [`QuakeMapEntities`] (brush geometry + entity properties): no rendering, no async
//! asset server, no `Entity` values. It yields stable [`RoomId`]/[`PortId`]s, door
//! state, and collision [`Aabb3`]s that the presentation layer then *projects* — and
//! that the tests verify headlessly.
//!
//! Brush AABBs come straight from the importer's `as_cuboid` (already converted to
//! Bevy space on load); point-entity positions are converted here with the same
//! config `to_bevy_space`, so the projected collision lines up with the geometry the
//! importer would render.

use bevy::math::Vec3;
use bevy_trenchbroom::brush::{Brush, ConvexHull};
use bevy_trenchbroom::config::TrenchBroomConfig;
use bevy_trenchbroom::qmap::{QuakeMapEntities, QuakeMapEntity};
use observed_core::{PortId, RoomId};
use observed_traversal::Aabb3;

/// What a room marker is for. The corridor connects the two rooms.
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

impl RoomInfo {
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
}

/// A typed connection between two rooms (the authored "port"/socket).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortInfo {
    pub id: PortId,
    pub a: RoomId,
    pub b: RoomId,
    pub pos: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorState {
    Open,
    Closed,
}

/// A door leaf: imported geometry whose *collision presence* is gated by the door
/// model state, never by the map material. Closed → its [`Aabb3`] blocks the gap;
/// open → it is absent and the threshold is passable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoorInfo {
    pub id: u32,
    pub port: PortId,
    pub state: DoorState,
    pub leaf: Aabb3,
}

/// The whole facility projected out of one authored map.
#[derive(Clone, Debug, PartialEq)]
pub struct Facility {
    pub rooms: Vec<RoomInfo>,
    pub ports: Vec<PortInfo>,
    pub doors: Vec<DoorInfo>,
    /// Always-collidable structural geometry (walls, floor, stairs, platform).
    pub solids: Vec<Aabb3>,
    /// Bevy-space ground position of the player spawn (feet at `floor_y`).
    pub spawn_ground: Vec3,
    pub spawn_yaw: f32,
    pub floor_y: f32,
    /// Half-extent that bounds the facility (respawn guard + debug grid).
    pub floor_half: f32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
}

impl Facility {
    pub fn room(&self, id: RoomId) -> Option<&RoomInfo> {
        self.rooms.iter().find(|r| r.id == id)
    }

    /// The collision set for the controller: structural solids plus the leaves of
    /// every door that is currently closed.
    pub fn collision_solids(&self) -> Vec<Aabb3> {
        let mut out = self.solids.clone();
        for d in &self.doors {
            if d.state == DoorState::Closed {
                out.push(d.leaf);
            }
        }
        out
    }
}

/// Component-wise AABB of a brush. Prefers the importer's `as_cuboid` (exact for the
/// authored boxes); falls back to the convex hull's vertices for any non-box brush,
/// so the projection never depends on a particular authoring style.
pub fn brush_aabb(brush: &Brush) -> Option<Aabb3> {
    if let Some((from, to)) = brush.as_cuboid() {
        let a = from.as_vec3();
        let b = to.as_vec3();
        return Some(Aabb3 {
            min: a.min(b),
            max: a.max(b),
        });
    }
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;
    for (vertex, _) in brush.calculate_vertices() {
        let v = vertex.as_vec3();
        min = min.min(v);
        max = max.max(v);
        any = true;
    }
    any.then_some(Aabb3 { min, max })
}

fn prop(entity: &QuakeMapEntity, key: &str) -> Option<String> {
    entity.properties.get(key).cloned()
}

fn prop_u32(entity: &QuakeMapEntity, key: &str) -> Option<u32> {
    prop(entity, key)?.trim().parse().ok()
}

/// Parse an `"x y z"` property as a TrenchBroom-space point and convert to Bevy space.
fn prop_point_bevy(entity: &QuakeMapEntity, key: &str, config: &TrenchBroomConfig) -> Option<Vec3> {
    let raw = prop(entity, key)?;
    let parts: Vec<f32> = raw
        .split_whitespace()
        .map(|p| p.parse().ok())
        .collect::<Option<Vec<f32>>>()?;
    if parts.len() != 3 {
        return None;
    }
    Some(config.to_bevy_space(Vec3::new(parts[0], parts[1], parts[2])))
}

/// Project the parsed map into the domain [`Facility`].
pub fn project(entities: &QuakeMapEntities, config: &TrenchBroomConfig) -> Facility {
    let mut solids = Vec::new();
    let mut rooms = Vec::new();
    let mut ports = Vec::new();
    let mut doors = Vec::new();
    let mut spawn_ground = Vec3::ZERO;
    let mut spawn_yaw = 0.0;

    for entity in entities.iter() {
        let classname = entity.classname().unwrap_or("");
        match classname {
            "worldspawn" => {
                for brush in &entity.brushes {
                    if let Some(aabb) = brush_aabb(brush) {
                        solids.push(aabb);
                    }
                }
            }
            "info_player_start" => {
                if let Some(p) = prop_point_bevy(entity, "origin", config) {
                    spawn_ground = p;
                }
                spawn_yaw = prop(entity, "yaw")
                    .and_then(|s| s.trim().parse::<f32>().ok())
                    .map(f32::to_radians)
                    .unwrap_or(0.0);
            }
            "observed_room" => {
                let id = RoomId(prop_u32(entity, "id").unwrap_or(0));
                let kind = match prop(entity, "kind").as_deref() {
                    Some("corridor") => RoomKind::Corridor,
                    _ => RoomKind::Room,
                };
                let mins = prop_point_bevy(entity, "mins", config).unwrap_or(Vec3::ZERO);
                let maxs = prop_point_bevy(entity, "maxs", config).unwrap_or(Vec3::ZERO);
                rooms.push(RoomInfo {
                    id,
                    kind,
                    min: mins.min(maxs),
                    max: mins.max(maxs),
                });
            }
            "observed_port" => {
                ports.push(PortInfo {
                    id: PortId(prop_u32(entity, "id").unwrap_or(0)),
                    a: RoomId(prop_u32(entity, "room_a").unwrap_or(0)),
                    b: RoomId(prop_u32(entity, "room_b").unwrap_or(0)),
                    pos: prop_point_bevy(entity, "origin", config).unwrap_or(Vec3::ZERO),
                });
            }
            "observed_door" => {
                let leaf = entity.brushes.first().and_then(brush_aabb);
                if let Some(leaf) = leaf {
                    let state = match prop(entity, "state").as_deref() {
                        Some("closed") => DoorState::Closed,
                        _ => DoorState::Open,
                    };
                    doors.push(DoorInfo {
                        id: prop_u32(entity, "id").unwrap_or(0),
                        port: PortId(prop_u32(entity, "port").unwrap_or(0)),
                        state,
                        leaf,
                    });
                }
            }
            _ => {}
        }
    }

    rooms.sort_by_key(|r| r.id);
    ports.sort_by_key(|p| p.id);
    doors.sort_by_key(|d| d.id);

    // Bounds over all imported geometry (structural + door leaves).
    let mut bounds_min = Vec3::splat(f32::INFINITY);
    let mut bounds_max = Vec3::splat(f32::NEG_INFINITY);
    for s in solids.iter().chain(doors.iter().map(|d| &d.leaf)) {
        bounds_min = bounds_min.min(s.min);
        bounds_max = bounds_max.max(s.max);
    }
    if !bounds_min.is_finite() {
        bounds_min = Vec3::ZERO;
        bounds_max = Vec3::ZERO;
    }

    let floor_y = 0.0;
    let floor_half = bounds_min
        .x
        .abs()
        .max(bounds_max.x.abs())
        .max(bounds_min.z.abs())
        .max(bounds_max.z.abs())
        + 3.0;

    spawn_ground.y = floor_y;

    Facility {
        rooms,
        ports,
        doors,
        solids,
        spawn_ground,
        spawn_yaw,
        floor_y,
        floor_half,
        bounds_min,
        bounds_max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_source;
    use bevy_trenchbroom::qmap::QuakeMapEntities;

    /// Parse the authored map text exactly as the asset loader does, but synchronously.
    fn parsed() -> (QuakeMapEntities, TrenchBroomConfig) {
        let config = TrenchBroomConfig::new("observed2_trenchbroom_lab");
        let source = map_source::facility_map();
        let qmap = quake_map::parse(&mut std::io::Cursor::new(source))
            .expect("authored facility.map must parse");
        let entities = QuakeMapEntities::from_quake_map(qmap, &config);
        (entities, config)
    }

    fn facility() -> Facility {
        let (entities, config) = parsed();
        project(&entities, &config)
    }

    #[test]
    fn a_box_brush_round_trips_to_its_bevy_space_aabb() {
        // The winding gate: a single authored box must reconstruct to a non-inverted
        // AABB (min < max on every axis) of a sensible metre size.
        let (entities, _) = parsed();
        let world = entities
            .iter()
            .find(|e| e.classname() == Ok("worldspawn"))
            .expect("worldspawn present");
        let first = world.brushes.first().expect("worldspawn has brushes");
        let aabb = brush_aabb(first).expect("box brush is a cuboid");
        assert!(aabb.min.x < aabb.max.x, "x not inverted: {aabb:?}");
        assert!(aabb.min.y < aabb.max.y, "y not inverted: {aabb:?}");
        assert!(aabb.min.z < aabb.max.z, "z not inverted: {aabb:?}");
    }

    #[test]
    fn projects_two_rooms_a_corridor_and_their_ids() {
        let f = facility();
        assert_eq!(f.rooms.len(), 3, "two rooms + one corridor");
        assert_eq!(f.room(RoomId(1)).unwrap().kind, RoomKind::Room);
        assert_eq!(f.room(RoomId(2)).unwrap().kind, RoomKind::Corridor);
        assert_eq!(f.room(RoomId(3)).unwrap().kind, RoomKind::Room);
        // Rooms are laid out front-to-back along Bevy −Z; the corridor sits between.
        let a = f.room(RoomId(1)).unwrap().center();
        let corr = f.room(RoomId(2)).unwrap().center();
        let b = f.room(RoomId(3)).unwrap().center();
        assert!(
            a.z > corr.z && corr.z > b.z,
            "A in front of corridor in front of B"
        );
    }

    #[test]
    fn ports_link_the_rooms_with_stable_ids() {
        let f = facility();
        assert_eq!(f.ports.len(), 2);
        assert_eq!(f.ports[0].id, PortId(1));
        assert_eq!((f.ports[0].a, f.ports[0].b), (RoomId(1), RoomId(2)));
        assert_eq!((f.ports[1].a, f.ports[1].b), (RoomId(2), RoomId(3)));
    }

    #[test]
    fn doors_carry_model_driven_state_not_materials() {
        let f = facility();
        assert_eq!(f.doors.len(), 2);
        assert_eq!(f.doors[0].state, DoorState::Open); // A → corridor open
        assert_eq!(f.doors[1].state, DoorState::Closed); // corridor → B closed
        // A closed door contributes a collision solid; an open one does not.
        let open_count = f.solids.len();
        let closed_count = f.collision_solids().len();
        assert_eq!(
            closed_count,
            open_count + 1,
            "exactly the one closed door blocks the gap"
        );
    }

    #[test]
    fn imported_collision_has_the_structural_solids() {
        let f = facility();
        // 1 floor + 12 walls + 4 stair/platform brushes were authored.
        assert_eq!(
            f.solids.len(),
            17,
            "all worldspawn brushes imported as solids"
        );
        // Every solid is a non-degenerate box.
        for s in &f.solids {
            assert!(s.min.x < s.max.x && s.min.y <= s.max.y && s.min.z < s.max.z);
        }
    }

    #[test]
    fn spawn_is_inside_room_a_on_the_floor() {
        let f = facility();
        let a = f.room(RoomId(1)).unwrap();
        assert!(f.spawn_ground.x >= a.min.x && f.spawn_ground.x <= a.max.x);
        assert!(f.spawn_ground.z >= a.min.z && f.spawn_ground.z <= a.max.z);
        assert_eq!(f.spawn_ground.y, f.floor_y);
    }

    #[test]
    fn the_elevation_change_was_imported() {
        // At least one solid rises a step above the floor (the staircase/platform).
        let f = facility();
        let raised = f.solids.iter().filter(|s| s.max.y > 0.2).count();
        assert!(raised >= 4, "stairs + platform import as raised solids");
    }
}
