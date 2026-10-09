//! Distant enclosure proxy matching the low Backrooms kit.
use super::{
    CEILING_BAND, CellSkin, OUTSET, SkinData, corner, keel, keel_hash, span_skin, unbuilt,
};
use bevy::prelude::*;
use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::{HexCoord, HexFace, HexWfcWorld};
use observed_hex::{FLOOR_SLAB_TOP, TILE_LEVEL_HEIGHT, hex_origin};
use observed_match::hex_wfc::open_edges;

/// The skin of one built cell, or `None` for anything unbuilt.
#[must_use]
pub(in crate::hex_wfc::view) fn cell_skin(world: &HexWfcWorld, at: HexCoord) -> Option<CellSkin> {
    if !world.placements.get(&at)?.space.built() {
        return None;
    }
    let o = Vec3::from_array(hex_origin(at));
    let low = world.architecture.get(&at) == Some(&ArchitectureRegister::LiminalGrid)
        && !matches!(
            world.placements[&at].archetype,
            observed_facility::hex_wfc::HexArchetype::Climb { .. }
                | observed_facility::hex_wfc::HexArchetype::Cistern { .. }
        );
    let height = if low { 3.625 } else { TILE_LEVEL_HEIGHT };
    let open = open_edges(world, at);
    let mut skin = CellSkin::default();
    if let Some(axis) = open.and_then(|open| open.span) {
        span_skin(&mut skin, o, axis);
        return Some(skin);
    }
    for face in HexFace::LATERAL
        .into_iter()
        .filter(|&face| unbuilt(world, at, face))
    {
        let (a, b) = (
            o + corner(face.index()) * OUTSET,
            o + corner(face.index() + 1) * OUTSET,
        );
        let outward = ((a + b) * 0.5 - o).with_y(0.0);
        let band = |skin: &mut SkinData, lo: f32, hi: f32| {
            skin.polygon(
                &[
                    a + Vec3::Y * lo,
                    b + Vec3::Y * lo,
                    b + Vec3::Y * hi,
                    a + Vec3::Y * hi,
                ],
                outward,
            );
        };
        if open.is_some_and(|open| open.opens(face)) {
            band(&mut skin.walls, 0.0, FLOOR_SLAB_TOP);
            band(&mut skin.walls, height - CEILING_BAND, height);
            band(&mut skin.lips, FLOOR_SLAB_TOP, FLOOR_SLAB_TOP + 0.08);
        } else {
            band(&mut skin.walls, 0.0, height);
            // Somebody is home: up to three lit slits, a hand's width proud of the face.
            let slits = keel_hash(at) >> (face.index() * 2) & 3;
            for slot in 0..slits {
                #[allow(clippy::cast_precision_loss)]
                let t = (slot as f32 + 1.0) / (slits as f32 + 1.0);
                let run = b - a;
                let centre = a + run * t + outward.normalize_or_zero() * 0.05;
                let half = run.normalize_or_zero() * 0.28;
                skin.windows.polygon(
                    &[
                        centre - half + Vec3::Y * 2.6,
                        centre + half + Vec3::Y * 2.6,
                        centre + half + Vec3::Y * 5.0_f32.min(height - 0.3),
                        centre - half + Vec3::Y * 5.0_f32.min(height - 0.3),
                    ],
                    outward,
                );
            }
        }
    }
    let ring = |y: f32, inset: f32| -> Vec<Vec3> {
        (0..6)
            .map(|i| o + corner(i) * inset + Vec3::Y * y)
            .collect()
    };
    if unbuilt(world, at, HexFace::Up) {
        skin.caps.polygon(&ring(height, OUTSET), Vec3::Y);
    }
    if unbuilt(world, at, HexFace::Down) {
        skin.flat_underside = keel(&mut skin.keel, world, at, o);
    }
    Some(skin)
}
