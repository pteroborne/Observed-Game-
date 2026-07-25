//! Plan-space (x, z) segment-vs-convex-hull intersection.
//!
//! A previous obstacle-avoidance attempt tested a straight-line heading against the
//! **axis-aligned bounding box** of a collider's hull and failed badly: the facility's
//! walls are angled hexagon edges, and an AABB wildly over-approximates an angled
//! wall, so nearly every straight heading appeared to graze a "wall" that wasn't
//! really there. This module is the missing primitive — an exact convex-hull test in
//! the horizontal plane — built as a pure, engine-independent utility so it can be
//! unit tested in isolation before (if ever) being wired into bot steering.
//!
//! World axes here follow the rest of the codebase: `y` is up, so the horizontal plan
//! is the `(x, z)` pair, **not** `(x, y)`.
//!
//! Pure arithmetic only: no RNG, no time, no globals, no Bevy. This crate feeds a
//! deterministic lockstep simulation whose snapshot digests are compared for LAN
//! desync detection, so the same inputs must always produce the same answer.

use glam::{Vec2, Vec3};

/// Tolerance for collinearity / overlap / boundary tests. Tuned for the facility's
/// metre-scale geometry (hex cells span ~14m), well above `f32` rounding noise from a
/// handful of arithmetic operations on coordinates of that magnitude.
const EPS: f32 = 1e-4;

/// The convex hull of `points` projected to the plan `(x, z)`, as a polygon.
///
/// Degenerate inputs are returned as their natural lower-dimensional hull rather than
/// panicking or padding: an empty slice (or a slice that projects to a single distinct
/// point) yields 0 or 1 points; a set of points that are all collinear in plan yields
/// the 2 extreme points of that line, not a "flattened triangle". Callers that need a
/// polygon should treat any result with fewer than 3 points as degenerate.
///
/// Uses Andrew's monotone chain. The winding order (CW vs CCW) is **not** guaranteed —
/// [`segment_crosses_convex_hull`] and [`point_in_convex_plan_hull`] are orientation
/// agnostic, so callers must not assume a winding direction either.
#[must_use]
pub fn plan_convex_hull(points: &[Vec3]) -> Vec<Vec2> {
    let mut pts: Vec<Vec2> = points.iter().map(|p| Vec2::new(p.x, p.z)).collect();
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup();
    if pts.len() < 2 {
        // 0 or 1 distinct points: no area, no edges. Return as-is.
        return pts;
    }

    let mut lower: Vec<Vec2> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross2(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<Vec2> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross2(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    // Each of lower/upper repeats the other's starting point at its end; drop it before
    // splicing them into a single loop. (Mirrors `observed_authoring::source::point_in_plan_hull`.)
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Does `point` lie inside (or on the boundary of) the convex hull of `hull`'s points
/// projected to the plan? Boundary points count as inside — see
/// [`segment_crosses_convex_hull`] for the reasoning.
#[must_use]
pub fn point_in_convex_plan_hull(hull: &[Vec3], point: Vec2) -> bool {
    // A point is exactly a zero-length segment; every case below (empty hull, a single
    // point, a degenerate line, and a real polygon via the half-plane sweep) already
    // reduces correctly to a point-in-hull test when `start == end`, so there is no
    // separate code path to keep in sync.
    segment_crosses_convex_hull(point, point, hull)
}

/// Does the plan-space (x, z) segment `start -> end` intersect the convex hull of
/// `hull`'s points projected to the plan?
///
/// Boundary handling: touching a vertex or lying exactly along an edge counts as
/// crossing (inclusive `<=` comparisons throughout). This is the safer default for
/// obstacle avoidance — a heading that just grazes a wall's exact edge should still be
/// treated as blocked, not waved through by a coin-flip of floating point rounding.
/// A segment that lies entirely inside the hull also counts as crossing: a body
/// already overlapping an obstacle is obstructed, not clear.
///
/// Degenerate hulls never panic: an empty hull can't be crossed (`false`); a
/// single-point hull is crossed only if the segment passes through that exact point;
/// a hull that collapses to a line (all input points collinear in plan) is treated as
/// a thin segment obstacle and tested with segment/segment intersection.
#[must_use]
pub fn segment_crosses_convex_hull(start: Vec2, end: Vec2, hull: &[Vec3]) -> bool {
    let poly = plan_convex_hull(hull);
    match poly.len() {
        0 => false,
        1 => segments_intersect(start, end, poly[0], poly[0]),
        2 => segments_intersect(start, end, poly[0], poly[1]),
        _ => segment_crosses_polygon(start, end, &poly),
    }
}

fn cross2(origin: Vec2, a: Vec2, b: Vec2) -> f32 {
    (a - origin).perp_dot(b - origin)
}

/// Signed area x2 of the triangle (a, b, c); this is `cross2` under a name that reads
/// naturally at call sites testing which side of a directed line a point falls on.
fn orient(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    cross2(a, b, c)
}

/// Is `p` within the axis-aligned bounding box of segment `a`-`b`? Only meaningful
/// (and only called) once `p` is already known to be collinear with `a`-`b`.
fn on_segment(a: Vec2, b: Vec2, p: Vec2) -> bool {
    p.x >= a.x.min(b.x) - EPS
        && p.x <= a.x.max(b.x) + EPS
        && p.y >= a.y.min(b.y) - EPS
        && p.y <= a.y.max(b.y) + EPS
}

/// Exact segment/segment intersection, including collinear overlap and degenerate
/// (zero-length) segments on either side. Used directly for the 0/1/2-point degenerate
/// hull cases, and internally reduces a zero-length query segment to a point test.
fn segments_intersect(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> bool {
    let d1 = orient(p3, p4, p1);
    let d2 = orient(p3, p4, p2);
    let d3 = orient(p1, p2, p3);
    let d4 = orient(p1, p2, p4);

    let straddles = |x: f32, y: f32| (x > EPS && y < -EPS) || (x < -EPS && y > EPS);
    if straddles(d1, d2) && straddles(d3, d4) {
        return true;
    }
    (d1.abs() <= EPS && on_segment(p3, p4, p1))
        || (d2.abs() <= EPS && on_segment(p3, p4, p2))
        || (d3.abs() <= EPS && on_segment(p1, p2, p3))
        || (d4.abs() <= EPS && on_segment(p1, p2, p4))
}

/// Segment vs. real (>=3 vertex) convex polygon via a Cyrus-Beck-style parametric
/// half-plane clip: walk the polygon's edges, and for each treat the *outward* side as
/// forbidden, narrowing the segment's parametric range `t in [0, 1]` to the part that
/// stays inside every edge. A non-empty remaining range means some part of the segment
/// (possibly all of it, possibly just an endpoint) lies inside or on the hull.
///
/// The per-edge outward normal is derived relative to the polygon's centroid rather
/// than assumed from winding order, so this works regardless of whether
/// `plan_convex_hull` happens to return points clockwise or counterclockwise.
fn segment_crosses_polygon(start: Vec2, end: Vec2, poly: &[Vec2]) -> bool {
    debug_assert!(poly.len() >= 3);
    let centroid = poly.iter().fold(Vec2::ZERO, |acc, &p| acc + p) / poly.len() as f32;
    let direction = end - start;

    let mut t_min = 0.0_f32;
    let mut t_max = 1.0_f32;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let edge = b - a;
        let mut normal = Vec2::new(edge.y, -edge.x);
        if normal.dot(centroid - a) > 0.0 {
            normal = -normal;
        }

        // value(t) = normal . (P(t) - a); inside this edge's half-plane iff value <= 0.
        let c = normal.dot(start - a); // value at t = 0
        let m = normal.dot(direction); // d(value)/dt

        if m.abs() <= EPS {
            // The segment runs parallel to this edge's constraint: it never changes
            // which side it's on, so either the whole segment is outside this
            // half-plane (reject outright) or this edge places no further limit on t.
            if c > EPS {
                return false;
            }
            continue;
        }
        let t_edge = -c / m;
        if m > 0.0 {
            t_max = t_max.min(t_edge);
        } else {
            t_min = t_min.max(t_edge);
        }
        if t_min > t_max + EPS {
            return false;
        }
    }
    t_min <= t_max + EPS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real hexagon footprint used by the facility's tiles: `observed_hex`'s
    /// `metrics::CORNERS`, mirrored here (not depended on — this crate stays
    /// engine/authoring independent) so the "grazing" regression reflects actual
    /// facility wall geometry, not a toy square. Plan metres, one vertex per corner,
    /// at y = 0 (the hull test only ever looks at x/z, so the y value is arbitrary and
    /// deliberately varied in some tests to prove that).
    fn hex_corners() -> Vec<Vec3> {
        [
            (7.0, -4.0),
            (7.0, 4.0),
            (0.0, 8.0),
            (-7.0, 4.0),
            (-7.0, -4.0),
            (0.0, -8.0),
        ]
        .into_iter()
        .map(|(x, z)| Vec3::new(x, 0.0, z))
        .collect()
    }

    // --- basic hits and misses -------------------------------------------------

    #[test]
    fn segment_clearly_missing_the_hull_is_false() {
        let hex = hex_corners();
        // Far off to the east, running parallel to the hull, never close to it.
        assert!(!segment_crosses_convex_hull(
            Vec2::new(50.0, -10.0),
            Vec2::new(50.0, 10.0),
            &hex,
        ));
    }

    #[test]
    fn segment_clearly_through_the_middle_is_true() {
        let hex = hex_corners();
        assert!(segment_crosses_convex_hull(
            Vec2::new(-20.0, 0.0),
            Vec2::new(20.0, 0.0),
            &hex,
        ));
    }

    #[test]
    fn segment_fully_inside_the_hull_is_true() {
        let hex = hex_corners();
        // A short chord entirely within the hexagon, nowhere near an edge.
        assert!(segment_crosses_convex_hull(
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, 1.0),
            &hex,
        ));
    }

    // --- THE regression: AABB over-approximates an angled edge, the hull does not ---

    #[test]
    fn segment_grazing_past_an_angled_edge_misses_the_hull_but_would_hit_its_aabb() {
        // The hexagon's north-east edge runs from (7, -4) to (0, -8): a wall angled in
        // plan, not axis aligned. The hull's AABB is x in [-7, 7], z in [-8, 8] -- a
        // square that includes the whole north-west corner region the real hexagon
        // cuts off.
        //
        // At x = 5 the true edge sits at z = -4 + (2/7)*(-8 - -4) = -36/7 ≈ -5.143 (the
        // interior is the *greater* z, toward the z = 0 center). A vertical segment at
        // x = 5 from z = -7.9 to z = -6.0 stays entirely on the far side of that edge:
        // outside the real hexagon. But it is comfortably inside the hull's AABB the
        // whole way. Under the old AABB-of-the-hull test this heading reads as a hit --
        // exactly the failure mode described in the task: "nearly every straight-line
        // heading appeared to cross a wall" against angled hex geometry. Against the
        // real convex hull it is a clean miss.
        let hex = hex_corners();
        let start = Vec2::new(5.0, -7.9);
        let end = Vec2::new(5.0, -6.0);

        // Sanity check the premise: the segment really is inside the hull's AABB...
        let aabb_min_x = hex.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let aabb_max_x = hex.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let aabb_min_z = hex.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
        let aabb_max_z = hex.iter().map(|p| p.z).fold(f32::NEG_INFINITY, f32::max);
        assert!(start.x > aabb_min_x && start.x < aabb_max_x);
        assert!(start.y > aabb_min_z && end.y < aabb_max_z);

        // ...but clear of the real edge (z stays north of/less than the boundary).
        let edge_z_at_x5 = -4.0 + (2.0 / 7.0) * (-8.0 - -4.0);
        assert!(start.y < edge_z_at_x5 && end.y < edge_z_at_x5);

        assert!(
            !segment_crosses_convex_hull(start, end, &hex),
            "a segment grazing past the angled north-east edge must miss the real hull, \
             even though it lies inside that edge's AABB"
        );
    }

    #[test]
    fn segment_just_inside_the_angled_edge_does_hit() {
        // Same x = 5 heading as above, extended 2m further south so its end point now
        // lands just inside the real edge (z = -5.0 > the boundary's -36/7 ≈ -5.143)
        // instead of stopping short of it -- confirms the miss above is really about
        // the hull boundary, not a bug that always returns false near this corner.
        let hex = hex_corners();
        assert!(segment_crosses_convex_hull(
            Vec2::new(5.0, -7.9),
            Vec2::new(5.0, -5.0),
            &hex,
        ));
    }

    // --- boundary / touching cases (documented choice: inclusive) --------------

    #[test]
    fn segment_touching_a_vertex_counts_as_crossing() {
        let hex = hex_corners();
        // Approaches the north corner (0, -8) from outside and stops exactly on it.
        assert!(
            segment_crosses_convex_hull(Vec2::new(0.0, -20.0), Vec2::new(0.0, -8.0), &hex),
            "touching a hull vertex is treated as a crossing (inclusive boundary, see \
             segment_crosses_convex_hull's doc comment)"
        );
    }

    #[test]
    fn segment_collinear_with_an_edge_and_overlapping_it_counts_as_crossing() {
        let hex = hex_corners();
        // Runs exactly along the east edge (x = 7, z from -4 to 4), fully overlapping it.
        assert!(segment_crosses_convex_hull(
            Vec2::new(7.0, -4.0),
            Vec2::new(7.0, 4.0),
            &hex,
        ));
    }

    #[test]
    fn segment_collinear_with_an_edges_line_but_past_its_ends_does_not_cross() {
        let hex = hex_corners();
        // Same line (x = 7) as the east edge, but entirely beyond its z extent.
        assert!(!segment_crosses_convex_hull(
            Vec2::new(7.0, 10.0),
            Vec2::new(7.0, 20.0),
            &hex,
        ));
    }

    // --- degenerate hulls: must not panic ---------------------------------------

    #[test]
    fn empty_hull_never_crosses() {
        let hull: Vec<Vec3> = Vec::new();
        assert!(!segment_crosses_convex_hull(
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, 1.0),
            &hull,
        ));
        assert!(!point_in_convex_plan_hull(&hull, Vec2::ZERO));
        assert!(plan_convex_hull(&hull).is_empty());
    }

    #[test]
    fn single_point_hull_only_crosses_through_that_exact_point() {
        let hull = vec![Vec3::new(2.0, 5.0, 3.0)];
        let point_plan = Vec2::new(2.0, 3.0); // (x, z) of the single hull point
        assert!(segment_crosses_convex_hull(
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 6.0),
            &hull,
        ));
        assert!(!segment_crosses_convex_hull(
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 1.0),
            &hull,
        ));
        assert!(point_in_convex_plan_hull(&hull, point_plan));
        assert!(!point_in_convex_plan_hull(&hull, Vec2::new(2.0, 3.1)));
    }

    #[test]
    fn two_point_hull_behaves_as_a_thin_segment_obstacle() {
        // Two points that are already distinct: the hull is the segment between them,
        // not a shape with area.
        let hull = vec![Vec3::new(-3.0, 9.0, 0.0), Vec3::new(3.0, -1.0, 0.0)];
        assert_eq!(plan_convex_hull(&hull).len(), 2);
        // Crosses it.
        assert!(segment_crosses_convex_hull(
            Vec2::new(-3.0, -3.0),
            Vec2::new(3.0, 3.0),
            &hull,
        ));
        // Misses it (parallel, offset).
        assert!(!segment_crosses_convex_hull(
            Vec2::new(-3.0, -3.0),
            Vec2::new(3.0, 3.0),
            &[Vec3::new(-3.0, 9.0, 10.0), Vec3::new(3.0, -1.0, 10.0)],
        ));
    }

    #[test]
    fn collinear_points_collapse_to_the_two_extremes() {
        // Five points on the same line in plan (y varies to prove y is ignored).
        let points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 7.0, 1.0),
            Vec3::new(2.0, -3.0, 2.0),
            Vec3::new(3.0, 2.0, 3.0),
            Vec3::new(4.0, 0.0, 4.0),
        ];
        let hull = plan_convex_hull(&points);
        assert_eq!(
            hull.len(),
            2,
            "a collinear point set has no area: hull is a segment"
        );
        assert!(hull.contains(&Vec2::new(0.0, 0.0)));
        assert!(hull.contains(&Vec2::new(4.0, 4.0)));

        // And segment_crosses_convex_hull must not panic or misbehave against it.
        assert!(segment_crosses_convex_hull(
            Vec2::new(0.0, 4.0),
            Vec2::new(4.0, 0.0),
            &points,
        ));
        assert!(!segment_crosses_convex_hull(
            Vec2::new(0.0, 4.0),
            Vec2::new(4.0, 8.0),
            &points,
        ));
    }

    #[test]
    fn duplicate_points_collapse_to_a_single_point_hull() {
        let points = vec![
            Vec3::new(5.0, 0.0, -2.0),
            Vec3::new(5.0, 3.0, -2.0),
            Vec3::new(5.0, -1.0, -2.0),
        ];
        let hull = plan_convex_hull(&points);
        assert_eq!(hull, vec![Vec2::new(5.0, -2.0)]);
    }

    // --- zero-length segments ----------------------------------------------------

    #[test]
    fn zero_length_segment_inside_the_hull_is_true() {
        let hex = hex_corners();
        let p = Vec2::new(0.0, 0.0);
        assert!(segment_crosses_convex_hull(p, p, &hex));
    }

    #[test]
    fn zero_length_segment_outside_the_hull_is_false() {
        let hex = hex_corners();
        let p = Vec2::new(50.0, 50.0);
        assert!(!segment_crosses_convex_hull(p, p, &hex));
    }

    #[test]
    fn zero_length_segment_on_the_boundary_is_true() {
        let hex = hex_corners();
        // Midpoint of the east edge.
        let p = Vec2::new(7.0, 0.0);
        assert!(segment_crosses_convex_hull(p, p, &hex));
    }

    // --- point_in_convex_plan_hull direct coverage --------------------------------

    #[test]
    fn point_in_convex_plan_hull_matches_the_zero_length_segment_case() {
        let hex = hex_corners();
        assert!(point_in_convex_plan_hull(&hex, Vec2::new(0.0, 0.0)));
        assert!(!point_in_convex_plan_hull(&hex, Vec2::new(50.0, 0.0)));
        assert!(point_in_convex_plan_hull(&hex, Vec2::new(7.0, 4.0))); // vertex
    }

    // --- plan_convex_hull sanity -------------------------------------------------

    #[test]
    fn plan_convex_hull_of_the_hexagon_keeps_all_six_corners() {
        let hex = hex_corners();
        let hull = plan_convex_hull(&hex);
        assert_eq!(
            hull.len(),
            6,
            "a regular hexagon has no redundant/collinear corners"
        );
        for corner in &hex {
            assert!(hull.contains(&Vec2::new(corner.x, corner.z)));
        }
    }

    #[test]
    fn plan_convex_hull_drops_interior_and_projects_x_z_not_x_y() {
        // A point at the hex's plan center but with a huge y (height) must not expand
        // the hull -- projection is (x, z), and interior points are dropped.
        let mut points = hex_corners();
        points.push(Vec3::new(0.0, 1000.0, 0.0));
        let hull = plan_convex_hull(&points);
        assert_eq!(
            hull.len(),
            6,
            "the sky-high interior point must not appear in the hull"
        );
    }
}
