//! Brush-text math for the typed tile generator: planes, prisms, walls,
//! doorways, and the interior-detail primitives (trim bands, pillars, pylons).
//!
//! Coordinates are TrenchBroom units (Z-up, [`crate::UNITS_PER_METER`] per
//! meter); the canonical hexagon corners land on integers by construction.
//! Interior detail that gets rotated by a face angle must stay inside
//! [`SAFE_INTERIOR_RADIUS`] — the quantized hexagon is not perfectly regular,
//! so only that disk survives every rotation without escaping the footprint.

use observed_hex::{HexFace, TILE_LEVEL_HEIGHT, face_edge};

use crate::UNITS_PER_METER;

/// Wall thickness in TB units (0.5 m).
pub(crate) const WALL: f64 = 8.0;
/// Doorway half-width (4.5 m opening) and lintel height (4 m clearance).
pub(crate) const DOOR_HALF_WIDTH: f64 = 36.0;
pub(crate) const DOOR_TOP: f64 = 72.0;
/// Floor slab thickness (0.5 m).
pub(crate) const FLOOR_TOP: f64 = 8.0;
/// Largest disk that fits the quantized hexagon interior with margin: the
/// diagonal faces sit ~111.1 units from center, so rotated interior geometry
/// must keep every vertex within this radius.
pub(crate) const SAFE_INTERIOR_RADIUS: f64 = 104.0;

/// One level's height in TrenchBroom units (128).
pub(crate) fn level_units() -> f64 {
    f64::from(TILE_LEVEL_HEIGHT) * UNITS_PER_METER
}

/// Canonical corner in TB units for a lateral face's edge (A, B order).
pub(crate) fn tb_edge(face: HexFace) -> ([f64; 2], [f64; 2]) {
    let [a, b] = face_edge(face);
    let convert = |(x, z): (i32, i32)| {
        [
            f64::from(x) * UNITS_PER_METER,
            f64::from(-z) * UNITS_PER_METER,
        ]
    };
    (convert(a), convert(b))
}

/// Angle (degrees) of the face's outward midpoint in the TB plane. Used to
/// rotate authored interior detail into a door face's frame.
pub(crate) fn face_angle_deg(face: HexFace) -> f64 {
    let (a, b) = tb_edge(face);
    let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    mid[1].atan2(mid[0]).to_degrees()
}

fn fmt(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.4}")
    }
}

pub(crate) fn plane_line(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> String {
    format!(
        "( {} {} {} ) ( {} {} {} ) ( {} {} {} ) __TB_empty 0 0 0 1 1\n",
        fmt(a[0]),
        fmt(a[1]),
        fmt(a[2]),
        fmt(b[0]),
        fmt(b[1]),
        fmt(b[2]),
        fmt(c[0]),
        fmt(c[1]),
        fmt(c[2])
    )
}

/// Plane through the vertical quad (a2, b2) x [z0, z0+64], wound so the
/// outward normal points away from `interior_hint` (a 2D point on the solid
/// side of the plane).
pub(crate) fn side_plane(a2: [f64; 2], b2: [f64; 2], z0: f64, interior_hint: [f64; 2]) -> String {
    let a = [a2[0], a2[1], z0];
    let b = [b2[0], b2[1], z0];
    let c = [a2[0], a2[1], z0 + 64.0];
    // normal = cross(c - a, b - a) = (-h*dy, h*dx, 0) with d = b - a.
    let normal = [-(b2[1] - a2[1]), b2[0] - a2[0]];
    let toward_hint = [interior_hint[0] - a2[0], interior_hint[1] - a2[1]];
    if normal[0] * toward_hint[0] + normal[1] * toward_hint[1] > 0.0 {
        // Normal points inward — flip winding.
        plane_line(a, c, b)
    } else {
        plane_line(a, b, c)
    }
}

/// Horizontal plane at `z`, outward normal up (`top = true`) or down.
pub(crate) fn flat_plane(z: f64, top: bool) -> String {
    let a = [0.0, 0.0, z];
    let b = [64.0, 0.0, z];
    let c = [0.0, 64.0, z];
    // cross(c-a, b-a) = (0,0,-64*64) points down; (a b c) order is down.
    if top {
        plane_line(a, c, b)
    } else {
        plane_line(a, b, c)
    }
}

/// A full-footprint hexagonal slab from z0 to z1 (floor/ceiling).
pub(crate) fn hex_slab_brush(z0: f64, z1: f64) -> String {
    let mut out = String::from("{\n");
    for face in HexFace::LATERAL {
        let (a, b) = tb_edge(face);
        out += &side_plane(a, b, z0, [0.0, 0.0]);
    }
    out += &flat_plane(z0, false);
    out += &flat_plane(z1, true);
    out += "}\n";
    out
}

/// Inward offset of a 2D edge by `t` units (both endpoints moved along the
/// inward normal, "inward" judged against the hexagon center at the origin).
pub(crate) fn offset_inward(a: [f64; 2], b: [f64; 2], t: f64) -> ([f64; 2], [f64; 2]) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let length = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let outward = [-d[1] / length, d[0] / length];
    // The hexagon center is the origin; outward has positive dot with a.
    let sign = if outward[0] * a[0] + outward[1] * a[1] > 0.0 {
        -1.0
    } else {
        1.0
    };
    let m = [outward[0] * sign * t, outward[1] * sign * t];
    ([a[0] + m[0], a[1] + m[1]], [b[0] + m[0], b[1] + m[1]])
}

/// A convex prism from a plan-view polygon (corners in perimeter order).
pub(crate) fn general_prism_brush(
    corners: &[[f64; 2]],
    z0: f64,
    z1: f64,
    interior_hint: [f64; 2],
) -> String {
    let mut out = String::from("{\n");
    let n = corners.len();
    for i in 0..n {
        out += &side_plane(corners[i], corners[(i + 1) % n], z0, interior_hint);
    }
    out += &flat_plane(z0, false);
    out += &flat_plane(z1, true);
    out += "}\n";
    out
}

/// A band hugging `face` between inward offsets `t0..t1`, mitered against the
/// neighbor faces at offset `t0` (so bands at matching offsets meet cleanly).
pub(crate) fn band_brush(face: HexFace, t0: f64, t1: f64, z0: f64, z1: f64) -> String {
    let (a, b) = tb_edge(face);
    let (oa, ob) = offset_inward(a, b, t0);
    let (ia, ib) = offset_inward(a, b, t1);
    let lateral = HexFace::LATERAL;
    let prev = lateral[(face.index() + 5) % 6];
    let next = lateral[(face.index() + 1) % 6];
    let (pa0, pb0) = tb_edge(prev);
    let (na0, nb0) = tb_edge(next);
    let (pa, pb) = offset_inward(pa0, pb0, t0);
    let (na, nb) = offset_inward(na0, nb0, t0);
    let mid = [
        (oa[0] + ob[0] + ia[0] + ib[0]) * 0.25,
        (oa[1] + ob[1] + ia[1] + ib[1]) * 0.25,
    ];
    let mut out = String::from("{\n");
    out += &side_plane(oa, ob, z0, [0.0, 0.0]); // outer face
    out += &side_plane(ia, ib, z0, [(oa[0] + ob[0]) * 0.5, (oa[1] + ob[1]) * 0.5]); // inner
    out += &side_plane(pa, pb, z0, mid); // miter at corner A
    out += &side_plane(na, nb, z0, mid); // miter at corner B
    out += &flat_plane(z0, false);
    out += &flat_plane(z1, true);
    out += "}\n";
    out
}

/// A full sealed wall on `face`, mitered against its neighbor face planes.
pub(crate) fn wall_brush(face: HexFace, z0: f64, z1: f64) -> String {
    band_brush(face, 0.0, WALL, z0, z1)
}

pub(crate) fn box_brush(min: [f64; 3], max: [f64; 3]) -> String {
    let mut out = String::from("{\n");
    out += &side_plane([max[0], min[1]], [max[0], max[1]], min[2], [min[0], min[1]]);
    out += &side_plane([min[0], min[1]], [min[0], max[1]], min[2], [max[0], max[1]]);
    out += &side_plane([min[0], max[1]], [max[0], max[1]], min[2], [min[0], min[1]]);
    out += &side_plane([min[0], min[1]], [max[0], min[1]], min[2], [max[0], max[1]]);
    out += &flat_plane(min[2], false);
    out += &flat_plane(max[2], true);
    out += "}\n";
    out
}

/// Integer box brush text (public for brush-math tests).
pub fn box_brush_text(min: [i32; 3], max: [i32; 3]) -> String {
    box_brush(min.map(f64::from), max.map(f64::from))
}

/// An axis-aligned square pillar centered at `center` with half-width `half`.
pub(crate) fn pillar_brush(center: [f64; 2], half: f64, z0: f64, z1: f64) -> String {
    box_brush(
        [center[0] - half, center[1] - half, z0],
        [center[0] + half, center[1] + half, z1],
    )
}

/// A hexagonal pylon of `radius` centered at the origin.
pub(crate) fn pylon_brush(radius: f64, z0: f64, z1: f64) -> String {
    general_prism_brush(
        &column_plan(ColumnForm::Round, radius, 0.0),
        z0,
        z1,
        [0.0, 0.0],
    )
}

/// What a support *is*, in plan.
///
/// A district's character is mostly carried by its supports — they are the
/// thing you stand next to. Before this there was one column shape in the whole
/// facility and ten districts differed by its radius, which is why they read as
/// one building.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColumnForm {
    /// A hexagonal prism: the historical pylon, and still the neutral reading.
    #[default]
    Round,
    /// A square pier, aligned to the cell frame.
    Square,
    /// A twelve-sided shaft with alternating radii, so it catches light in
    /// facets rather than a single sweep.
    Faceted,
}

/// The plan polygon of a support, centered at `center`.
///
/// **Every form covers the same floor area for a given `radius`.** A square of
/// half-width `r` is 54% larger than a hexagon of radius `r`, and swapping one
/// for the other in the middle of a junction narrowed the crossing enough to
/// wedge bots against it — caught by `bot_soak_has_no_stalls`, which is exactly
/// what that gate is for. Form is a look; it must not be a floorplan change.
pub(crate) fn column_plan(form: ColumnForm, radius: f64, rotation_deg: f64) -> Vec<[f64; 2]> {
    // Hexagon area is `3*sqrt(3)/2 * r^2`; a square of half-width `h` is
    // `4h^2`. Equal area at `h = r * sqrt(3*sqrt(3)/8)`.
    const SQUARE_EQUAL_AREA: f64 = 0.805_927_0;
    let corners: Vec<[f64; 2]> = match form {
        ColumnForm::Round => (0..6)
            .map(|i| {
                let angle = f64::from(i) * 60.0f64.to_radians();
                [radius * angle.cos(), radius * angle.sin()]
            })
            .collect(),
        ColumnForm::Square => {
            let h = radius * SQUARE_EQUAL_AREA;
            vec![[-h, -h], [h, -h], [h, h], [-h, h]]
        }
        ColumnForm::Faceted => (0..12)
            .map(|i| {
                let angle = f64::from(i) * 30.0f64.to_radians();
                // Alternating radii: the flutes that make a facet read.
                let r = if i % 2 == 0 { radius } else { radius * 0.78 };
                [r * angle.cos(), r * angle.sin()]
            })
            .collect(),
    };
    if rotation_deg.abs() < f64::EPSILON {
        corners
    } else {
        rotate_points(&corners, rotation_deg)
    }
}

/// Inboard ceiling relief. Never touches the boundary plane, so it cannot move
/// a face signature — see the seam note on [`open_span`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CeilingForm {
    /// The historical flat slab.
    #[default]
    Flat,
    /// A recessed centre panel: one step up into the slab.
    Coffered,
    /// A lattice of ribs below the slab, reading as an open grid.
    Grid,
}

/// Ceiling relief hung under the slab at `ceiling_z`, inboard of `reach`.
pub(crate) fn ceiling_relief(form: CeilingForm, ceiling_z: f64, reach: f64) -> String {
    match form {
        CeilingForm::Flat => String::new(),
        CeilingForm::Coffered => {
            // A single recessed panel: a ring of mass around an open centre.
            let outer = reach;
            let inner = reach * 0.55;
            let mut out = String::new();
            for (i, sign) in [(0usize, 1.0f64), (1, -1.0)] {
                let _ = i;
                out += &general_prism_brush(
                    &[
                        [-outer, sign * inner],
                        [outer, sign * inner],
                        [outer, sign * outer],
                        [-outer, sign * outer],
                    ],
                    ceiling_z - 10.0,
                    ceiling_z,
                    [0.0, sign * (inner + outer) * 0.5],
                );
            }
            out
        }
        CeilingForm::Grid => {
            // Ribs both ways: an overlit district reads its ceiling as a grid.
            let mut out = String::new();
            for offset in [-48.0, 0.0, 48.0] {
                out += &general_prism_brush(
                    &[
                        [-reach, offset - 4.0],
                        [reach, offset - 4.0],
                        [reach, offset + 4.0],
                        [-reach, offset + 4.0],
                    ],
                    ceiling_z - 8.0,
                    ceiling_z,
                    [0.0, offset],
                );
            }
            out
        }
    }
}

/// A support of `form` at `center`, from `z0` to `z1`.
pub(crate) fn column_brush(
    form: ColumnForm,
    center: [f64; 2],
    radius: f64,
    z0: f64,
    z1: f64,
) -> String {
    let plan: Vec<[f64; 2]> = column_plan(form, radius, 0.0)
        .into_iter()
        .map(|corner| [corner[0] + center[0], corner[1] + center[1]])
        .collect();
    general_prism_brush(&plan, z0, z1, center)
}

pub(crate) fn rotate_points(corners: &[[f64; 2]], angle_deg: f64) -> Vec<[f64; 2]> {
    let rad = angle_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    corners
        .iter()
        .map(|&[x, y]| [x * cos - y * sin, x * sin + y * cos])
        .collect()
}

/// A face left open across its **whole width**: the lintel band above `top`
/// and the sill band below `sill`, and nothing between them.
///
/// [`door_wall`] with its jambs removed. That difference is the whole of what
/// makes an expanse an expanse: a cell that opens through a `DOOR_HALF_WIDTH`
/// aperture is a room with a doorway, and a run of them is a honeycomb of rooms
/// with doorways however open the topology says they are.
///
/// The seam contract is untouched, and deliberately so.
/// `seam_auditor::sample_face_signature` takes the min and max Y of geometry
/// touching the face plane; the lintel and sill still touch it at exactly the
/// heights the jambs did, so `floor_height` and `headroom` are bit-identical to
/// the doorway's. An open span therefore mates with an ordinary hall door on
/// the same terms — which it must, because an expanse borders halls far more
/// often than it borders another expanse.
pub(crate) fn open_span(face: HexFace, z0: f64, z1: f64, sill: f64, top: f64) -> String {
    let (a, b) = tb_edge(face);
    let (ia, ib) = offset_inward(a, b, WALL);
    let inward = [ia[0] - a[0], ia[1] - a[1]];
    let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let hint = [mid[0] + inward[0] * 0.5, mid[1] + inward[1] * 0.5];
    let mut out = String::new();
    if top < z1 {
        out += &general_prism_brush(&[a, b, ib, ia], top, z1, hint);
    }
    if sill > z0 {
        out += &general_prism_brush(&[a, b, ib, ia], z0, sill, hint);
    }
    out
}

/// A doorway wall on `face`: two jamb prisms beside a `DOOR_HALF_WIDTH`
/// opening, a lintel band above `top`, an optional sill band below `sill`,
/// and — when `trim_height > 0` — a register trim band along each jamb at
/// sill level (the door opening itself stays clean).
pub(crate) fn door_wall(
    face: HexFace,
    z0: f64,
    z1: f64,
    sill: f64,
    top: f64,
    trim_height: f64,
) -> String {
    let (a, b) = tb_edge(face);
    let (ia, ib) = offset_inward(a, b, WALL);
    let inward = [ia[0] - a[0], ia[1] - a[1]];
    let d = [b[0] - a[0], b[1] - a[1]];
    let length = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let u = [d[0] / length, d[1] / length];
    let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let da = [
        mid[0] - u[0] * DOOR_HALF_WIDTH,
        mid[1] - u[1] * DOOR_HALF_WIDTH,
    ];
    let db = [
        mid[0] + u[0] * DOOR_HALF_WIDTH,
        mid[1] + u[1] * DOOR_HALF_WIDTH,
    ];
    let (ida, idb) = offset_inward(da, db, WALL);
    let mut out = String::new();
    let left_hint = [
        (a[0] + da[0]) * 0.5 + inward[0] * 0.5,
        (a[1] + da[1]) * 0.5 + inward[1] * 0.5,
    ];
    out += &general_prism_brush(&[a, da, ida, ia], z0, top, left_hint);
    let right_hint = [
        (db[0] + b[0]) * 0.5 + inward[0] * 0.5,
        (db[1] + b[1]) * 0.5 + inward[1] * 0.5,
    ];
    out += &general_prism_brush(&[db, b, ib, idb], z0, top, right_hint);
    let wall_hint = [mid[0] + inward[0] * 0.5, mid[1] + inward[1] * 0.5];
    if top < z1 {
        out += &general_prism_brush(&[a, b, ib, ia], top, z1, wall_hint);
    }
    if sill > z0 {
        out += &general_prism_brush(&[a, b, ib, ia], z0, sill, wall_hint);
    }
    if trim_height > 0.0 {
        for (s0, s1) in [(a, da), (db, b)] {
            let (o0, o1) = offset_inward(s0, s1, WALL);
            let (i0, i1) = offset_inward(s0, s1, WALL + 8.0);
            let hint = [
                (o0[0] + o1[0]) * 0.5 + inward[0],
                (o0[1] + o1[1]) * 0.5 + inward[1],
            ];
            out += &general_prism_brush(&[o0, o1, i1, i0], sill, sill + trim_height, hint);
        }
    }
    out
}

/// The two-level ramp floor: a full-footprint wedge rising from the entrance
/// edge (at `floor_top`) to the exit face midpoint (at `floor_top + h`).
pub(crate) fn sloped_slab_brush(
    entrance_face: HexFace,
    exit_face: HexFace,
    floor_top: f64,
    h: f64,
) -> String {
    let mut out = String::from("{\n");
    for face in HexFace::LATERAL {
        let (a, b) = tb_edge(face);
        out += &side_plane(a, b, 0.0, [0.0, 0.0]);
    }
    out += &flat_plane(0.0, false);
    let (a2, b2) = tb_edge(entrance_face);
    let (ea2, eb2) = tb_edge(exit_face);
    let mid_exit = [(ea2[0] + eb2[0]) * 0.5, (ea2[1] + eb2[1]) * 0.5];
    let p1 = [a2[0], a2[1], floor_top];
    let p2 = [b2[0], b2[1], floor_top];
    let p3 = [mid_exit[0], mid_exit[1], floor_top + h];
    out += &plane_line(p1, p2, p3);
    out += "}\n";
    out
}

/// Point entity text (`tile_meta`, `tile_port`).
pub(crate) fn point_entity(props: &[(&str, &str)]) -> String {
    let mut out = String::from("{\n");
    for (key, value) in props {
        out += &format!("\"{key}\" \"{value}\"\n");
    }
    out += "}\n";
    out
}

pub(crate) fn tile_meta(archetype: &str, register: &str, variant: u16, levels: u8) -> String {
    point_entity(&[
        ("classname", "tile_meta"),
        ("archetype", archetype),
        ("register", register),
        ("variant", &variant.to_string()),
        ("levels", &levels.to_string()),
    ])
}

pub(crate) fn tile_port(face: &str, class: &str) -> String {
    point_entity(&[("classname", "tile_port"), ("face", face), ("class", class)])
}

/// One node of the climbable line through a compatibility tile.
///
/// The strict authored path has had this since Arc O; the generated kit did
/// not, which is why a generated ramp projected no traversal annotation at all
/// and had to be walked by inferring a heading from its archetype. `index`
/// orders the climb bottom to top and the importer rejects a repeat.
pub(crate) fn tile_stair_node(index: u16, x: f64, y: f64, z: f64) -> String {
    point_entity(&[
        ("classname", "tile_stair_node"),
        ("index", &index.to_string()),
        ("origin", &format!("{} {} {}", fmt(x), fmt(y), fmt(z))),
    ])
}

/// A presentation-owned practical light at a tile-local TrenchBroom point.
/// Geometry generators choose placement; the shared style layer chooses its
/// colour, energy, and legibility treatment at runtime.
pub(crate) fn tile_light(x: f64, y: f64, z: f64) -> String {
    point_entity(&[
        ("classname", "tile_light"),
        ("kind", "practical"),
        ("origin", &format!("{} {} {}", fmt(x), fmt(y), fmt(z))),
    ])
}

pub(crate) fn worldspawn(brushes: &str) -> String {
    format!("{{\n\"classname\" \"worldspawn\"\n{brushes}}}\n")
}
