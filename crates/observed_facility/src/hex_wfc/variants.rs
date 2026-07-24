//! The production variant alphabet: grounded lateral rooms/halls, paired
//! `RampOpen` ramps, and logical `ShaftOpen` links whose physical projection
//! is a ground-supported switchback stair tower rather than an elevator.

use std::collections::BTreeSet;

use observed_hex::{HexFace, PortClass, PortSignature};

use crate::map_spec::RoomRole;

use super::blueprint::{blueprint_cell_archetype, blueprint_for_role};
use super::{HexArchetype, HexPlacement, HexSpace, lateral_bit};

/// One exact authored-tile requirement emitted by hex geometry projection.
///
/// Architecture register is deliberately not part of this value: the
/// authoring coverage gate requires every one of these semantic pairs in every
/// production register.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct HexGeometryDemand {
    pub archetype: &'static str,
    pub signature: PortSignature,
}

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

    // 1. Room variants: any lateral door mask, optionally a vertical opening
    //    for the fixed two-level Guardian Control atrium.
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
            // Ramp halves are sparse in the much larger flat-hall alphabet;
            // this weight makes three-level chains occur in the seeded corpus.
            weight: 20,
        });
        variants.push(HexVariant {
            space: HexSpace::Hall,
            archetype: HexArchetype::RampHead,
            doors: lateral_bit(d),
            up: PortClass::Sealed,
            down: PortClass::RampOpen,
            weight: 20,
        });
    }

    // 4. Legacy logical well states remain part of the solver's vertical
    // alphabet, but presentation resolves them to grounded stair towers.
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

/// Manifest archetype for a non-room placement, or `None` when the cell emits
/// no prefab. Rooms are projected once per stamped blueprint through
/// [`blueprint_cell_archetype`]; `RampHead` is the empty upper half of the
/// two-level prefab emitted by its `RampUp` below.
#[must_use]
pub fn placement_tile_archetype(placement: &HexPlacement) -> Option<&'static str> {
    match placement.archetype {
        HexArchetype::Void | HexArchetype::Room | HexArchetype::RampHead => None,
        HexArchetype::Straight => Some("hall_straight"),
        HexArchetype::Corner => Some(corner_tile_archetype(placement.doors)),
        HexArchetype::Junction if placement.doors.count_ones() == 3 => Some("hall_junction_3way"),
        HexArchetype::Junction => Some("hall_junction_4way"),
        HexArchetype::RampUp => Some("hall_ramp"),
        HexArchetype::Shaft => Some("stair_tower"),
    }
}

fn corner_tile_archetype(doors: u8) -> &'static str {
    let faces = HexFace::LATERAL
        .into_iter()
        .filter(|&face| doors & lateral_bit(face) != 0)
        .map(HexFace::index)
        .collect::<Vec<_>>();
    debug_assert_eq!(faces.len(), 2);
    let distance = faces[0].abs_diff(faces[1]);
    if distance.min(6 - distance) == 1 {
        "hall_turn_60"
    } else {
        "hall_turn_120"
    }
}

/// Every exact `(archetype, signature)` pair geometry projection can request.
///
/// This is intentionally narrower than [`demandable_signatures`], which is the
/// WFC propagation alphabet and includes generic Room variants that can never
/// leave a blueprint domain plus the geometry-free `RampHead`. The manifest
/// coverage gate and projector both consume this function so the contract
/// cannot drift back to a hand-written subset.
#[must_use]
pub fn geometry_demands() -> Vec<HexGeometryDemand> {
    let mut demands = BTreeSet::new();
    for variant in catalogue() {
        let placement = HexPlacement {
            coord: observed_hex::HexCoord::default(),
            space: variant.space,
            archetype: variant.archetype,
            doors: variant.doors,
            up: variant.up,
            down: variant.down,
        };
        if let Some(archetype) = placement_tile_archetype(&placement) {
            demands.insert(HexGeometryDemand {
                archetype,
                signature: variant.signature(),
            });
        }
    }

    const ROOM_ROLES: [RoomRole; 11] = [
        RoomRole::Start,
        RoomRole::Exit,
        RoomRole::Decision,
        RoomRole::DecoherenceFork,
        RoomRole::AnchorCheckpoint,
        RoomRole::TeleportRelay,
        RoomRole::Keystone,
        RoomRole::DualStation,
        RoomRole::GuardianControl,
        RoomRole::Monitor,
        RoomRole::Recovery,
    ];
    for role in ROOM_ROLES {
        let blueprint = blueprint_for_role(role);
        for (cell_index, &offset) in blueprint.cells.iter().enumerate() {
            let archetype = blueprint_cell_archetype(role, cell_index)
                .expect("every blueprint cell has an authored tile archetype");
            demands.insert(HexGeometryDemand {
                archetype,
                signature: blueprint.cell_signature(offset),
            });
        }
    }
    demands.into_iter().collect()
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

#[cfg(test)]
mod geometry_tests {
    use super::*;

    #[test]
    fn geometry_demands_are_exact_and_exclude_non_emitters() {
        let demands = geometry_demands();
        let unique: BTreeSet<_> = demands.iter().copied().collect();
        assert_eq!(unique.len(), demands.len());
        assert!(
            !demands
                .iter()
                .any(|demand| demand.archetype.contains("shaft"))
        );
        assert_eq!(
            demands
                .iter()
                .filter(|demand| demand.archetype == "hall_ramp")
                .count(),
            6
        );
        assert_eq!(
            demands
                .iter()
                .filter(|demand| demand.archetype == "stair_tower")
                .count(),
            64
        );
        assert!(!demands.iter().any(|demand| demand.archetype == "void"));
        assert!(!demands.iter().any(|demand| demand.archetype == "ramp_head"));
        assert!(!demands.iter().any(|demand| demand.archetype == "room"));
    }

    #[test]
    fn ramp_head_is_geometry_free_but_ramp_up_selects_the_prefab() {
        let placement = |archetype| HexPlacement {
            coord: observed_hex::HexCoord::default(),
            space: HexSpace::Hall,
            archetype,
            doors: lateral_bit(HexFace::East),
            up: PortClass::Sealed,
            down: PortClass::RampOpen,
        };
        assert_eq!(
            placement_tile_archetype(&placement(HexArchetype::RampHead)),
            None
        );
        assert_eq!(
            placement_tile_archetype(&placement(HexArchetype::RampUp)),
            Some("hall_ramp")
        );
    }
}
