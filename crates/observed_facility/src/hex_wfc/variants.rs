//! The Phase 88 variant alphabet: every cell state the collapse can choose.
//!
//! Built programmatically over the six lateral door bits — deliberately NOT a
//! `u8`-mask enumeration baked into types, so Phase 90 can extend the same
//! construction with vertical port classes and the manifest-driven catalog.

use observed_hex::HexFace;

use super::{HexArchetype, HexSpace, lateral_bit};

#[derive(Clone, Copy, Debug)]
pub(super) struct HexVariant {
    pub space: HexSpace,
    pub archetype: HexArchetype,
    pub doors: u8,
    pub weight: u32,
}

/// All 6-bit lateral door masks: void, rooms of every degree, degree-2 halls,
/// and weight-0 junction halls (authored only by the deterministic promotion
/// pass, kept in the domain so a promoted junction stays representable).
pub(super) fn catalogue() -> Vec<HexVariant> {
    let mut variants = vec![HexVariant {
        space: HexSpace::Void,
        archetype: HexArchetype::Void,
        doors: 0,
        weight: 4,
    }];
    for mask in 1u8..64 {
        let degree = mask.count_ones();
        let room_weight = match degree {
            1 => 4,
            2 => 3,
            3 => 2,
            _ => 1,
        };
        variants.push(HexVariant {
            space: HexSpace::Room,
            archetype: HexArchetype::Room,
            doors: mask,
            weight: room_weight,
        });
        if degree == 2 {
            variants.push(HexVariant {
                space: HexSpace::Hall,
                archetype: hall_archetype(mask),
                doors: mask,
                weight: 6,
            });
        } else if (3..=4).contains(&degree) {
            variants.push(HexVariant {
                space: HexSpace::Hall,
                archetype: HexArchetype::Junction,
                doors: mask,
                weight: 0,
            });
        }
    }
    variants
}

/// A degree-2 hall is `Straight` when its doors face opposite directions,
/// `Corner` otherwise; 3+ doors are junctions.
pub(super) fn hall_archetype(mask: u8) -> HexArchetype {
    if mask.count_ones() >= 3 {
        return HexArchetype::Junction;
    }
    let straight = HexFace::LATERAL
        .into_iter()
        .any(|face| mask == lateral_bit(face) | lateral_bit(face.opposite()));
    if straight {
        HexArchetype::Straight
    } else {
        HexArchetype::Corner
    }
}

/// Face compatibility between two adjacent variants across `face` (from `a`'s
/// side): door meets door, sealed meets sealed, and two rooms never connect
/// directly.
pub(super) fn variants_compatible(a: HexVariant, b: HexVariant, face: HexFace) -> bool {
    let a_open = a.doors & lateral_bit(face) != 0;
    let b_open = b.doors & lateral_bit(face.opposite()) != 0;
    if a_open != b_open {
        return false;
    }
    if a_open && a.space == HexSpace::Room && b.space == HexSpace::Room {
        return false;
    }
    if a_open && (a.space == HexSpace::Void || b.space == HexSpace::Void) {
        return false;
    }
    true
}
