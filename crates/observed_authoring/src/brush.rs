//! Convex brush math: Quake half-space triples → deduplicated vertex clouds.
//!
//! A `.map` brush is a convex solid described by bounding planes, each plane
//! given as three points whose winding fixes the outward direction. Vertices
//! are every triple-plane intersection that lies inside (or on) all
//! half-spaces. All math is `f64`; authored planes are integer-coordinate, so
//! results are exact to well below the tolerances used here.

use quake_map::Brush;

/// Inside/on-boundary tolerance in TrenchBroom units (1/16 mm at 16 u/m).
const EPSILON: f64 = 1.0e-3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: [f64; 3],
    /// Outward half-space: `dot(normal, p) <= d` is inside.
    pub d: f64,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Quake winding: for points `(a b c)` the outward normal is
/// `cross(c - a, b - a)`.
pub fn plane_of(half_space: &[[f64; 3]; 3]) -> Plane {
    let [a, b, c] = *half_space;
    let normal = cross(sub(c, a), sub(b, a));
    Plane {
        normal,
        d: dot(normal, a),
    }
}

fn intersect(p1: Plane, p2: Plane, p3: Plane) -> Option<[f64; 3]> {
    // Solve [n1; n2; n3] x = [d1; d2; d3] by Cramer's rule.
    let det = dot(p1.normal, cross(p2.normal, p3.normal));
    if det.abs() < 1.0e-9 {
        return None;
    }
    let term = |scale: f64, a: [f64; 3], b: [f64; 3]| {
        let c = cross(a, b);
        [scale * c[0], scale * c[1], scale * c[2]]
    };
    let t1 = term(p1.d, p2.normal, p3.normal);
    let t2 = term(p2.d, p3.normal, p1.normal);
    let t3 = term(p3.d, p1.normal, p2.normal);
    Some([
        (t1[0] + t2[0] + t3[0]) / det,
        (t1[1] + t2[1] + t3[1]) / det,
        (t1[2] + t2[2] + t3[2]) / det,
    ])
}

/// The vertex cloud of a convex brush in TrenchBroom units, deduplicated and
/// deterministically ordered. `None` when the brush is degenerate (fewer than
/// four distinct vertices).
pub fn brush_vertices(brush: &Brush) -> Option<Vec<[f64; 3]>> {
    let planes: Vec<Plane> = brush
        .iter()
        .map(|surface| plane_of(&surface.half_space))
        .collect();
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    for i in 0..planes.len() {
        for j in (i + 1)..planes.len() {
            for k in (j + 1)..planes.len() {
                let Some(point) = intersect(planes[i], planes[j], planes[k]) else {
                    continue;
                };
                // Normalize the inside test by plane magnitude so tolerance
                // is in units, not scaled by unnormalized normals.
                let inside = planes.iter().all(|plane| {
                    let magnitude = dot(plane.normal, plane.normal).sqrt().max(1.0e-9);
                    (dot(plane.normal, point) - plane.d) / magnitude <= EPSILON
                });
                if !inside {
                    continue;
                }
                let duplicate = vertices.iter().any(|other| {
                    (other[0] - point[0]).abs() < EPSILON
                        && (other[1] - point[1]).abs() < EPSILON
                        && (other[2] - point[2]).abs() < EPSILON
                });
                if !duplicate {
                    vertices.push(point);
                }
            }
        }
    }
    if vertices.len() < 4 {
        return None;
    }
    vertices.sort_by(|a, b| {
        a.partial_cmp(b)
            .expect("brush vertices are finite by construction")
    });
    Some(vertices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile_source::box_brush_text;

    fn parse_single_brush(text: &str) -> Brush {
        let map = quake_map::parse(&mut std::io::Cursor::new(format!(
            "{{\n\"classname\" \"worldspawn\"\n{text}}}\n"
        )))
        .expect("test brush parses");
        map.entities[0].brushes[0].clone()
    }

    #[test]
    fn a_box_brush_yields_its_eight_corners() {
        let brush = parse_single_brush(&box_brush_text([0, 0, 0], [32, 64, 128]));
        let vertices = brush_vertices(&brush).expect("box is not degenerate");
        assert_eq!(vertices.len(), 8);
        for corner in [
            [0.0, 0.0, 0.0],
            [32.0, 64.0, 128.0],
            [0.0, 64.0, 128.0],
            [32.0, 0.0, 0.0],
        ] {
            assert!(
                vertices.iter().any(|v| v
                    .iter()
                    .zip(corner.iter())
                    .all(|(a, b)| (a - b).abs() < 1.0e-6)),
                "missing corner {corner:?} in {vertices:?}"
            );
        }
    }

    #[test]
    fn a_degenerate_brush_is_rejected() {
        // Two parallel plane pairs only — no closed solid.
        let text = "{\n( 0 0 0 ) ( 0 1 0 ) ( 0 0 1 ) __TB_empty 0 0 0 1 1\n( 32 0 0 ) ( 32 0 1 ) ( 32 1 0 ) __TB_empty 0 0 0 1 1\n( 0 0 0 ) ( 1 0 0 ) ( 0 0 1 ) __TB_empty 0 0 0 1 1\n( 0 64 0 ) ( 0 64 1 ) ( 1 64 0 ) __TB_empty 0 0 0 1 1\n}\n";
        let brush = parse_single_brush(text);
        assert_eq!(brush_vertices(&brush), None);
    }
}
