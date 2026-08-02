//! Brush and plane emission: a byte-exact port of `tools/tileforge.py`.
//!
//! The Python is documented as a 1:1 port of `tile_source/geometry.rs`, which
//! means the repository has been carrying two implementations of the same brush
//! math, kept in agreement by hand, with the authored corpus depending on both.
//! That is the largest correctness hazard left in the authoring pipeline, and
//! it is why a parametric builder could never be written in the tool that reads
//! the output.
//!
//! **Byte-exact is the contract, not an aspiration.** The 58 committed `.map`
//! files reproduce from the Python today with a zero-byte diff, so this port is
//! gated the same way: generate, compare, fail. Anything short of that would
//! leave the corpus depending on which implementation last ran.
//!
//! Two places where that contract is subtler than it looks, both load-bearing:
//!
//! - [`fmt`] must match Python's `str(int(v))` / `f"{v:.4f}"` split exactly,
//!   including for negative zero, which both languages print as `0`.
//! - Winding is decided by dot products against an interior hint. Flipping a
//!   comparison produces geometry that still parses, still validates, and is
//!   inside out.

use std::f64::consts::PI;

/// TrenchBroom units per metre.
pub const UPM: f64 = 16.0;
/// One level, in TB units (8 m).
pub const LEVEL: f64 = 128.0;
/// Wall thickness (0.5 m).
pub const WALL: f64 = 8.0;
/// Floor slab thickness (0.5 m).
pub const FLOOR_TOP: f64 = 8.0;
/// A door aperture is 72 units (4.5 m) wide.
pub const DOOR_HALF_WIDTH: f64 = 36.0;
/// Lintel underside: 4 m of clearance above the floor.
pub const DOOR_TOP: f64 = 72.0;
pub const SAFE_INTERIOR_RADIUS: f64 = 104.0;

/// Plan corners in metres, matching `observed_hex::CORNERS`.
pub const CORNERS_PLAN: [(i32, i32); 6] = [(7, -4), (7, 4), (0, 8), (-7, 4), (-7, -4), (0, -8)];

/// Plan corners in TB units. TB y is the negated world z, which is where the
/// sign flip comes from.
#[must_use]
pub fn corners() -> [(f64, f64); 6] {
    let mut out = [(0.0, 0.0); 6];
    for (index, &(x, z)) in CORNERS_PLAN.iter().enumerate() {
        out[index] = (f64::from(x) * UPM, -f64::from(z) * UPM);
    }
    out
}

pub const FACE_NAMES: [&str; 6] = [
    "east",
    "south_east",
    "south_west",
    "west",
    "north_west",
    "north_east",
];

/// Axial neighbour step per lateral face index, matching `observed_hex::HexFace`.
pub const FACE_DELTA: [(i32, i32); 6] = [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)];

pub type P2 = (f64, f64);
pub type P3 = (f64, f64, f64);

#[must_use]
pub fn lerp(a: P2, b: P2, t: f64) -> P2 {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// The two TB corners bounding lateral face `face`, in (A, B) order.
#[must_use]
pub fn edge(face: usize) -> (P2, P2) {
    let corners = corners();
    (corners[face % 6], corners[(face + 1) % 6])
}

#[must_use]
pub fn face_mid(face: usize) -> P2 {
    let (a, b) = edge(face);
    ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
}

/// Move edge `(a, b)` inward - toward the hexagon centre - by `t` units.
#[must_use]
pub fn offset_inward(a: P2, b: P2, t: f64) -> (P2, P2) {
    let d = (b.0 - a.0, b.1 - a.1);
    let length = d.0.hypot(d.1);
    let outward = (-d.1 / length, d.0 / length);
    let sign = if outward.0 * a.0 + outward.1 * a.1 > 0.0 {
        -1.0
    } else {
        1.0
    };
    let m = (outward.0 * sign * t, outward.1 * sign * t);
    ((a.0 + m.0, a.1 + m.1), (b.0 + m.0, b.1 + m.1))
}

/// TB-unit plan origin of footprint cell `(q, r)`.
///
/// Mirrors `expected_port_origin` in `source.rs`: world x = q*14 + r*7 metres,
/// world z = r*12 metres, and TB y is the negated world z.
/// Negated as an **integer** before the conversion, matching Python. Negating
/// after would make `r == 0` produce `-0.0` rather than `0.0`. Both format as
/// `0` today, so nothing in the corpus moves - but the port's contract is
/// byte-exactness under all inputs, not under the ones currently used, and a
/// signed zero entering a comparison later would be a genuinely hard bug to
/// find.
#[must_use]
pub fn cell_origin(q: i32, r: i32) -> P2 {
    (f64::from(q * 14 + r * 7) * UPM, f64::from(-(r * 12)) * UPM)
}

/// Format one coordinate.
///
/// Integral values print without a decimal point, everything else to four
/// places. This is Python's `str(int(v))` / `f"{v:.4f}"` split, and it is on the
/// byte-identity path for every plane in the corpus.
///
/// `int(v)` truncates toward zero in Python and `as i64` does the same in Rust,
/// so `-0.0` takes the integral branch in both and prints `0` rather than `-0`.
#[must_use]
pub fn fmt(v: f64) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let truncated = v as i64;
    #[allow(clippy::cast_precision_loss)]
    if v == truncated as f64 {
        return truncated.to_string();
    }
    format!("{v:.4}")
}

/// One brush plane through three points; the outward normal is
/// `cross(c - a, b - a)`.
#[must_use]
pub fn plane_line(a: P3, b: P3, c: P3) -> String {
    let point = |p: P3| format!("( {} {} {} )", fmt(p.0), fmt(p.1), fmt(p.2));
    format!(
        "{} {} {} __TB_empty 0 0 0 1 1\n",
        point(a),
        point(b),
        point(c)
    )
}

/// A vertical plane through 2D segment `(a2, b2)`, wound so the outward normal
/// points away from `interior_hint`.
#[must_use]
pub fn side_plane(a2: P2, b2: P2, z0: f64, interior_hint: P2) -> String {
    let a = (a2.0, a2.1, z0);
    let b = (b2.0, b2.1, z0);
    let c = (a2.0, a2.1, z0 + 64.0);
    let normal = (-(b2.1 - a2.1), b2.0 - a2.0);
    let toward = (interior_hint.0 - a2.0, interior_hint.1 - a2.1);
    if normal.0 * toward.0 + normal.1 * toward.1 > 0.0 {
        plane_line(a, c, b)
    } else {
        plane_line(a, b, c)
    }
}

#[must_use]
pub fn flat_plane(z: f64, top: bool) -> String {
    let a = (0.0, 0.0, z);
    let b = (64.0, 0.0, z);
    let c = (0.0, 64.0, z);
    if top {
        plane_line(a, c, b)
    } else {
        plane_line(a, b, c)
    }
}

/// A plane through three points, wound so the outward normal points up or down.
#[must_use]
pub fn custom_plane(p0: P3, p1: P3, p2: P3, want_up: bool) -> String {
    let n_z = ((p1.0 - p0.0) * (p2.1 - p0.1)) - ((p1.1 - p0.1) * (p2.0 - p0.0));
    // normal = cross(p2 - p0, p1 - p0); its z component is -n_z
    let up = -n_z > 0.0;
    if up == want_up {
        plane_line(p0, p1, p2)
    } else {
        plane_line(p0, p2, p1)
    }
}

/// A plane through three points, wound so the outward normal points away from
/// `interior`. The general-orientation helper for chamfer and bevel planes.
#[must_use]
pub fn plane3(p0: P3, p1: P3, p2: P3, interior: P3) -> String {
    let u = (p1.0 - p0.0, p1.1 - p0.1, p1.2 - p0.2);
    let v = (p2.0 - p0.0, p2.1 - p0.1, p2.2 - p0.2);
    // Emission convention: outward normal = cross(C - A, B - A).
    let n = (
        v.1 * u.2 - v.2 * u.1,
        v.2 * u.0 - v.0 * u.2,
        v.0 * u.1 - v.1 * u.0,
    );
    let toward = (interior.0 - p0.0, interior.1 - p0.1, interior.2 - p0.2);
    if n.0 * toward.0 + n.1 * toward.1 + n.2 * toward.2 > 0.0 {
        plane_line(p0, p2, p1)
    } else {
        plane_line(p0, p1, p2)
    }
}

#[must_use]
pub fn centroid(corners: &[P2]) -> P2 {
    #[allow(clippy::cast_precision_loss)]
    let count = corners.len() as f64;
    (
        corners.iter().map(|p| p.0).sum::<f64>() / count,
        corners.iter().map(|p| p.1).sum::<f64>() / count,
    )
}

/// Chamfer planes around a convex plan polygon's horizontal rim.
///
/// Extra planes on a convex brush cost nothing, and this is how tiles get
/// smooth transitions instead of hard assembled-block edges. Inward is judged
/// against the polygon's own centroid, so off-centre decks and parapets chamfer
/// correctly.
#[must_use]
pub fn rim_chamfers(
    corners: &[P2],
    z_edge: f64,
    z_face: f64,
    chamfer: f64,
    interior: P3,
) -> String {
    let mut out = String::new();
    let reference = centroid(corners);
    let n = corners.len();
    for index in 0..n {
        let (a, b) = (corners[index], corners[(index + 1) % n]);
        let d = (b.0 - a.0, b.1 - a.1);
        let length = d.0.hypot(d.1);
        let normal = (-d.1 / length, d.0 / length);
        let to_ref = (reference.0 - a.0, reference.1 - a.1);
        let sign = if normal.0 * to_ref.0 + normal.1 * to_ref.1 > 0.0 {
            1.0
        } else {
            -1.0
        };
        let inward = (normal.0 * sign * chamfer, normal.1 * sign * chamfer);
        let mid = ((a.0 + b.0) * 0.5 + inward.0, (a.1 + b.1) * 0.5 + inward.1);
        out.push_str(&plane3(
            (a.0, a.1, z_edge),
            (b.0, b.1, z_edge),
            (mid.0, mid.1, z_face),
            interior,
        ));
    }
    out
}

/// A convex vertical prism from a plan polygon in perimeter order.
#[must_use]
pub fn prism(
    corners: &[P2],
    z0: f64,
    z1: f64,
    hint: Option<P2>,
    chamfer_top: f64,
    chamfer_bottom: f64,
) -> String {
    let hint = hint.unwrap_or_else(|| centroid(corners));
    let interior = (hint.0, hint.1, (z0 + z1) * 0.5);
    let mut out = String::from("{\n");
    let n = corners.len();
    for index in 0..n {
        out.push_str(&side_plane(
            corners[index],
            corners[(index + 1) % n],
            z0,
            hint,
        ));
    }
    out.push_str(&flat_plane(z0, false));
    out.push_str(&flat_plane(z1, true));
    if chamfer_top > 0.0 {
        out.push_str(&rim_chamfers(
            corners,
            z1 - chamfer_top,
            z1,
            chamfer_top,
            interior,
        ));
    }
    if chamfer_bottom > 0.0 {
        out.push_str(&rim_chamfers(
            corners,
            z0 + chamfer_bottom,
            z0,
            chamfer_bottom,
            interior,
        ));
    }
    out.push_str("}\n");
    out
}

/// A convex prism with a flat bottom at `z0` and a slanted top plane.
#[must_use]
pub fn sloped_prism(corners: &[P2], z0: f64, top_points: [P3; 3], hint: Option<P2>) -> String {
    let hint = hint.unwrap_or_else(|| centroid(corners));
    let mut out = String::from("{\n");
    let n = corners.len();
    for index in 0..n {
        out.push_str(&side_plane(
            corners[index],
            corners[(index + 1) % n],
            z0,
            hint,
        ));
    }
    out.push_str(&flat_plane(z0, false));
    out.push_str(&custom_plane(
        top_points[0],
        top_points[1],
        top_points[2],
        true,
    ));
    out.push_str("}\n");
    out
}

#[must_use]
pub fn boxed(min3: P3, max3: P3) -> String {
    let corners = [
        (min3.0, min3.1),
        (max3.0, min3.1),
        (max3.0, max3.1),
        (min3.0, max3.1),
    ];
    prism(&corners, min3.2, max3.2, None, 0.0, 0.0)
}

/// A full-footprint hexagonal slab: floor or ceiling.
#[must_use]
pub fn hex_slab(z0: f64, z1: f64, chamfer_top: f64, chamfer_bottom: f64) -> String {
    let interior = (0.0, 0.0, (z0 + z1) * 0.5);
    let mut out = String::from("{\n");
    for face in 0..6 {
        let (a, b) = edge(face);
        out.push_str(&side_plane(a, b, z0, (0.0, 0.0)));
    }
    out.push_str(&flat_plane(z0, false));
    out.push_str(&flat_plane(z1, true));
    let corners = corners();
    if chamfer_top > 0.0 {
        out.push_str(&rim_chamfers(
            &corners,
            z1 - chamfer_top,
            z1,
            chamfer_top,
            interior,
        ));
    }
    if chamfer_bottom > 0.0 {
        out.push_str(&rim_chamfers(
            &corners,
            z0 + chamfer_bottom,
            z0,
            chamfer_bottom,
            interior,
        ));
    }
    out.push_str("}\n");
    out
}

/// A band hugging `face` between inward offsets `t0..t1`, mitered against the
/// neighbouring faces so bands at matching offsets meet cleanly.
#[must_use]
pub fn band(face: usize, t0: f64, t1: f64, z0: f64, z1: f64) -> String {
    let (a, b) = edge(face);
    let (oa, ob) = offset_inward(a, b, t0);
    let (ia, ib) = offset_inward(a, b, t1);
    let (pa0, pb0) = edge((face + 5) % 6);
    let (na0, nb0) = edge((face + 1) % 6);
    let (pa, pb) = offset_inward(pa0, pb0, t0);
    let (na, nb) = offset_inward(na0, nb0, t0);
    let mid = (
        (oa.0 + ob.0 + ia.0 + ib.0) * 0.25,
        (oa.1 + ob.1 + ia.1 + ib.1) * 0.25,
    );
    let mut out = String::from("{\n");
    out.push_str(&side_plane(oa, ob, z0, (0.0, 0.0)));
    out.push_str(&side_plane(
        ia,
        ib,
        z0,
        ((oa.0 + ob.0) * 0.5, (oa.1 + ob.1) * 0.5),
    ));
    out.push_str(&side_plane(pa, pb, z0, mid));
    out.push_str(&side_plane(na, nb, z0, mid));
    out.push_str(&flat_plane(z0, false));
    out.push_str(&flat_plane(z1, true));
    out.push_str("}\n");
    out
}

/// A full sealed wall on `face`, mitered against its neighbours.
#[must_use]
pub fn wall(face: usize, z0: f64, z1: f64) -> String {
    band(face, 0.0, WALL, z0, z1)
}

/// A doorway wall on `face`.
///
/// Two jambs beside the canonical 4.5 m opening, a lintel wall above `top`, and
/// a sill band below `sill`. The aperture **at the seam plane** stays exactly
/// canonical - 72 wide, `sill..top` - so tiles keep matching across seams;
/// `splay` widens only the *interior* reveal so the opening reads as a shaped
/// portal instead of a punched hole, and `lintel_bevel` chamfers the lintel
/// soffit's inner edge.
#[must_use]
pub fn door_wall(
    face: usize,
    z0: f64,
    z1: f64,
    sill: f64,
    top: f64,
    splay: f64,
    lintel_bevel: f64,
) -> String {
    let (a, b) = edge(face);
    let (ia, ib) = offset_inward(a, b, WALL);
    let inward = (ia.0 - a.0, ia.1 - a.1);
    let d = (b.0 - a.0, b.1 - a.1);
    let length = d.0.hypot(d.1);
    let u = (d.0 / length, d.1 / length);
    let mid = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let da = (mid.0 - u.0 * DOOR_HALF_WIDTH, mid.1 - u.1 * DOOR_HALF_WIDTH);
    let db = (mid.0 + u.0 * DOOR_HALF_WIDTH, mid.1 + u.1 * DOOR_HALF_WIDTH);
    let (ida, idb) = offset_inward(da, db, WALL);
    // Splayed reveal: the interior aperture corners slide away from the door
    // centre, angling the reveal faces.
    let ida = (ida.0 - u.0 * splay, ida.1 - u.1 * splay);
    let idb = (idb.0 + u.0 * splay, idb.1 + u.1 * splay);

    let mut out = String::new();
    let left_hint = (
        (a.0 + da.0) * 0.5 + inward.0 * 0.5,
        (a.1 + da.1) * 0.5 + inward.1 * 0.5,
    );
    out.push_str(&prism(
        &[a, da, ida, ia],
        z0,
        top,
        Some(left_hint),
        0.0,
        0.0,
    ));
    let right_hint = (
        (db.0 + b.0) * 0.5 + inward.0 * 0.5,
        (db.1 + b.1) * 0.5 + inward.1 * 0.5,
    );
    out.push_str(&prism(
        &[db, b, ib, idb],
        z0,
        top,
        Some(right_hint),
        0.0,
        0.0,
    ));

    let wall_hint = (mid.0 + inward.0 * 0.5, mid.1 + inward.1 * 0.5);
    if top < z1 {
        // Lintel wall with a bevelled soffit edge on the interior side.
        let interior = (wall_hint.0, wall_hint.1, (top + z1) * 0.5);
        let mut lintel = String::from("{\n");
        for seg in [(a, b), (ib, ia)] {
            lintel.push_str(&side_plane(seg.0, seg.1, top, wall_hint));
        }
        lintel.push_str(&side_plane(a, ia, top, wall_hint));
        lintel.push_str(&side_plane(b, ib, top, wall_hint));
        lintel.push_str(&flat_plane(top, false));
        lintel.push_str(&flat_plane(z1, true));
        if lintel_bevel > 0.0 {
            let inset = offset_inward(a, b, WALL - lintel_bevel * 0.5);
            let bevel_mid = ((inset.0.0 + inset.1.0) * 0.5, (inset.0.1 + inset.1.1) * 0.5);
            lintel.push_str(&plane3(
                (ia.0, ia.1, top + lintel_bevel),
                (ib.0, ib.1, top + lintel_bevel),
                (bevel_mid.0, bevel_mid.1, top),
                interior,
            ));
        }
        lintel.push_str("}\n");
        out.push_str(&lintel);
    }
    if sill > z0 {
        out.push_str(&prism(&[a, b, ib, ia], z0, sill, Some(wall_hint), 0.0, 0.0));
    }
    out
}

/// [`door_wall`] with the canonical sill, lintel, splay, and bevel.
#[must_use]
pub fn door_wall_default(face: usize, z0: f64, z1: f64) -> String {
    door_wall(face, z0, z1, FLOOR_TOP, DOOR_TOP, 10.0, 8.0)
}

/// A regular hexagonal pylon centred on the origin.
#[must_use]
pub fn pylon(
    radius: f64,
    z0: f64,
    z1: f64,
    phase_deg: f64,
    chamfer_top: f64,
    chamfer_bottom: f64,
) -> String {
    let corners: Vec<P2> = (0..6)
        .map(|index| {
            #[allow(clippy::cast_precision_loss)]
            let angle = (phase_deg + index as f64 * 60.0).to_radians();
            (radius * angle.cos(), radius * angle.sin())
        })
        .collect();
    prism(
        &corners,
        z0,
        z1,
        Some((0.0, 0.0)),
        chamfer_top,
        chamfer_bottom,
    )
}

/// Shift every brush plane coordinate triple by `(dx, dy, dz)`.
///
/// `plane_line` is the only emitter of parenthesised coordinate triples and its
/// format is fixed, so a textual shift is exact. Only ever pass worldspawn
/// *brush* text: point entities carry their origin as a quoted property, which
/// this deliberately does not touch.
#[must_use]
pub fn translate(brushes: &str, dx: f64, dy: f64, dz: f64) -> String {
    let mut out = String::with_capacity(brushes.len());
    let bytes: Vec<char> = brushes.chars().collect();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == '('
            && let Some((triple, next)) = parse_triple(&bytes, index)
        {
            out.push_str(&format!(
                "( {} {} {} )",
                fmt(triple.0 + dx),
                fmt(triple.1 + dy),
                fmt(triple.2 + dz)
            ));
            index = next;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    out
}

/// Match Python's `\(\s*(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s*\)` at `start`.
fn parse_triple(chars: &[char], start: usize) -> Option<(P3, usize)> {
    let mut index = start + 1;
    let mut values = [0.0_f64; 3];
    for (slot, value) in values.iter_mut().enumerate() {
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        let begin = index;
        if index < chars.len() && chars[index] == '-' {
            index += 1;
        }
        while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.') {
            index += 1;
        }
        if index == begin {
            return None;
        }
        *value = chars[begin..index]
            .iter()
            .collect::<String>()
            .parse()
            .ok()?;
        // The first two numbers must be followed by whitespace, as `\s+`
        // requires; the third by optional whitespace and a close paren.
        if slot < 2 {
            if index >= chars.len() || !chars[index].is_whitespace() {
                return None;
            }
        } else {
            while index < chars.len() && chars[index].is_whitespace() {
                index += 1;
            }
            if index >= chars.len() || chars[index] != ')' {
                return None;
            }
            index += 1;
        }
    }
    Some(((values[0], values[1], values[2]), index))
}

/// A regular polygon in plan, `phase_deg` degrees off the +x axis.
#[must_use]
pub fn regular_polygon(radius: f64, sides: usize, phase_deg: f64) -> Vec<P2> {
    (0..sides)
        .map(|index| {
            #[allow(clippy::cast_precision_loss)]
            let angle = phase_deg.to_radians() + 2.0 * PI * index as f64 / sides as f64;
            (radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The formatter is on the byte-identity path for every plane in the
    /// corpus, so its exact split is pinned rather than assumed.
    #[test]
    fn the_formatter_matches_pythons_integral_split() {
        assert_eq!(fmt(0.0), "0");
        assert_eq!(fmt(-0.0), "0", "negative zero must not print a sign");
        assert_eq!(fmt(112.0), "112");
        assert_eq!(fmt(-64.0), "-64");
        assert_eq!(fmt(0.5), "0.5000");
        assert_eq!(fmt(-13.8564), "-13.8564");
        assert_eq!(fmt(1.0 / 3.0), "0.3333");
    }

    /// The corner table is the same quantized hexagon the importer validates
    /// against, with TB's y being world z negated.
    #[test]
    fn the_corner_table_is_the_quantized_hexagon_in_tb_units() {
        let corners = corners();
        assert_eq!(corners[0], (112.0, 64.0));
        assert_eq!(corners[2], (0.0, -128.0));
        for (index, corner) in corners.iter().enumerate() {
            let plan = CORNERS_PLAN[index];
            assert!((corner.0 - f64::from(plan.0) * UPM).abs() < 1e-9);
            assert!((corner.1 + f64::from(plan.1) * UPM).abs() < 1e-9);
        }
    }

    /// Translation is textual, so it must move brush coordinates and leave a
    /// quoted origin property alone. Getting this wrong silently relocates
    /// every port in a multi-cell room.
    #[test]
    fn translation_moves_brush_planes_and_not_quoted_properties() {
        let brush = plane_line((1.0, 2.0, 3.0), (4.0, 5.0, 6.0), (7.0, 8.0, 9.0));
        let moved = translate(&brush, 10.0, 0.0, 0.0);
        assert!(moved.starts_with("( 11 2 3 )"), "{moved}");
        assert!(moved.contains("( 14 5 6 )"), "{moved}");

        let property = "\"origin\" \"112 0 48\"\n";
        assert_eq!(
            translate(property, 10.0, 0.0, 0.0),
            property,
            "a quoted property is not a brush plane"
        );
    }

    /// A prism must be closed: one plane per side, plus a floor and a ceiling.
    #[test]
    fn a_prism_emits_one_plane_per_side_plus_two_caps() {
        let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let text = prism(&square, 0.0, 8.0, None, 0.0, 0.0);
        assert_eq!(text.lines().filter(|l| l.contains("__TB_empty")).count(), 6);
        assert!(text.starts_with("{\n") && text.ends_with("}\n"));
    }

    /// Chamfers add planes without opening the brush, which is the trick that
    /// lets a single convex brush have soft edges.
    #[test]
    fn chamfers_add_planes_to_the_same_brush() {
        let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let plain = prism(&square, 0.0, 8.0, None, 0.0, 0.0);
        let chamfered = prism(&square, 0.0, 8.0, None, 2.0, 0.0);
        let planes = |text: &str| text.lines().filter(|l| l.contains("__TB_empty")).count();
        assert_eq!(planes(&chamfered), planes(&plain) + 4);
        assert_eq!(chamfered.matches('{').count(), 1, "still one brush");
    }

    /// Winding is decided by a dot product against an interior hint, and a
    /// flipped comparison yields geometry that parses, validates, and is inside
    /// out. Pinned by requiring the two hints to disagree.
    #[test]
    fn side_plane_winding_follows_the_interior_hint() {
        let a = (0.0, 0.0);
        let b = (10.0, 0.0);
        let left = side_plane(a, b, 0.0, (5.0, 5.0));
        let right = side_plane(a, b, 0.0, (5.0, -5.0));
        assert_ne!(left, right, "the hint must decide the winding");
    }
}
