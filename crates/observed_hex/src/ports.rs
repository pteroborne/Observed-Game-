//! Port classes: the typed replacement for the square lattice's boolean
//! opening mask. Every face of a tile carries one class, and WFC face
//! compatibility is equal-class matching on the shared face.

use crate::faces::HexFace;

/// What a tile face offers across its boundary.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum PortClass {
    /// Solid boundary; meets only another sealed face.
    #[default]
    Sealed = 0,
    /// A walkable doorway. Lateral faces only.
    Door = 1,
    /// The vertical bond inside a climb: a `RampUp` tile's `Up` face meets its
    /// `RampHead`'s `Down` face, and a climb composition's high cell meets the landing
    /// above it. Vertical faces only.
    RampOpen = 2,
    /// A climbable shaft opening (wellshaft/silo stacks). Vertical faces only.
    ShaftOpen = 3,
    /// The lateral bond inside a climb composition: the flight running on from one of
    /// its cells into the next, at whatever height it has reached there. Lateral
    /// faces only.
    ///
    /// Packed into the lane `RampOpen` uses on vertical faces: no lateral face could
    /// ever carry that, so every signature from before this class existed reads back
    /// exactly as it was.
    Span = 4,
}

impl PortClass {
    pub const ALL: [PortClass; 5] = [
        PortClass::Sealed,
        PortClass::Door,
        PortClass::RampOpen,
        PortClass::ShaftOpen,
        PortClass::Span,
    ];

    /// Whether this class may sit on `face` at all.
    #[must_use]
    pub const fn valid_on(self, face: HexFace) -> bool {
        match self {
            PortClass::Sealed => true,
            PortClass::Door | PortClass::Span => face.is_lateral(),
            PortClass::RampOpen | PortClass::ShaftOpen => face.is_vertical(),
        }
    }

    /// The two bits this class packs to on a face it is valid on.
    const fn lane(self) -> u16 {
        match self {
            PortClass::Sealed => 0,
            PortClass::Door => 1,
            PortClass::RampOpen | PortClass::Span => 2,
            PortClass::ShaftOpen => 3,
        }
    }

    /// The class two packed bits mean on `face`. Lane 2 is a span on a lateral face
    /// and a ramp bond on a vertical one; lane 3 has no lateral meaning and reads as a
    /// shaft opening, which [`PortSignature::is_valid`] then refuses there.
    const fn from_bits(bits: u16, face: HexFace) -> PortClass {
        match bits & 0b11 {
            0 => PortClass::Sealed,
            1 => PortClass::Door,
            2 if face.is_lateral() => PortClass::Span,
            2 => PortClass::RampOpen,
            _ => PortClass::ShaftOpen,
        }
    }
}

/// WFC face compatibility: two touching faces bond only when they offer the
/// same class. (`Sealed` meets `Sealed`, `Door` meets `Door`, and the two
/// vertical classes meet themselves.) The relation is symmetric by
/// construction.
#[must_use]
pub const fn ports_compatible(a: PortClass, b: PortClass) -> bool {
    a as u8 == b as u8
}

/// All eight face ports of a tile packed two bits per face — the hash/dedup
/// key for tile catalogs and manifests. Build one from a readable
/// `[PortClass; 8]` (indexed by [`HexFace::index`]); it refuses classes on
/// faces where they are invalid.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortSignature(pub u16);

/// A port class placed on a face that cannot carry it (for example `Door` on
/// `Up`, or `ShaftOpen` on a lateral face).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidPort {
    pub face: HexFace,
    pub class: PortClass,
}

impl PortSignature {
    /// Pack a full port assignment, validating every face/class pairing.
    pub fn try_from_ports(ports: [PortClass; 8]) -> Result<PortSignature, InvalidPort> {
        let mut packed = 0u16;
        for face in HexFace::ALL {
            let class = ports[face.index()];
            if !class.valid_on(face) {
                return Err(InvalidPort { face, class });
            }
            packed |= class.lane() << (face.index() * 2);
        }
        Ok(PortSignature(packed))
    }

    /// The class on one face.
    #[must_use]
    pub const fn port(self, face: HexFace) -> PortClass {
        PortClass::from_bits(self.0 >> (face.index() * 2), face)
    }

    /// Unpack into the readable form.
    #[must_use]
    pub fn ports(self) -> [PortClass; 8] {
        HexFace::ALL.map(|face| self.port(face))
    }

    /// Whether every lane of the packed value is a legal face/class pairing.
    #[must_use]
    pub fn is_valid(self) -> bool {
        HexFace::ALL
            .into_iter()
            .all(|face| self.port(face).valid_on(face))
    }
}

#[cfg(test)]
mod tests {
    use super::{PortClass, PortSignature, ports_compatible};
    use crate::faces::HexFace;

    #[test]
    fn compatibility_is_equal_class_matching_and_symmetric() {
        for a in PortClass::ALL {
            for b in PortClass::ALL {
                assert_eq!(ports_compatible(a, b), a == b);
                assert_eq!(ports_compatible(a, b), ports_compatible(b, a));
            }
        }
    }

    #[test]
    fn signatures_round_trip_every_valid_port_assignment_per_face() {
        // Exhaustive per-face sweep: for each face, each class that is valid
        // there survives a pack/unpack round trip on an otherwise-sealed tile.
        for face in HexFace::ALL {
            for class in PortClass::ALL {
                let mut ports = [PortClass::Sealed; 8];
                ports[face.index()] = class;
                let packed = PortSignature::try_from_ports(ports);
                if class.valid_on(face) {
                    let signature = packed.expect("valid assignment must pack");
                    assert!(signature.is_valid());
                    assert_eq!(signature.port(face), class);
                    assert_eq!(signature.ports(), ports);
                } else {
                    let err = packed.expect_err("invalid assignment must be refused");
                    assert_eq!((err.face, err.class), (face, class));
                }
            }
        }
    }

    #[test]
    fn door_is_lateral_only_and_vertical_classes_are_vertical_only() {
        for face in HexFace::ALL {
            assert!(PortClass::Sealed.valid_on(face));
            assert_eq!(PortClass::Door.valid_on(face), face.is_lateral());
            assert_eq!(PortClass::RampOpen.valid_on(face), face.is_vertical());
            assert_eq!(PortClass::ShaftOpen.valid_on(face), face.is_vertical());
            assert_eq!(PortClass::Span.valid_on(face), face.is_lateral());
        }
    }

    #[test]
    fn a_span_shares_a_lane_without_changing_any_older_signature() {
        // A climb's mid cell: spans fore and aft, nothing else.
        let mut ports = [PortClass::Sealed; 8];
        ports[HexFace::West.index()] = PortClass::Span;
        ports[HexFace::East.index()] = PortClass::Span;
        let signature = PortSignature::try_from_ports(ports).expect("valid mid cell");
        assert_eq!(signature.ports(), ports);
        // The same bits on a vertical face are the ramp bond they always were.
        let mut ramp = [PortClass::Sealed; 8];
        ramp[HexFace::Up.index()] = PortClass::RampOpen;
        let packed = PortSignature::try_from_ports(ramp).expect("valid ramp");
        assert_eq!(packed.0, 2 << (HexFace::Up.index() * 2));
        assert_eq!(packed.port(HexFace::Up), PortClass::RampOpen);
    }

    #[test]
    fn a_representative_ramp_low_tile_signature_packs_and_reads_back() {
        // RampUp(East): low Door entrance on West, RampOpen above.
        let mut ports = [PortClass::Sealed; 8];
        ports[HexFace::West.index()] = PortClass::Door;
        ports[HexFace::Up.index()] = PortClass::RampOpen;
        let signature = PortSignature::try_from_ports(ports).expect("valid ramp-low tile");
        assert_eq!(signature.port(HexFace::West), PortClass::Door);
        assert_eq!(signature.port(HexFace::Up), PortClass::RampOpen);
        assert_eq!(signature.port(HexFace::Down), PortClass::Sealed);
        assert_eq!(signature.port(HexFace::East), PortClass::Sealed);
    }
}
