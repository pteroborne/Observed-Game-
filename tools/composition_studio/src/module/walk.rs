//! Walk a body through a module and report where it would get stuck.
//!
//! The validator answers "is this module legal". This answers a different and
//! harder question: **can something actually get through it**. A module can
//! satisfy every contract in `validate_module` and still strand a body - the
//! switchback did exactly that, and it took four stalled soak bots in a live
//! match to find out.
//!
//! Geometric, not physical. No Rapier, no timestep, no controller: the probe
//! samples the authored hulls directly and applies the same thresholds the
//! character config carries. That trade is deliberate. A physics walk is higher
//! fidelity but non-deterministic, slow, and impossible to run as a test over
//! all 58 modules; this runs in milliseconds, gives the same answer every time,
//! and can gate the whole corpus. It will not catch everything a real body
//! meets - it is a floor, not a ceiling, and the honest claim is "this cannot be
//! walked" rather than "this walks fine".
//!
//! Thresholds come from `FpsConfig::default()` and the importer, never from
//! numbers invented here: a probe with its own idea of a step height would pass
//! modules the game rejects, which is worse than no probe.

use bevy::prelude::*;
use observed_authoring::{AuthoredModule, TilePrototype};
use observed_traversal::ConvexRenderMesh;

/// What a body needs, taken from the shipped traversal profile.
#[derive(Clone, Copy, Debug)]
pub struct Thresholds {
    /// Autostep. A rise beyond this is a wall, not a step.
    pub step_height: f32,
    /// Above this the controller slides rather than climbs.
    pub max_slope_degrees: f32,
    /// The importer's own minimum, so the probe and `validate_module` agree.
    pub min_headroom: f32,
    /// How far apart samples are taken along the route, in metres.
    pub sample_spacing: f32,
}

impl Default for Thresholds {
    fn default() -> Self {
        // The builder's defaults are the shipped values; there is no free
        // `Default` on the profile itself.
        let config = observed_content::TraversalProfile::builder().build();
        Self {
            step_height: config.step_height,
            max_slope_degrees: config.maximum_slope_degrees,
            min_headroom: 2.2,
            // Finer than the body radius (0.38 m), so a gap narrower than a
            // body cannot slip between two samples unnoticed.
            sample_spacing: 0.25,
        }
    }
}

/// How far above the route's own height the *first* sample may find floor.
///
/// A body is placed on the surface at the start rather than required to step
/// up onto it. Kept to about a body's height so the start still cannot be the
/// roof of an 8 m cell.
const START_REACH: f32 = 1.8;

/// The module's surfaces, triangulated once.
pub struct Probe {
    triangles: Vec<[Vec3; 3]>,
}

impl Probe {
    #[must_use]
    pub fn from_prototype(prototype: &TilePrototype) -> Self {
        let mut triangles = Vec::new();
        for hull in &prototype.hulls {
            let Some(mesh) = ConvexRenderMesh::from_convex_hull(hull) else {
                continue;
            };
            for face in mesh.indices.chunks_exact(3) {
                let point = |index: u32| {
                    let p = mesh.positions[index as usize];
                    Vec3::new(p[0], p[1], p[2])
                };
                triangles.push([point(face[0]), point(face[1]), point(face[2])]);
            }
        }
        Self { triangles }
    }

    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// The highest surface at `(x, z)` no higher than `ceiling`.
    ///
    /// Bounded above rather than taking the global maximum, because the global
    /// maximum is the roof. Walking is always "what is under my feet, reachable
    /// from where I am", which is what the bound expresses.
    #[must_use]
    pub fn support(&self, x: f32, z: f32, ceiling: f32) -> Option<f32> {
        let mut best: Option<f32> = None;
        for tri in &self.triangles {
            let Some(y) = height_at(tri, x, z) else {
                continue;
            };
            if y <= ceiling && best.is_none_or(|b| y > b) {
                best = Some(y);
            }
        }
        best
    }

    /// Clear height above `floor` at `(x, z)`; infinite when nothing is over it.
    #[must_use]
    pub fn overhead(&self, x: f32, z: f32, floor: f32) -> f32 {
        let mut lowest = f32::INFINITY;
        for tri in &self.triangles {
            let Some(y) = height_at(tri, x, z) else {
                continue;
            };
            // A surface within a hair of the floor is the floor, not a ceiling.
            if y > floor + 0.05 && y < lowest {
                lowest = y;
            }
        }
        lowest - floor
    }
}

/// Where `(x, z)` meets a triangle, if it is inside its plan projection.
fn height_at(tri: &[Vec3; 3], x: f32, z: f32) -> Option<f32> {
    let (a, b, c) = (tri[0], tri[1], tri[2]);
    // Barycentric in plan. A degenerate triangle - one seen edge-on - has zero
    // plan area and no answer here, which is correct: you cannot stand on it.
    let det = (b.z - c.z) * (a.x - c.x) + (c.x - b.x) * (a.z - c.z);
    if det.abs() < 1e-6 {
        return None;
    }
    let l1 = ((b.z - c.z) * (x - c.x) + (c.x - b.x) * (z - c.z)) / det;
    let l2 = ((c.z - a.z) * (x - c.x) + (a.x - c.x) * (z - c.z)) / det;
    let l3 = 1.0 - l1 - l2;
    const EDGE: f32 = -1e-4;
    (l1 >= EDGE && l2 >= EDGE && l3 >= EDGE).then_some(l1 * a.y + l2 * b.y + l3 * c.y)
}

/// Why a walk stopped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WalkFailure {
    /// Nothing underfoot: a hole, or the route leaves the geometry.
    NoSurface { at: Vec3 },
    /// A rise the autostep cannot lift a body over.
    StepTooHigh { at: Vec3, rise: f32 },
    /// A surface the controller slides down instead of climbing.
    TooSteep { at: Vec3, degrees: f32 },
    /// Not enough room to stand up.
    LowCeiling { at: Vec3, clearance: f32 },
}

impl WalkFailure {
    #[must_use]
    pub fn at(self) -> Vec3 {
        match self {
            Self::NoSurface { at }
            | Self::StepTooHigh { at, .. }
            | Self::TooSteep { at, .. }
            | Self::LowCeiling { at, .. } => at,
        }
    }

    /// One line naming the fault and what to change.
    #[must_use]
    pub fn describe(self, limits: &Thresholds) -> String {
        match self {
            Self::NoSurface { .. } => {
                "nothing underfoot - a hole, or the route leaves the floor".into()
            }
            Self::StepTooHigh { rise, .. } => format!(
                "a {rise:.2} m rise; autostep only lifts {:.2} m",
                limits.step_height
            ),
            Self::TooSteep { degrees, .. } => format!(
                "{degrees:.0} degrees; the controller slides above {:.0}",
                limits.max_slope_degrees
            ),
            Self::LowCeiling { clearance, .. } => format!(
                "{clearance:.2} m of headroom; {:.2} m is the minimum",
                limits.min_headroom
            ),
        }
    }
}

/// The result of walking one route.
pub struct WalkReport {
    /// Every point actually stood on, in order. Empty if the start was unusable.
    pub path: Vec<Vec3>,
    pub failure: Option<WalkFailure>,
    /// Total height gained, in metres.
    pub climbed: f32,
    /// How far along the intended route the body got, 0..1.
    pub progress: f32,
}

impl WalkReport {
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.failure.is_none() && !self.path.is_empty()
    }
}

/// Walk `route`, sampling the surface under each step.
///
/// The route is a plan-space intent, not a path in 3D: heights come from the
/// geometry, which is the point. A route that reads as a straight line across
/// a tile becomes a climb if the tile is a ramp.
#[must_use]
pub fn walk(probe: &Probe, route: &[Vec3], limits: &Thresholds) -> WalkReport {
    let mut path: Vec<Vec3> = Vec::new();
    let mut climbed = 0.0;
    let mut previous: Option<Vec3> = None;

    let samples = sample(route, limits.sample_spacing);
    let total = samples.len().max(1);

    for (index, point) in samples.iter().enumerate() {
        let progress = |index: usize| {
            #[allow(clippy::cast_precision_loss)]
            {
                index as f32 / total as f32
            }
        };
        // Reachable from where the body is: it can step up by `step_height` and
        // fall any distance. Unbounded above would let the probe teleport onto
        // a mezzanine it cannot actually reach.
        //
        // The *first* sample is bounded too, off the route's own height. It was
        // unbounded once, which put the body on the roof - and then it walked
        // the roof end to end and reported every hall traversable. A probe that
        // fails open is worse than none, because it answers the question wrong
        // in the reassuring direction.
        // The first sample places the body on whatever surface is there, within
        // a body's height of the route: it has not walked anywhere yet, so
        // reachability does not constrain it. Every later sample is bounded by
        // the step the body can actually take.
        //
        // It was unbounded once, which put the body on the roof - and then it
        // walked the roof end to end and reported every hall traversable. A
        // probe that fails open is worse than none: it answers wrong in the
        // reassuring direction.
        let ceiling = previous.map_or(point.y + START_REACH, |p| p.y + limits.step_height);
        let Some(y) = probe.support(point.x, point.z, ceiling) else {
            return WalkReport {
                failure: Some(WalkFailure::NoSurface { at: *point }),
                progress: progress(index),
                path,
                climbed,
            };
        };
        let here = Vec3::new(point.x, y, point.z);

        if let Some(prior) = previous {
            let rise = here.y - prior.y;
            let run = (here - prior).xz().length().max(1e-4);
            if rise > limits.step_height {
                return WalkReport {
                    failure: Some(WalkFailure::StepTooHigh { at: here, rise }),
                    progress: progress(index),
                    path,
                    climbed,
                };
            }
            let degrees = (rise / run).atan().to_degrees();
            if degrees > limits.max_slope_degrees {
                return WalkReport {
                    failure: Some(WalkFailure::TooSteep { at: here, degrees }),
                    progress: progress(index),
                    path,
                    climbed,
                };
            }
            if rise > 0.0 {
                climbed += rise;
            }
        }

        let clearance = probe.overhead(point.x, point.z, y);
        if clearance < limits.min_headroom {
            return WalkReport {
                failure: Some(WalkFailure::LowCeiling {
                    at: here,
                    clearance,
                }),
                progress: progress(index),
                path,
                climbed,
            };
        }

        path.push(here);
        previous = Some(here);
    }

    WalkReport {
        path,
        failure: None,
        climbed,
        progress: 1.0,
    }
}

/// Break a polyline into points no further apart than `spacing`.
#[must_use]
fn sample(route: &[Vec3], spacing: f32) -> Vec<Vec3> {
    let mut out = Vec::new();
    if route.is_empty() {
        return out;
    }
    out.push(route[0]);
    for pair in route.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let span = (to - from).xz().length();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = ((span / spacing.max(0.01)).ceil() as usize).max(1);
        for step in 1..=steps {
            #[allow(clippy::cast_precision_loss)]
            let t = step as f32 / steps as f32;
            out.push(from.lerp(to, t));
        }
    }
    out
}

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
        let y = probe.support(x, z, deck + START_REACH)?;
        (probe.overhead(x, z, y) >= limits.min_headroom).then_some(Vec3::new(x, y, z))
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

#[cfg(test)]
mod tests {
    use super::*;
    use observed_authoring::parse_authored_module;

    fn module(stem: &str) -> AuthoredModule {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/tiles/authored")
            .join(format!("{stem}.map"));
        let text = std::fs::read_to_string(&path).expect("authored module");
        parse_authored_module(&text).expect("module validates")
    }

    /// The probe has to find the floor of a plain hall at the deck height the
    /// contract puts it at. If this is wrong every other answer is noise.
    #[test]
    fn the_probe_finds_the_deck_of_a_hall() {
        let hall = module("hall_straight");
        let probe = Probe::from_prototype(&hall.prototype);
        assert!(probe.triangle_count() > 0);
        // Bounded to standing height. Unbounded returns the roof, correctly -
        // "the highest surface here" and "what I am standing on" are different
        // questions and the API keeps them apart.
        let floor = probe
            .support(0.0, 0.0, observed_hex::FLOOR_SLAB_TOP + 0.42)
            .expect("a hall has a floor at its centre");
        // Floor slab top is 0.5 m; the ceiling is 8 m up and must not be picked.
        assert!(
            (floor - 0.5).abs() < 0.05,
            "centre support came back at {floor:.2} m, expected the 0.5 m deck"
        );
    }

    /// A bounded query is what stops the probe standing on the roof.
    #[test]
    fn support_is_bounded_and_does_not_pick_the_ceiling() {
        let hall = module("hall_straight");
        let probe = Probe::from_prototype(&hall.prototype);
        let roof = probe.support(0.0, 0.0, f32::INFINITY).expect("roof");
        let deck = probe
            .support(0.0, 0.0, observed_hex::FLOOR_SLAB_TOP + 0.42)
            .expect("deck");
        assert!(
            roof > deck + 2.0,
            "the bound must actually exclude the roof"
        );
        let ceiling_height = probe.overhead(0.0, 0.0, deck);
        assert!(
            ceiling_height > 2.2,
            "a hall must be standable at its centre, got {ceiling_height:.2} m"
        );
    }

    /// Halls whose middle is clear must walk door to door.
    ///
    /// This is the regression gate the switchback never had: a module can pass
    /// `validate_module` and still strand a body, and that is what shipped.
    #[test]
    fn a_straight_route_walks_clean_where_the_middle_is_clear() {
        let limits = Thresholds::default();
        for stem in [
            "hall_straight",
            "hall_straight_buttressed",
            "hall_cap",
            "hall_turn_60",
            "hall_turn_60_buttressed",
            "hall_turn_120",
        ] {
            let hall = module(stem);
            let probe = Probe::from_prototype(&hall.prototype);
            let route = route_for(&hall, &probe, &limits);
            assert!(route.len() >= 2, "{stem} produced no route");
            let report = walk(&probe, &route, &limits);
            assert!(
                report.is_clear(),
                "{stem} is not walkable: {} at {:?} ({:.0}% along)",
                report
                    .failure
                    .map_or_else(|| "no path".to_string(), |f| f.describe(&limits)),
                report.failure.map(WalkFailure::at),
                report.progress * 100.0
            );
        }
    }

    /// A junction's waypoint pylon stands dead centre on purpose, so a straight
    /// route walks into it. The probe must **say so** rather than quietly
    /// finding a way round.
    ///
    /// Pinned as an expectation rather than excused, because it is the honest
    /// boundary of what this tool claims: `route_for` is a straight probe, not
    /// a pathfinder. It answers "is a direct line through here clear", and for
    /// a junction the true answer is no. Reporting that as a tile fault would
    /// be wrong; hiding it would make the probe's silence meaningless.
    #[test]
    fn a_centre_pylon_blocks_a_straight_route_and_the_probe_says_so() {
        let limits = Thresholds::default();
        for stem in ["hall_junction_3way", "hall_junction_4way"] {
            let junction = module(stem);
            let probe = Probe::from_prototype(&junction.prototype);
            let route = route_for(&junction, &probe, &limits);
            let report = walk(&probe, &route, &limits);
            let failure = report.failure.unwrap_or_else(|| {
                panic!("{stem} has a centre pylon; a straight line must hit it")
            });
            assert!(
                matches!(failure, WalkFailure::LowCeiling { .. }),
                "{stem} should be blocked by the pylon overhead, got {failure:?}"
            );
            assert!(
                failure.at().xz().length() < 3.0,
                "{stem} should be blocked near the centre, blocked at {:?}",
                failure.at()
            );
        }
    }

    /// The probe must actually be able to fail, or the gate above proves
    /// nothing. A route out through the wall has no floor under it.
    #[test]
    fn walking_off_the_module_is_reported_rather_than_ignored() {
        let hall = module("hall_cap");
        let probe = Probe::from_prototype(&hall.prototype);
        let off_the_edge = vec![
            Vec3::new(0.0, observed_hex::FLOOR_SLAB_TOP, 0.0),
            Vec3::new(40.0, observed_hex::FLOOR_SLAB_TOP, 0.0),
        ];
        let report = walk(&probe, &off_the_edge, &Thresholds::default());
        assert!(
            matches!(report.failure, Some(WalkFailure::NoSurface { .. })),
            "walking into open air must fail, got {:?}",
            report.failure
        );
        assert!(report.progress < 1.0);
    }

    /// A ramp must register as climbed, or the probe is silently walking a
    /// flat interpretation of sloped geometry.
    #[test]
    fn a_ramp_reports_the_height_it_gains() {
        let ramp = module("hall_ramp");
        let probe = Probe::from_prototype(&ramp.prototype);
        // West door inward to the centre and on toward the east sill.
        let deck = observed_hex::FLOOR_SLAB_TOP;
        let route = vec![
            Vec3::new(-5.5, deck, 0.0),
            Vec3::new(0.0, deck, 0.0),
            Vec3::new(5.5, deck, 0.0),
        ];
        let report = walk(&probe, &route, &Thresholds::default());
        assert!(
            report.climbed > 3.0,
            "a full-level ramp should gain height; got {:.2} m ({:?})",
            report.climbed,
            report.failure
        );
    }

    /// Thresholds come from the shipped config, not from numbers chosen here.
    /// A probe with its own idea of a step height passes what the game rejects.
    #[test]
    fn thresholds_track_the_character_config() {
        let limits = Thresholds::default();
        // The builder's defaults are the shipped values; there is no free
        // `Default` on the profile itself.
        let config = observed_content::TraversalProfile::builder().build();
        assert_eq!(limits.step_height, config.step_height);
        assert_eq!(limits.max_slope_degrees, config.maximum_slope_degrees);
        assert!(
            limits.sample_spacing < config.radius,
            "samples must be finer than a body is wide, or a gap can hide between them"
        );
    }
}
