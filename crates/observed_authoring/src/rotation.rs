//! The six turns of a hex module, in one place.
//!
//! A module is authored once and may be placed at any of six 60-degree turns.
//! [`catalog`](crate::catalog) bakes those turns into the compiled catalog:
//! hulls, lights, footprint cells, ports, spine and deck are each transformed
//! and stored, one prototype per turn, keyed `variant = module.variant * 6 +
//! turn`.
//!
//! These are the primitives that transform does, lifted out and made public so
//! that a tool asking *"what would this module look like turned?"* — the module
//! studio's neighbour ring, for one — uses the transform the catalog uses
//! rather than a second copy of it. A copy would have to be found and changed
//! every time this moves, and its failure mode is a preview that shows an
//! author a tile the game will never build.
//!
//! # Primitives, deliberately, and not a `rotated_prototype`
//!
//! There is no function here that hands back a whole turned [`TilePrototype`],
//! because there is no honest one to write. A prototype's `contract` is stored
//! **module-local and unrotated** (`TilePrototype::contract`), and its `key`
//! encodes the turn in `variant`. A helper that rotated the geometry and left
//! those two alone would return a value whose fields are in different frames —
//! the exact trap that makes a half-transformed struct worse than no helper.
//! Callers take the pieces they need and stay aware of which frame they are in.
//!
//! # Direction
//!
//! `turn` counts clockwise 60-degree steps, `0..6`. It is the same sense
//! `CompiledModule::rotations` records and the same sense `RotationPolicy`
//! authorises, so a turn number means one thing everywhere.

use glam::{Quat, Vec3};
use observed_hex::{HexFace, PortClass, PortSignature};

use crate::source::ModuleCellRef;

/// Turns per full revolution. Six, because the prism has six lateral faces.
pub const TURNS: u8 = 6;

/// The rotation applied to points and poses for `turn`.
///
/// Negative because the lattice's face numbering runs counterclockwise while a
/// turn is clockwise; getting this sign wrong turns every module the wrong way
/// while still looking plausible in a screenshot.
#[must_use]
pub fn turn_quat(turn: u8) -> Quat {
    Quat::from_rotation_y(-f32::from(turn % TURNS) * std::f32::consts::TAU / 6.0)
}

/// Which face `face` becomes after `turn`. Vertical faces do not move.
#[must_use]
pub fn rotate_face(face: HexFace, turn: u8) -> HexFace {
    if face.is_vertical() {
        return face;
    }
    HexFace::LATERAL[(face.index() + usize::from(turn % TURNS)) % 6]
}

/// Where an axial footprint cell lands after `turn`.
#[must_use]
pub fn rotate_cell(cell: ModuleCellRef, turn: u8) -> ModuleCellRef {
    let mut q = cell.q;
    let mut r = cell.r;
    for _ in 0..(turn % TURNS) {
        (q, r) = (-r, q.saturating_add(r));
    }
    ModuleCellRef {
        q,
        r,
        level: cell.level,
    }
}

/// Turn a set of tile-local convex hulls.
#[must_use]
pub fn rotate_hulls(hulls: &[Vec<Vec3>], turn: u8) -> Vec<Vec<Vec3>> {
    let rotation = turn_quat(turn);
    hulls
        .iter()
        .map(|hull| hull.iter().map(|point| rotation * *point).collect())
        .collect()
}

/// Turn a port signature: every lateral class moves to the face it lands on.
///
/// This is the cheap half of a turn, and the only half a legality question
/// needs — "could this module sit there, turned?" is answered entirely by where
/// its ports end up. Rotating hulls to answer it would be thousands of times
/// the work for the same yes or no.
///
/// Total by construction: a rotation is a permutation of the lateral faces and
/// leaves the vertical ones alone, so no class ever lands on a face where it is
/// invalid and the repack cannot fail.
#[must_use]
pub fn rotate_signature(signature: PortSignature, turn: u8) -> PortSignature {
    let mut ports = [PortClass::Sealed; 8];
    for face in HexFace::ALL {
        ports[rotate_face(face, turn).index()] = signature.port(face);
    }
    PortSignature::try_from_ports(ports)
        .expect("a turn permutes lateral faces and fixes vertical ones, so classes stay valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_turns_return_every_face_home() {
        for face in HexFace::ALL {
            assert_eq!(rotate_face(face, 6), face);
            assert_eq!(rotate_face(face, 0), face);
        }
    }

    /// The lateral faces must permute, not collapse. A rotation that mapped two
    /// faces onto one would silently drop a port.
    #[test]
    fn a_turn_permutes_the_lateral_faces() {
        for turn in 0..6 {
            let mut landed: Vec<usize> = HexFace::LATERAL
                .into_iter()
                .map(|face| rotate_face(face, turn).index())
                .collect();
            landed.sort_unstable();
            assert_eq!(
                landed,
                vec![0, 1, 2, 3, 4, 5],
                "turn {turn} is not a permutation"
            );
        }
    }

    #[test]
    fn vertical_faces_never_turn() {
        for turn in 0..6 {
            assert_eq!(rotate_face(HexFace::Up, turn), HexFace::Up);
            assert_eq!(rotate_face(HexFace::Down, turn), HexFace::Down);
        }
    }

    /// The signature has to travel with the faces, or a turned module would be
    /// offered on a face it does not actually open onto.
    #[test]
    fn a_turned_signature_follows_its_faces() {
        let mut ports = [PortClass::Sealed; 8];
        ports[HexFace::East.index()] = PortClass::Door;
        ports[HexFace::Up.index()] = PortClass::ShaftOpen;
        let signature = PortSignature::try_from_ports(ports).expect("valid fixture");

        let turned = rotate_signature(signature, 1);
        assert_eq!(turned.port(HexFace::SouthEast), PortClass::Door);
        assert_eq!(turned.port(HexFace::East), PortClass::Sealed);
        assert_eq!(
            turned.port(HexFace::Up),
            PortClass::ShaftOpen,
            "a vertical class does not move"
        );
        assert_eq!(rotate_signature(signature, 6), signature);
    }

    /// Rotating hulls and rotating the axial cell send a thing the same way —
    /// but **not** to the same number, and the gap is not a bug.
    ///
    /// [`rotate_hulls`] applies a true 60-degree rotation. The plan lattice does
    /// not: the canonical hexagon is *quantized* to 14 m across flats and 16 m
    /// across corners so every corner lands on an integer TrenchBroom
    /// coordinate, and a regular hexagon 14 m across flats would be 16.17 m
    /// across corners. So a cell step of 12 m stands in for the exact
    /// `14 * sin(60) = 12.124`, and turned geometry lands 0.8 % off the lattice
    /// it is placed on.
    ///
    /// This is what the compiled catalog already ships — it rotates hulls by
    /// quaternion and places them on the quantized lattice — so a preview that
    /// did anything else would disagree with the game. The tolerance below is
    /// that 0.8 %, written down, rather than a fudge factor.
    #[test]
    fn a_turn_agrees_with_the_lattice_to_within_its_own_quantization() {
        let east_plan = vec![vec![Vec3::new(14.0, 0.0, 0.0)]];
        let turned = rotate_hulls(&east_plan, 1);
        let cell = rotate_cell(
            ModuleCellRef {
                q: 1,
                r: 0,
                level: 0,
            },
            1,
        );
        // One turn takes the East cell to SouthEast, whose plan origin is
        // (7, 12) by `hex_origin_plan`.
        assert_eq!((cell.q, cell.r), (0, 1));
        let point = turned[0][0];
        assert!((point.x - 7.0).abs() < 0.001, "x is exact: {point:?}");
        assert!(
            (point.z - 12.0).abs() < 0.13,
            "z is the quantized stand-in for 14*sin(60): {point:?}"
        );
        assert!(
            (point.z - 12.0).abs() > 0.1,
            "if this ever became exact the hexagon stopped being quantized, \
             and every authored corner moved off its integer editor coordinate"
        );
    }

    #[test]
    fn turns_past_six_wrap_rather_than_running_off_the_table() {
        assert_eq!(rotate_face(HexFace::East, 7), rotate_face(HexFace::East, 1));
        assert_eq!(
            rotate_cell(
                ModuleCellRef {
                    q: 1,
                    r: 0,
                    level: 0
                },
                7
            ),
            rotate_cell(
                ModuleCellRef {
                    q: 1,
                    r: 0,
                    level: 0
                },
                1
            )
        );
    }
}
