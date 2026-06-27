//! The **derived navigation consumer**. It builds a `vleue_navigator` navmesh from
//! the authoritative [`Facility`]'s current obstacle set and answers route queries
//! by running polyanya pathfinding over it. Crucially it is a *one-way* consumer:
//! it reads door state and geometry, and never writes back to the facility graph,
//! which remains the source of truth.
//!
//! The navmesh is rebuilt whenever door state changes, so a closed door becomes a
//! solid obstacle and the path either detours or fails — the navmesh can never
//! route through a door the facility says is shut.

use bevy::math::Vec2;
use observed_core::RoomId;
use vleue_navigator::NavMesh;

use crate::facility::{self, Facility};

/// Build a navmesh from the facility's current walls + closed-door plugs. Pure CPU
/// construction (constrained Delaunay triangulation + polyanya), so it is callable
/// in tests with no Bevy app or GPU.
pub fn build_navmesh(facility: &Facility) -> NavMesh {
    let edges = facility::outer_boundary();
    let obstacles: Vec<Vec<Vec2>> = facility
        .obstacles()
        .iter()
        .map(|rect| rect.corners())
        .collect();
    NavMesh::from_edge_and_obstacles(edges, obstacles)
}

/// A navigation query result, classified against the authoritative room graph.
#[derive(Clone, Debug)]
pub struct NavRoute {
    /// The polyanya path, with the start point prepended so it is a complete
    /// polyline from `from`'s centre to `to`'s centre.
    pub waypoints: Vec<Vec2>,
    /// Total path length in floor units.
    pub length: f32,
    /// The rooms the path passes through, in order (doorway/divider points
    /// skipped). Derived only to *compare* against the graph; not connectivity.
    pub rooms: Vec<RoomId>,
}

/// Ask the navmesh for a route from one room centre to another. Returns `None`
/// exactly when the navmesh finds no path (which should coincide with the graph
/// reporting the rooms disconnected — that agreement is what the tests assert).
pub fn query(navmesh: &NavMesh, from: RoomId, to: RoomId) -> Option<NavRoute> {
    let start = facility::room_center(from);
    let goal = facility::room_center(to);
    if from == to {
        return Some(NavRoute {
            waypoints: vec![start],
            length: 0.0,
            rooms: vec![from],
        });
    }
    let path = navmesh.path(start, goal)?;
    let mut waypoints = Vec::with_capacity(path.path.len() + 1);
    waypoints.push(start);
    waypoints.extend(path.path.iter().copied());
    let rooms = path_rooms(&waypoints);
    Some(NavRoute {
        waypoints,
        length: path.length,
        rooms,
    })
}

/// The sequence of rooms a polyline passes through (deduped; points on dividers or
/// in doorways are skipped). Used only to compare the derived route against the
/// authoritative graph.
pub fn path_rooms(waypoints: &[Vec2]) -> Vec<RoomId> {
    let mut out: Vec<RoomId> = Vec::new();
    for &p in waypoints {
        if let Some(room) = facility::room_at(p)
            && out.last() != Some(&room)
        {
            out.push(room);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facility::{DoorId, Facility, all_rooms};

    /// The headline cross-check: across an exhaustive sweep of door configurations
    /// and every ordered room pair, the navmesh agrees with the authoritative graph
    /// about reachability, and never produces a path that crosses a closed door.
    #[test]
    fn navmesh_agrees_with_the_authoritative_graph_for_every_door_config() {
        for bits in 0u8..(1 << 4) {
            let mut facility = Facility::all_open();
            for door in 0..4u8 {
                facility.set_open(DoorId(door), bits & (1 << door) != 0);
            }
            let navmesh = build_navmesh(&facility);
            for from in all_rooms() {
                for to in all_rooms() {
                    let nav = query(&navmesh, from, to);
                    let graph = facility.graph_reachable(from, to);
                    assert_eq!(
                        nav.is_some(),
                        graph,
                        "reachability disagrees for {from:?}->{to:?} with doors {bits:04b}"
                    );
                    if let Some(route) = nav {
                        assert!(
                            facility.open_walk_valid(&route.rooms, from, to),
                            "navmesh route {:?} for {from:?}->{to:?} (doors {bits:04b}) \
                             is not a valid open-door walk",
                            route.rooms
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn all_doors_open_reaches_every_room_from_a() {
        let facility = Facility::all_open();
        let navmesh = build_navmesh(&facility);
        for to in all_rooms() {
            let route = query(&navmesh, RoomId(0), to).expect("reachable with all doors open");
            assert_eq!(route.rooms.first(), Some(&RoomId(0)));
            assert_eq!(route.rooms.last(), Some(&to));
        }
    }

    #[test]
    fn closing_a_door_reroutes_around_it() {
        // All open: A->D exists.
        let mut facility = Facility::all_open();
        let open_route = query(&build_navmesh(&facility), RoomId(0), RoomId(3))
            .expect("A->D reachable with all doors open");
        assert!(open_route.length > 0.0);

        // Close AB: the only A->D route now runs through C, and it must not pass
        // through room B at all.
        facility.set_open(DoorId(0), false);
        let detour = query(&build_navmesh(&facility), RoomId(0), RoomId(3))
            .expect("A->D still reachable via C");
        assert_eq!(detour.rooms, vec![RoomId(0), RoomId(2), RoomId(3)]);
        assert!(
            !detour.rooms.contains(&RoomId(1)),
            "rerouted path must avoid B after AB closes"
        );
    }

    #[test]
    fn a_closed_door_blocks_the_path_entirely() {
        // Shut both of A's doors: navmesh must report A->D unreachable, matching
        // the graph.
        let mut facility = Facility::all_open();
        facility.set_open(DoorId(0), false); // AB
        facility.set_open(DoorId(1), false); // AC
        let navmesh = build_navmesh(&facility);
        assert!(query(&navmesh, RoomId(0), RoomId(3)).is_none());
        assert!(query(&navmesh, RoomId(0), RoomId(1)).is_none());
        assert!(!facility.graph_reachable(RoomId(0), RoomId(3)));
    }

    #[test]
    fn navmesh_construction_and_pathfinding_are_deterministic() {
        let facility = Facility::all_open();
        let a = query(&build_navmesh(&facility), RoomId(0), RoomId(3)).unwrap();
        let b = query(&build_navmesh(&facility), RoomId(0), RoomId(3)).unwrap();
        assert!((a.length - b.length).abs() < 1e-4);
        assert_eq!(a.rooms, b.rooms);
        assert_eq!(a.waypoints.len(), b.waypoints.len());
    }

    #[test]
    fn rerouting_lengthens_or_preserves_the_path_but_never_cheats() {
        // Detour length must be >= the all-open shortest, and still a valid walk.
        let mut facility = Facility::all_open();
        let direct = query(&build_navmesh(&facility), RoomId(0), RoomId(3))
            .unwrap()
            .length;
        facility.set_open(DoorId(0), false);
        facility.set_open(DoorId(2), false); // close AB and BD: kill the whole B side
        let route = query(&build_navmesh(&facility), RoomId(0), RoomId(3)).unwrap();
        assert!(route.length >= direct - 1e-3);
        assert!(facility.open_walk_valid(&route.rooms, RoomId(0), RoomId(3)));
    }
}
