//! Windows: where a room's wall faces the outside, a window is cut in it.
//!
//! Rooms keep their enclosure where they meet open air (only corridor halls open, see
//! `open_edge`), so without windows the moon lights nothing indoors. A room face whose
//! neighbour is outside the room and unbuilt, or off the lattice, gets a tall window:
//! the walls standing against that face are cut into what stays below the sill, what
//! stays above the lintel, a jamb at each end, and mullions between the panes.
//!
//! **Glazed, not opened.** A window changes what is drawn, never what collides. The
//! wall it was cut from keeps its collider ID and its exact hull, as a
//! [`HexPiecePart::Glazing`] that is never drawn; the cut pieces are
//! [`HexPiecePart::Window`]s that are drawn and never collided. So nothing can walk,
//! fall or be pushed through a window, and a body touching the wall touches the same
//! collider it always did. Light passes, because only drawn pieces cast shadows.
//!
//! **Built versus unbuilt**, for the reason `open_edge` gives: a relayout only builds
//! or clears cells inside its region, and the observation halo keeps a watched room's
//! lateral neighbours out of it, so a window never opens or closes where someone is
//! looking.
//!
//! **How a wall is found.** Authored hulls carry no face, so a hull belongs to the face
//! whose sector holds its plan centroid (`open_edge::sector_of`). It is that face's
//! outside wall if it reaches the hex edge and runs along it; a pier, or the end of a
//! partition meeting the wall, is too short to cut. Every piece is the hull clipped by
//! planes, so each is convex and lies inside the wall it came from.
use glam::{Vec2, Vec3};
use observed_facility::hex_wfc::{HexCoord, HexFace, HexWfcWorld};
use observed_hex::{FLOOR_SLAB_TOP, face_edge};

use super::open_edge::{face_frame, sector_of};

/// Sill height above the walking surface, metres.
pub const SILL: f32 = 1.0;
/// Lintel height above the walking surface, metres. Levels are eight metres; a
/// window this tall reads as a room built to look out of.
pub const LINTEL: f32 = 4.4;
/// How far a window stops short of each end of its face, metres.
const JAMB: f32 = 1.1;
/// A mullion's width, metres.
const MULLION: f32 = 0.16;
/// The widest a pane may be, metres.
const PANE: f32 = 1.7;
/// A hull whose outer side stands within this of the hex edge is the outside wall.
const AT_THE_EDGE: f32 = 0.45;
/// A hull shorter than this along its face is a pier or a partition's end.
const WALL_LENGTH: f32 = 1.2;
/// Clipping tolerance, and the thinnest a kept piece may be on any axis, metres.
const EPSILON: f32 = 1e-4;
const THINNEST: f32 = 0.02;

/// Whether the room occupying `room` looks out through `face` of its cell `at`: the
/// cell beyond is not the room's own, and is unbuilt or off the lattice.
#[must_use]
pub fn looks_out(world: &HexWfcWorld, room: &[HexCoord], at: HexCoord, face: HexFace) -> bool {
    face.is_lateral()
        && world.config.grid().neighbor(at, face).is_none_or(|next| {
            !room.contains(&next)
                && world
                    .placements
                    .get(&next)
                    .is_none_or(|placement| placement.space.unbuilt())
        })
}

/// The plan frame of a face: outward unit normal, unit tangent along the edge, the
/// distance from the cell centre to the edge, and the edge's half length.
fn frame(face: HexFace) -> (Vec3, Vec3, f32, f32) {
    let (normal, apothem) = face_frame(face);
    let [a, b] = face_edge(face);
    #[allow(clippy::cast_precision_loss)]
    let run = Vec2::new((b.0 - a.0) as f32, (b.1 - a.1) as f32);
    (
        Vec3::new(normal.x, 0.0, normal.y),
        Vec3::new(run.x, 0.0, run.y).normalize(),
        apothem,
        run.length() * 0.5,
    )
}

/// The part of a convex solid, given by its vertices, on the near side of a plane
/// `axis · p <= at`, as the vertices of the clipped solid.
///
/// A vertex already on the near side stays a vertex. The new ones are the corners of
/// the section the plane cuts, and every one of those is where some segment between a
/// kept and a dropped vertex crosses the plane; the other crossings lie inside the
/// section. So the section is the convex hull of all the crossings, taken in the
/// plane. Keeping only its corners keeps a chain of clips from squaring the point
/// count at every step.
fn clip(points: &[Vec3], axis: Vec3, at: f32) -> Vec<Vec3> {
    let side: Vec<f32> = points.iter().map(|&p| axis.dot(p) - at).collect();
    let mut kept: Vec<Vec3> = points
        .iter()
        .zip(&side)
        .filter(|&(_, &s)| s <= EPSILON)
        .map(|(&p, _)| p)
        .collect();
    let u = if axis.y.abs() > 0.5 {
        Vec3::X
    } else {
        axis.cross(Vec3::Y).normalize()
    };
    let v = axis.cross(u);
    let mut section = Vec::new();
    for (i, &inside) in side.iter().enumerate() {
        if inside >= -EPSILON {
            continue;
        }
        for (j, &outside) in side.iter().enumerate() {
            if outside > EPSILON {
                let p = points[i] + (points[j] - points[i]) * (inside / (inside - outside));
                section.push(Vec2::new(u.dot(p), v.dot(p)));
            }
        }
    }
    kept.extend(
        super::convex_hull(section)
            .into_iter()
            .map(|q| u * q.x + v * q.y + axis * at),
    );
    kept.sort_by(|a, b| {
        a.x.total_cmp(&b.x)
            .then(a.y.total_cmp(&b.y))
            .then(a.z.total_cmp(&b.z))
    });
    kept.dedup_by(|a, b| a.distance_squared(*b) < EPSILON * EPSILON);
    kept
}

/// Extent of a point set along an axis.
fn span(points: &[Vec3], axis: Vec3) -> (f32, f32) {
    points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &p| {
        let d = axis.dot(p);
        (lo.min(d), hi.max(d))
    })
}

/// A clipped piece worth keeping: a solid, not a sliver left where a plane grazed it.
fn solid(points: &[Vec3], normal: Vec3, along: Vec3) -> bool {
    points.len() >= 4
        && [normal, along, Vec3::Y].into_iter().all(|axis| {
            let (lo, hi) = span(points, axis);
            hi - lo > THINNEST
        })
}

/// The drawn pieces of `hull` with a window cut through it, if `hull` is the outside
/// wall of `face` and the window crosses it. Cell-local in and out.
#[must_use]
pub fn cut(hull: &[Vec3], face: HexFace) -> Option<Vec<Vec<Vec3>>> {
    if sector_of(hull) != Some(face) {
        return None;
    }
    let (normal, along, apothem, half) = frame(face);
    let (_, reach) = span(hull, normal);
    let (from, to) = span(hull, along);
    if reach < apothem - AT_THE_EDGE || to - from < WALL_LENGTH {
        return None;
    }
    let (sill, lintel) = (FLOOR_SLAB_TOP + SILL, FLOOR_SLAB_TOP + LINTEL);
    let width = half - JAMB;
    let band = clip(&clip(hull, -Vec3::Y, -sill), Vec3::Y, lintel);
    let opening = clip(&clip(&band, along, width), -along, width);
    if !solid(&opening, normal, along) {
        return None;
    }
    let mut pieces = vec![
        clip(hull, Vec3::Y, sill),
        clip(hull, -Vec3::Y, -lintel),
        clip(&band, along, -width),
        clip(&band, -along, -width),
    ];
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let panes = ((2.0 * width) / PANE).ceil().max(1.0) as u32;
    for mullion in 1..panes {
        #[allow(clippy::cast_precision_loss)]
        let t = -width + 2.0 * width * mullion as f32 / panes as f32;
        pieces.push(clip(
            &clip(&band, along, t + MULLION * 0.5),
            -along,
            -(t - MULLION * 0.5),
        ));
    }
    pieces.retain(|piece| solid(piece, normal, along));
    Some(pieces)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use glam::{Vec2, Vec3};
    use observed_facility::hex_wfc::{HexCoord, HexFace};
    use observed_hex::{FLOOR_SLAB_TOP, hex_origin};

    use super::{LINTEL, SILL, cut, frame, span};

    /// A slab of wall standing along `face`, `depth` thick at the hex edge, floor to
    /// ceiling, as a room authors it.
    fn wall(face: HexFace, depth: f32) -> Vec<Vec3> {
        let (normal, along, apothem, half) = frame(face);
        let mut points = Vec::new();
        for t in [-half * 0.9, half * 0.9] {
            for d in [apothem - depth, apothem] {
                for y in [0.0, 7.5] {
                    points.push(normal * d + along * t + Vec3::Y * y);
                }
            }
        }
        points
    }

    fn inside(hull: &[Vec3], point: Vec3) -> bool {
        let (lo, hi) = hull.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        });
        (lo..=hi).contains(&point.y)
            && observed_traversal::point_in_convex_plan_hull(hull, Vec2::new(point.x, point.z))
    }

    #[test]
    fn a_wall_is_cut_below_the_sill_above_the_lintel_and_between_the_panes() {
        for face in HexFace::LATERAL {
            let original = wall(face, 0.4);
            let pieces = cut(&original, face).expect("an outside wall takes a window");
            let (normal, along, apothem, _) = frame(face);
            let at = |t: f32, y: f32| normal * (apothem - 0.2) + along * t + Vec3::Y * y;
            let drawn = |p: Vec3| pieces.iter().any(|piece| inside(piece, p));
            // Wall below the sill, above the lintel, and at each end.
            assert!(drawn(at(0.3, FLOOR_SLAB_TOP + SILL * 0.5)), "{face:?} sill");
            assert!(
                drawn(at(0.3, FLOOR_SLAB_TOP + LINTEL + 1.0)),
                "{face:?} lintel"
            );
            assert!(drawn(at(3.5, FLOOR_SLAB_TOP + 2.5)), "{face:?} jamb");
            // And through the middle of the window, at eye height, nothing.
            let clear = (-20..=20)
                .map(|step| step as f32 * 0.1)
                .filter(|&t| !drawn(at(t, FLOOR_SLAB_TOP + 1.7)))
                .count();
            assert!(clear > 25, "{face:?}: only {clear} of 41 samples see out");
            // Every piece lies within the wall it was cut from.
            for axis in [normal, along, Vec3::Y] {
                let (lo, hi) = span(&original, axis);
                for &p in pieces.iter().flatten() {
                    let d = axis.dot(p);
                    assert!(
                        d > lo - 1e-3 && d < hi + 1e-3,
                        "{face:?}: {p} outside the wall"
                    );
                }
            }
        }
    }

    /// The corpus assumption, pinned: every committed room face that looks out, with
    /// wall standing across its middle, is cut clear there at eye height. A room
    /// authored some other way must fail here, not quietly keep a blank wall.
    #[test]
    fn every_room_face_that_looks_out_is_cut_clear_across_the_corpus() {
        let rooms = &crate::hex_wfc::test_catalog().rooms;
        assert!(rooms.len() > 10, "{} rooms", rooms.len());
        let grid = observed_hex::HexGridSize {
            cols: 40,
            rows: 40,
            levels: 12,
        };
        let anchor = HexCoord {
            q: 20,
            r: 20,
            level: 5,
        };
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let place = |cell: &observed_authoring::ModuleCellRef| HexCoord {
            q: (i32::from(anchor.q) + i32::from(cell.q)) as u16,
            r: (i32::from(anchor.r) + i32::from(cell.r)) as u16,
            level: (i32::from(anchor.level) + i32::from(cell.level)) as u8,
        };
        let (mut walled, mut windows) = (0, 0);
        let mut blank = Vec::new();
        for room in rooms {
            let cells: Vec<HexCoord> = room.footprint.iter().map(place).collect();
            let ports: BTreeSet<(HexCoord, HexFace)> = room
                .ports
                .iter()
                .map(|port| (place(&port.cell), port.face))
                .collect();
            let looks_out = |cell: HexCoord, face: HexFace| {
                face.is_lateral()
                    && !ports.contains(&(cell, face))
                    && grid
                        .neighbor(cell, face)
                        .is_none_or(|next| !cells.contains(&next))
            };
            let cut = super::super::windows_for(&room.hulls, anchor, &cells, looks_out);
            windows += cut.len();
            let drawn: Vec<&Vec<Vec3>> = room
                .hulls
                .iter()
                .enumerate()
                .filter(|(index, _)| !cut.contains_key(index))
                .map(|(_, hull)| hull)
                .chain(cut.values().flatten())
                .collect();
            let origin = Vec3::from_array(hex_origin(anchor));
            for &cell in &cells {
                for face in HexFace::LATERAL {
                    if !looks_out(cell, face) {
                        continue;
                    }
                    let (normal, along, apothem, _) = frame(face);
                    let eye = Vec3::from_array(hex_origin(cell)) - origin
                        + normal * (apothem - 0.15)
                        + Vec3::Y * (FLOOR_SLAB_TOP + 2.5);
                    if !room.hulls.iter().any(|hull| inside(hull, eye)) {
                        continue;
                    }
                    walled += 1;
                    // The middle two panes' centres, clear of every mullion.
                    let panes = [-0.72, 0.72].map(|t| eye + along * t);
                    if panes
                        .iter()
                        .any(|&p| drawn.iter().any(|hull| inside(hull, p)))
                    {
                        blank.push(format!("{} {cell:?} {face:?}", room.id));
                    }
                }
            }
        }
        eprintln!(
            "{walled} walled faces look out, {} blank; {windows} hulls cut",
            blank.len()
        );
        assert!(walled > 50, "only {walled} walled faces look out");
        assert!(
            blank.is_empty(),
            "{} of {walled} stay blank: {blank:#?}",
            blank.len()
        );
    }

    #[test]
    fn a_pier_a_partition_end_and_a_wall_of_another_face_are_left_alone() {
        let face = HexFace::LATERAL[0];
        let (normal, along, apothem, _) = frame(face);
        let pier: Vec<Vec3> = [-0.3, 0.3]
            .into_iter()
            .flat_map(|t| {
                [apothem - 0.4, apothem].into_iter().flat_map(move |d| {
                    [0.0, 7.5]
                        .into_iter()
                        .map(move |y| normal * d + along * t + Vec3::Y * y)
                })
            })
            .collect();
        assert!(cut(&pier, face).is_none(), "a pier is too short to cut");
        let set_back: Vec<Vec3> = wall(face, 0.4)
            .into_iter()
            .map(|p| p - normal * 2.0)
            .collect();
        assert!(
            cut(&set_back, face).is_none(),
            "an inner wall is not the outside"
        );
        assert!(cut(&wall(face, 0.4), HexFace::LATERAL[1]).is_none());
    }
}
