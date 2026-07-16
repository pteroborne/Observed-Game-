//! The eight faces of a hex prism: six lateral neighbors plus Up/Down.

/// One face of a hex-prism cell. Lateral faces are numbered counterclockwise
/// so that `opposite` is `(face + 3) % 6`; `Up`/`Down` complete the prism.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum HexFace {
    East = 0,
    SouthEast = 1,
    SouthWest = 2,
    West = 3,
    NorthWest = 4,
    NorthEast = 5,
    Up = 6,
    Down = 7,
}

impl HexFace {
    /// Every face, in index order. Iteration order is part of the determinism
    /// contract: solvers and validators walk faces in this order.
    pub const ALL: [HexFace; 8] = [
        HexFace::East,
        HexFace::SouthEast,
        HexFace::SouthWest,
        HexFace::West,
        HexFace::NorthWest,
        HexFace::NorthEast,
        HexFace::Up,
        HexFace::Down,
    ];

    /// The six lateral faces, in index order.
    pub const LATERAL: [HexFace; 6] = [
        HexFace::East,
        HexFace::SouthEast,
        HexFace::SouthWest,
        HexFace::West,
        HexFace::NorthWest,
        HexFace::NorthEast,
    ];

    /// Stable face index (0..8), also the port-signature lane.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn is_lateral(self) -> bool {
        (self as u8) < 6
    }

    #[must_use]
    pub const fn is_vertical(self) -> bool {
        (self as u8) >= 6
    }

    /// The face met on the other side of this one: lateral faces pair across
    /// the hex (`(f + 3) % 6`), `Up` pairs with `Down`.
    #[must_use]
    pub const fn opposite(self) -> HexFace {
        match self {
            HexFace::East => HexFace::West,
            HexFace::SouthEast => HexFace::NorthWest,
            HexFace::SouthWest => HexFace::NorthEast,
            HexFace::West => HexFace::East,
            HexFace::NorthWest => HexFace::SouthEast,
            HexFace::NorthEast => HexFace::SouthWest,
            HexFace::Up => HexFace::Down,
            HexFace::Down => HexFace::Up,
        }
    }

    /// Axial step to the neighbor behind this face: `(dq, dr, dlevel)`.
    #[must_use]
    pub const fn delta(self) -> (i32, i32, i32) {
        match self {
            HexFace::East => (1, 0, 0),
            HexFace::SouthEast => (0, 1, 0),
            HexFace::SouthWest => (-1, 1, 0),
            HexFace::West => (-1, 0, 0),
            HexFace::NorthWest => (0, -1, 0),
            HexFace::NorthEast => (1, -1, 0),
            HexFace::Up => (0, 0, 1),
            HexFace::Down => (0, 0, -1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HexFace;

    #[test]
    fn opposite_is_an_involution_and_lateral_pairs_are_three_apart() {
        for face in HexFace::ALL {
            assert_eq!(face.opposite().opposite(), face);
            if face.is_lateral() {
                assert_eq!(
                    face.opposite().index(),
                    (face.index() + 3) % 6,
                    "lateral opposite must be (f + 3) % 6 for {face:?}"
                );
            }
        }
    }

    #[test]
    fn delta_of_opposite_faces_cancels_exactly() {
        for face in HexFace::ALL {
            let (dq, dr, dl) = face.delta();
            let (oq, or, ol) = face.opposite().delta();
            assert_eq!((dq + oq, dr + or, dl + ol), (0, 0, 0), "{face:?}");
        }
    }

    #[test]
    fn lateral_and_vertical_partition_the_face_set() {
        assert_eq!(
            HexFace::ALL.iter().filter(|f| f.is_lateral()).count(),
            HexFace::LATERAL.len()
        );
        for face in HexFace::ALL {
            assert_ne!(face.is_lateral(), face.is_vertical(), "{face:?}");
        }
    }
}
