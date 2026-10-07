//! Enclosure semantics at projection, shared by collision and presentation.
use super::HexStructureRole;
use glam::Vec3;
use observed_authoring::HullSurface;

pub(super) fn projected_surface(
    explicit: Option<HullSurface>,
    hull: &[Vec3],
    levels: u8,
    role: HexStructureRole,
) -> Option<HullSurface> {
    if explicit.is_some() || role == HexStructureRole::Boundary {
        return explicit;
    }
    let min = hull
        .iter()
        .copied()
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let max = hull
        .iter()
        .copied()
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    let extent = max - min;
    let roof = f32::from(levels) * observed_hex::TILE_LEVEL_HEIGHT;
    // The module's declared enclosure boundary, rather than an arbitrary thin
    // horizontal hull. Interior galleries and platforms do not terminate here.
    if (max.y - roof).abs() < 0.1
        && min.y >= roof - 1.0
        && extent.y <= 1.0
        && extent.x.max(extent.z) >= 6.0
        && extent.x.min(extent.z) >= 2.0
    {
        Some(HullSurface::Ceiling)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declared_roofs_are_explicit_but_galleries_and_climb_platforms_are_not() {
        let slab = |y: f32| vec![Vec3::new(-7.0, y, -7.0), Vec3::new(7.0, y + 0.5, 7.0)];
        assert_eq!(
            projected_surface(None, &slab(15.5), 2, HexStructureRole::Room),
            Some(HullSurface::Ceiling)
        );
        assert_eq!(
            projected_surface(None, &slab(7.5), 2, HexStructureRole::Room),
            None
        );
        assert_eq!(
            projected_surface(None, &slab(8.0), 1, HexStructureRole::Climb),
            None
        );
        assert_eq!(
            projected_surface(
                Some(HullSurface::Floor),
                &slab(7.5),
                1,
                HexStructureRole::Room
            ),
            Some(HullSurface::Floor)
        );
    }
}
