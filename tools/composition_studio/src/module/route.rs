//! Choosing the line a body is asked to walk, and walking it.
//!
//! Separate from `walk` because deciding *where* to go is a different concern
//! from judging whether the going is possible. `walk` takes a route as given;
//! this is where a route comes from.

use bevy::prelude::*;
use observed_authoring::{AuthoredModule, TilePrototype};

use super::probe::Probe;
use super::walk::{Thresholds, WalkReport, walk};

/// The route a body would take through `module`.
///
/// The authored stair spine when there is one - that is what it is *for*, and
/// walking it checks the spine against the surface underneath, which is the
/// drift the two were meant to be unable to have. Otherwise the doors are
/// joined through the cell centre, which is the route through a hall.
#[must_use]
pub fn route_for(module: &AuthoredModule, probe: &Probe, limits: &Thresholds) -> Vec<Vec3> {
    if module.prototype.spine.nodes.len() >= 2 {
        return module.prototype.spine.nodes.clone();
    }

    // Just inside each door, so the probe starts on floor rather than in the
    // door's own threshold geometry.
    const INSET: f32 = 1.5;
    let mut doors: Vec<Vec3> = Vec::new();
    for port in &module.ports {
        let Some(origin) = port.origin else {
            continue;
        };
        let world = observed_authoring::editor_origin_to_world(origin);
        let plan = Vec2::new(world.x, world.z);
        let inward = -plan.normalize_or_zero() * INSET;
        doors.push(Vec3::new(
            world.x + inward.x,
            observed_hex::FLOOR_SLAB_TOP,
            world.z + inward.y,
        ));
    }
    let waypoint = interior_waypoint(probe, limits);
    match doors.len() {
        0 => Vec::new(),
        // One door: in and back out is not a traversal. Walk to the interior
        // waypoint, which is where a vertical exit lives and where a dead end
        // ends.
        1 => vec![doors[0], waypoint],
        _ => {
            let mut route = vec![doors[0], waypoint];
            route.extend(doors.iter().skip(1).copied());
            route
        }
    }
}

/// A standable point in the middle of the cell.
///
/// The centre first, then a ring around it. Junctions put a **waypoint pylon**
/// dead centre on purpose, so a route through the centre walks into it - and
/// reporting that as "this tile is not walkable" would be wrong twice over: the
/// tile is fine, and a body simply goes round. Trying offsets is not
/// pathfinding, it is the one thing a body obviously does when the middle is
/// occupied, and it keeps the probe's failures about geometry rather than about
/// the naivety of a straight line.
#[must_use]
fn interior_waypoint(probe: &Probe, limits: &Thresholds) -> Vec3 {
    let deck = observed_hex::FLOOR_SLAB_TOP;
    let standable = |x: f32, z: f32| -> Option<Vec3> {
        let y = probe.support(x, z, deck + limits.requirements.required_headroom)?;
        (probe.overhead(x, z, y) >= limits.authoring_headroom_standard)
            .then_some(Vec3::new(x, y, z))
    };
    if let Some(centre) = standable(0.0, 0.0) {
        return centre;
    }
    // Six directions at a radius that clears a centre fitting but stays well
    // inside the walls.
    const RADIUS: f32 = 3.2;
    for step in 0..6 {
        #[allow(clippy::cast_precision_loss)]
        let angle = std::f32::consts::TAU * step as f32 / 6.0;
        if let Some(point) = standable(RADIUS * angle.cos(), RADIUS * angle.sin()) {
            return point;
        }
    }
    Vec3::new(0.0, deck, 0.0)
}

/// Probe the module in `diagnosis`, if it has geometry and a route.
///
/// Runs on whatever parsed, including a module that failed validation - the
/// moment you most want to know whether a body can get through is when
/// something else is already wrong with it.
#[must_use]
pub fn walk_module(diagnosis: &crate::module::diagnose::Diagnosis) -> Option<WalkReport> {
    let prototype = diagnosis.prototype.as_ref()?;
    let module = diagnosis.module.as_ref()?;
    let limits = Thresholds::default();
    let probe = Probe::from_prototype(prototype);
    let route = route_for(module, &probe, &limits);
    (route.len() >= 2).then(|| walk(&probe, &route, &limits))
}

/// Walk a prototype along its own authored spine.
///
/// The spine is the contract a follower actually uses, so walking it is the
/// closest a geometric probe gets to asking "can a bot climb this". A tower
/// with no spine cannot be followed at all and reports `None` rather than a
/// misleading pass.
#[must_use]
pub fn walk_spine(prototype: &TilePrototype, limits: &Thresholds) -> Option<WalkReport> {
    let nodes = &prototype.spine.nodes;
    (nodes.len() >= 2).then(|| {
        let probe = Probe::from_prototype(prototype);
        walk(&probe, nodes, limits)
    })
}
