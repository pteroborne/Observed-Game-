//! Cell-local line work: the footprint, its walls, and the two glyphs that say
//! a floor change is available here.
//!
//! Everything is produced in a cell's own local frame, so a caller instances it
//! by translating to `hex_origin(cell)` rather than re-deriving per cell.

use bevy::prelude::*;
use observed_hex::CORNERS;

/// The cell-local corners at `inset`, in [`CORNERS`] order.
fn ring(y: f32, inset: f32) -> Vec<Vec3> {
    CORNERS
        .iter()
        .map(|&(x, z)| {
            #[allow(clippy::cast_precision_loss)]
            Vec3::new(x as f32 * inset, y, z as f32 * inset)
        })
        .collect()
}

/// The floor outline of a cell at full extent, so neighbouring cells meet and
/// the lattice reads as one connected surface rather than as scattered tiles.
#[must_use]
pub fn floor_ring(inset: f32) -> Vec<(Vec3, Vec3)> {
    let corners = ring(0.0, inset);
    (0..6)
        .map(|index| (corners[index], corners[(index + 1) % 6]))
        .collect()
}

/// Solid wall bands, one per face the cell does **not** open through.
///
/// A face that opens keeps a jamb at each end and leaves the middle clear, so a
/// doorway is a real hole in a real wall. Walls sit slightly inside the
/// footprint: neighbouring cells each own their own wall, which is both true of
/// the authored tiles and what keeps two coincident quads from fighting.
#[must_use]
pub fn wall_bands(height: f32, open: [bool; 6]) -> Vec<[Vec3; 4]> {
    /// How much of an opening's edge stays walled at each end.
    const JAMB: f32 = 0.3;
    /// Pulled off the shared edge so adjacent cells do not co-plane.
    const WALL_INSET: f32 = 0.965;

    let low = ring(0.0, WALL_INSET);
    let high = ring(height, WALL_INSET);
    let mut out = Vec::with_capacity(8);
    let band = |from: usize, to: usize, start: f32, end: f32, out: &mut Vec<[Vec3; 4]>| {
        let a = low[from].lerp(low[to], start);
        let b = low[from].lerp(low[to], end);
        let c = high[from].lerp(high[to], end);
        let d = high[from].lerp(high[to], start);
        out.push([a, b, c, d]);
    };
    for (index, opens) in open.iter().enumerate() {
        let next = (index + 1) % 6;
        if *opens {
            band(index, next, 0.0, JAMB, &mut out);
            band(index, next, 1.0 - JAMB, 1.0, &mut out);
        } else {
            band(index, next, 0.0, 1.0, &mut out);
        }
    }
    out
}

/// A staircase, drawn in profile at the centre of a cell that connects upward.
///
/// The schematic convention: you should be able to tell a floor change is
/// available here without selecting anything or counting hull edges.
#[must_use]
pub fn stair_glyph(height: f32) -> Vec<(Vec3, Vec3)> {
    const STEPS: usize = 4;
    const RUN: f32 = 1.5;
    #[allow(clippy::cast_precision_loss)]
    let span = RUN * STEPS as f32;
    #[allow(clippy::cast_precision_loss)]
    let rise = height / STEPS as f32;
    let mut out = Vec::with_capacity(STEPS * 2);
    let mut cursor = Vec3::new(-span * 0.5, 0.0, 0.0);
    for _ in 0..STEPS {
        let tread = cursor + Vec3::X * RUN;
        out.push((cursor, tread));
        let riser = tread + Vec3::Y * rise;
        out.push((tread, riser));
        cursor = riser;
    }
    out
}

/// Compact plan-view arrows for a shaft that connects to another floor.
///
/// Unlike [`stair_glyph`], this stays flat and occupies only the centre of the
/// cell. An up connection points toward the top of the map, a down connection
/// toward the bottom; a through-shaft draws both side by side.
#[must_use]
pub fn level_arrow_glyph(up: bool, down: bool) -> Vec<(Vec3, Vec3)> {
    const HALF_SPAN: f32 = 1.6;
    const HEAD_DEPTH: f32 = 0.75;
    const HEAD_WIDTH: f32 = 0.7;
    const Y: f32 = 0.18;

    let mut out = Vec::with_capacity(6);
    let mut arrow = |x: f32, direction: f32| {
        let tail = Vec3::new(x, Y, -direction * HALF_SPAN);
        let tip = Vec3::new(x, Y, direction * HALF_SPAN);
        let shoulder_z = direction * (HALF_SPAN - HEAD_DEPTH);
        out.push((tail, tip));
        out.push((tip, Vec3::new(x - HEAD_WIDTH, Y, shoulder_z)));
        out.push((tip, Vec3::new(x + HEAD_WIDTH, Y, shoulder_z)));
    };
    match (up, down) {
        (true, true) => {
            arrow(-1.0, 1.0);
            arrow(1.0, -1.0);
        }
        (true, false) => arrow(0.0, 1.0),
        (false, true) => arrow(0.0, -1.0),
        (false, false) => {}
    }
    out
}

/// A slope with a chevron at its high end, for a cell that ramps upward.
#[must_use]
pub fn ramp_glyph(height: f32) -> Vec<(Vec3, Vec3)> {
    let low = Vec3::new(-3.0, 0.0, 0.0);
    let high = Vec3::new(3.0, height, 0.0);
    vec![
        (low, high),
        (high, high + Vec3::new(-1.4, -0.5, 0.9)),
        (high, high + Vec3::new(-1.4, -0.5, -0.9)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_floor_ring_closes_at_full_extent_so_neighbours_meet() {
        let ring = floor_ring(1.0);
        assert_eq!(ring.len(), 6);
        // Every edge must end where the next begins, or the lattice reads as
        // scattered tiles instead of one connected surface.
        for index in 0..6 {
            assert_eq!(ring[index].1, ring[(index + 1) % 6].0);
        }
        let reach = ring
            .iter()
            .flat_map(|(a, b)| [a.x.abs(), b.x.abs()])
            .fold(0.0, f32::max);
        assert!(
            (reach - 7.0).abs() < 1e-5,
            "full extent reaches the flat faces"
        );
    }

    #[test]
    fn a_sealed_face_walls_and_an_open_one_leaves_a_doorway() {
        let sealed = wall_bands(1.0, [false; 6]);
        assert_eq!(sealed.len(), 6, "six sealed faces, six solid bands");

        let mut open = [false; 6];
        open[0] = true;
        let doored = wall_bands(1.0, open);
        assert_eq!(
            doored.len(),
            7,
            "an opening splits its face into two jambs, so the count rises by one"
        );
        // The doorway is a real hole: the two jambs must not meet.
        let jamb_a = doored[0];
        let jamb_b = doored[1];
        assert!(
            (jamb_a[1] - jamb_b[0]).length() > 1.0,
            "the jambs leave a gap wide enough to read as a door"
        );
    }

    #[test]
    fn walls_sit_inside_the_footprint_so_neighbours_do_not_co_plane() {
        let reach = wall_bands(1.0, [false; 6])
            .iter()
            .flat_map(|quad| quad.iter().map(|corner| corner.x.abs()))
            .fold(0.0, f32::max);
        let floor = floor_ring(1.0)
            .iter()
            .flat_map(|(a, b)| [a.x.abs(), b.x.abs()])
            .fold(0.0, f32::max);
        assert!(
            reach < floor,
            "a cell owns its own wall, just inside its edge"
        );
    }

    #[test]
    fn a_wall_band_is_only_as_tall_as_it_is_asked_to_be() {
        let top = wall_bands(2.5, [false; 6])
            .iter()
            .flat_map(|quad| quad.iter().map(|corner| corner.y))
            .fold(f32::MIN, f32::max);
        assert!((top - 2.5).abs() < 1e-5);
    }

    #[test]
    fn level_arrows_are_small_flat_and_directional() {
        let up = level_arrow_glyph(true, false);
        let down = level_arrow_glyph(false, true);
        assert_eq!(up.len(), 3);
        assert_eq!(down.len(), 3);
        assert!(
            up.iter()
                .flat_map(|(from, to)| [from, to])
                .all(|point| point.y == 0.18)
        );
        assert!(
            down.iter()
                .flat_map(|(from, to)| [from, to])
                .all(|point| point.y == 0.18)
        );
        assert!(up[0].1.z > up[0].0.z);
        assert!(down[0].1.z < down[0].0.z);
        assert_eq!(level_arrow_glyph(true, true).len(), 6);
        assert!(level_arrow_glyph(false, false).is_empty());
    }
}
