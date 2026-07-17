//! The Phase 90 variant alphabet: every cell state the collapse can choose,
//! now spanning verticality — `ShaftOpen` wells and paired `RampOpen` ramps.

use std::collections::BTreeSet;

use observed_hex::{HexFace, PortClass, PortSignature};

use super::{HexArchetype, HexSpace, lateral_bit};

#[derive(Clone, Copy, Debug)]
pub(super) struct HexVariant {
    pub space: HexSpace,
    pub archetype: HexArchetype,
    pub doors: u8,
    pub up: PortClass,
    pub down: PortClass,
    pub weight: u32,
}

impl HexVariant {
    /// The port signature this variant presents to its neighbors.
    pub(super) fn signature(self) -> PortSignature {
        let mut ports = [PortClass::Sealed; 8];
        for face in HexFace::LATERAL {
            if self.doors & lateral_bit(face) != 0 {
                ports[face.index()] = PortClass::Door;
            }
        }
        ports[HexFace::Up.index()] = self.up;
        ports[HexFace::Down.index()] = self.down;
        PortSignature::try_from_ports(ports).expect("variant ports are valid by construction")
    }
}

pub(super) fn catalogue() -> Vec<HexVariant> {
    let mut variants = vec![HexVariant {
        space: HexSpace::Void,
        archetype: HexArchetype::Void,
        doors: 0,
        up: PortClass::Sealed,
        down: PortClass::Sealed,
        weight: 4,
    }];

    // 1. Room variants: any lateral door mask, optionally a vertical shaft
    //    face (rooms carry ShaftOpen for atria/silos; ramps are hall-only).
    for &up in &[PortClass::Sealed, PortClass::ShaftOpen] {
        for &down in &[PortClass::Sealed, PortClass::ShaftOpen] {
            for mask in 0u8..64 {
                if mask == 0 && up == PortClass::Sealed && down == PortClass::Sealed {
                    continue;
                }
                let degree = mask.count_ones();
                let room_weight = match degree {
                    0 | 1 => 4,
                    2 => 3,
                    3 => 2,
                    _ => 1,
                };
                variants.push(HexVariant {
                    space: HexSpace::Room,
                    archetype: HexArchetype::Room,
                    doors: mask,
                    up,
                    down,
                    weight: room_weight,
                });
            }
        }
    }

    // 2. Flat hall and junction variants (lateral only).
    for mask in 1u8..64 {
        let degree = mask.count_ones();
        if degree == 2 {
            variants.push(HexVariant {
                space: HexSpace::Hall,
                archetype: hall_archetype(mask),
                doors: mask,
                up: PortClass::Sealed,
                down: PortClass::Sealed,
                weight: 6,
            });
        } else if (3..=4).contains(&degree) {
            variants.push(HexVariant {
                space: HexSpace::Hall,
                archetype: HexArchetype::Junction,
                doors: mask,
                up: PortClass::Sealed,
                down: PortClass::Sealed,
                weight: 2,
            });
        }
    }

    // 3. Ramp pairs for all six lateral directions. `RampUp(d)` enters
    //    laterally at `opposite(d)` and offers `RampOpen` above; `RampHead(d)`
    //    receives `RampOpen` from below and exits laterally at `d`. Face
    //    compatibility self-assembles the pair — no diagonal constraints.
    for &d in &HexFace::LATERAL {
        variants.push(HexVariant {
            space: HexSpace::Hall,
            archetype: HexArchetype::RampUp,
            doors: lateral_bit(d.opposite()),
            up: PortClass::RampOpen,
            down: PortClass::Sealed,
            weight: 5,
        });
        variants.push(HexVariant {
            space: HexSpace::Hall,
            archetype: HexArchetype::RampHead,
            doors: lateral_bit(d),
            up: PortClass::Sealed,
            down: PortClass::RampOpen,
            weight: 5,
        });
    }

    // 4. Shaft (wellshaft) variants: a through-column segment plus capped and
    //    landing segments. The high through weight lets columns stack into
    //    multi-level wells.
    variants.push(HexVariant {
        space: HexSpace::Hall,
        archetype: HexArchetype::Shaft,
        doors: 0,
        up: PortClass::ShaftOpen,
        down: PortClass::ShaftOpen,
        weight: 6,
    });
    for &up in &[PortClass::Sealed, PortClass::ShaftOpen] {
        for &down in &[PortClass::Sealed, PortClass::ShaftOpen] {
            if up == PortClass::Sealed && down == PortClass::Sealed {
                continue;
            }
            for mask in 1u8..64 {
                let degree = mask.count_ones();
                if degree == 1 || degree == 2 {
                    variants.push(HexVariant {
                        space: HexSpace::Hall,
                        archetype: HexArchetype::Shaft,
                        doors: mask,
                        up,
                        down,
                        weight: 4,
                    });
                }
            }
        }
    }

    variants
}

/// Every distinct port signature the solver can demand of a tile — the feed
/// for Phase 91's manifest coverage validator. Deterministically ordered
/// (`PortSignature` is `Ord`).
#[must_use]
pub fn demandable_signatures() -> Vec<PortSignature> {
    catalogue()
        .into_iter()
        .filter(|variant| variant.space != HexSpace::Void)
        .map(HexVariant::signature)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

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

pub(super) fn variants_compatible(a: HexVariant, b: HexVariant, face: HexFace) -> bool {
    if face.is_lateral() {
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
    } else {
        let a_port = if face == HexFace::Up { a.up } else { a.down };
        let b_port = if face == HexFace::Up { b.down } else { b.up };
        if a_port as u8 != b_port as u8 {
            return false;
        }
        // Ramp halves only ever meet their partner across the shared RampOpen
        // face, in the correct vertical orientation and lateral direction.
        if a_port == PortClass::RampOpen {
            let (lower, upper) = if face == HexFace::Up { (a, b) } else { (b, a) };
            if lower.archetype != HexArchetype::RampUp || upper.archetype != HexArchetype::RampHead
            {
                return false;
            }
            if ramp_direction(lower) != ramp_direction(upper) {
                return false;
            }
        }
        true
    }
}

fn ramp_direction(variant: HexVariant) -> Option<HexFace> {
    match variant.archetype {
        HexArchetype::RampUp => {
            let face = HexFace::LATERAL
                .into_iter()
                .find(|&f| variant.doors & lateral_bit(f) != 0)?;
            Some(face.opposite())
        }
        HexArchetype::RampHead => HexFace::LATERAL
            .into_iter()
            .find(|&f| variant.doors & lateral_bit(f) != 0),
        _ => None,
    }
}
