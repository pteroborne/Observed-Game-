//! The authored vista: towers, floating decks and the walkways between them, written
//! as cells on the real hex lattice and loaded into a real [`HexWfcWorld`].
//!
//! Nothing here is solved. The layout is hand-placed because the question is what
//! open air *looks like* from inside it, and a solver would answer a different
//! question (what open air the corpus happens to produce). What the lab does borrow
//! from the facility is everything downstream of placement: the typed ports, the
//! routing oracle, and — the point — [`HexWfcWorld::mark_open_air`], which is the
//! only thing that decides which unbuilt cells are sky. The renderer never guesses.
use std::collections::{BTreeMap, BTreeSet};

use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::profile::SpaceMix;
use observed_facility::hex_wfc::{
    HexArchetype, HexCoord, HexFace, HexPlacement, HexSpace, HexWfcConfig, HexWfcWorld, PortClass,
};

/// The lattice. Twenty-two cells across is ~300 m: far enough that the open-air fog
/// has fully become horizon before the edge.
pub const CONFIG: HexWfcConfig = HexWfcConfig {
    cols: 22,
    rows: 18,
    levels: 8,
    min_rooms: 0,
    max_rooms: 0,
    retry_budget: 1,
    min_room_distance: 0,
};

/// From this level up, decks and walkways carry no railing — the design's "higher
/// floors draw increasingly unsafe architecture", told in the one way a first-person
/// eye cannot miss.
pub const UNSAFE_FROM_LEVEL: u8 = 5;

/// A named structure in the vista, for the HUD and the tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Landmark {
    /// Where you start: a five-storey stack with a terrace on top.
    Bastion,
    /// A floating two-storey deck with a pavilion, hanging over nothing.
    LanternIsle,
    /// A one-cell tower, forty metres of sheer face.
    Needle,
    /// A floating deck south of the isle, facing an island it cannot reach.
    Gallery,
    /// The island across the gap from the Gallery: seen, never crossed.
    FarShore,
    /// Two low decks and the walkway between them, twenty-four metres down.
    Understory,
    /// The summit tower and its beacon.
    Summit,
    /// Massifs and floaters at the edge of sight, for scale.
    Distance,
}

impl Landmark {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Bastion => "the Bastion",
            Self::LanternIsle => "Lantern Isle",
            Self::Needle => "the Needle",
            Self::Gallery => "the Gallery",
            Self::FarShore => "the far shore",
            Self::Understory => "the understory",
            Self::Summit => "the Summit",
            Self::Distance => "the distance",
        }
    }
}

/// An authored camera pose: where the eye stands and what it looks at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vantage {
    pub slug: &'static str,
    pub title: &'static str,
    /// Eye position, metres.
    pub eye: [f32; 3],
    /// A point the eye looks toward, metres.
    pub look_at: [f32; 3],
    /// Whether the pose is one a body can stand at. Outside views fly.
    pub standing: bool,
}

/// The authored vista, loaded into a real facility world.
pub struct Vista {
    pub world: HexWfcWorld,
    pub landmarks: BTreeMap<HexCoord, Landmark>,
    /// The summit deck the beacon stands on.
    pub summit: HexCoord,
    /// Where a body spawns: the Bastion terrace.
    pub spawn: HexCoord,
    /// The walk across every kind of walkway, cell by cell.
    pub tour: Vec<HexCoord>,
    /// How many unbuilt cells `mark_open_air` turned into air.
    pub air_cells: usize,
}

/// Axial shorthand: `c(q, r, level)`.
#[must_use]
pub const fn c(q: u16, r: u16, level: u8) -> HexCoord {
    HexCoord { q, r, level }
}

struct Author {
    placements: BTreeMap<HexCoord, HexPlacement>,
    architecture: BTreeMap<HexCoord, ArchitectureRegister>,
    landmarks: BTreeMap<HexCoord, Landmark>,
}

impl Author {
    fn new() -> Self {
        let mut placements = BTreeMap::new();
        let grid = CONFIG.grid();
        for index in 0..grid.cell_count() {
            let coord = grid.coord(index);
            placements.insert(
                coord,
                HexPlacement {
                    coord,
                    space: HexSpace::Void,
                    archetype: HexArchetype::Void,
                    doors: 0,
                    up: PortClass::Sealed,
                    down: PortClass::Sealed,
                },
            );
        }
        Self {
            placements,
            architecture: BTreeMap::new(),
            landmarks: BTreeMap::new(),
        }
    }

    fn put(
        &mut self,
        at: HexCoord,
        space: HexSpace,
        archetype: HexArchetype,
        register: ArchitectureRegister,
        landmark: Landmark,
    ) {
        let cell = self
            .placements
            .get_mut(&at)
            .unwrap_or_else(|| panic!("{at:?} lies outside the vista lattice"));
        assert!(
            cell.space == HexSpace::Void,
            "{at:?} authored twice ({:?} then {space:?})",
            cell.space
        );
        cell.space = space;
        cell.archetype = archetype;
        self.architecture.insert(at, register);
        self.landmarks.insert(at, landmark);
    }

    /// Open a lateral door on both sides of a face. Both cells must already exist.
    fn join(&mut self, at: HexCoord, face: HexFace) {
        let next = CONFIG
            .grid()
            .neighbor(at, face)
            .unwrap_or_else(|| panic!("{at:?} has no neighbour through {face:?}"));
        for (cell, side) in [(at, face), (next, face.opposite())] {
            let placement = self.placements.get_mut(&cell).expect("lattice cell");
            assert!(
                placement.space.built(),
                "door from {at:?} through {face:?} opens onto unbuilt {cell:?}"
            );
            placement.doors |= 1 << side.index();
        }
    }

    /// A stack: sealed storeys from `base` up to (not including) `top`, and an open
    /// deck at `top` whose cells all open onto one another.
    fn tower(
        &mut self,
        footprint: &[(u16, u16)],
        base: u8,
        top: u8,
        register: ArchitectureRegister,
        landmark: Landmark,
    ) {
        for level in base..top {
            for &(q, r) in footprint {
                self.put(
                    c(q, r, level),
                    HexSpace::Room,
                    HexArchetype::Room,
                    register,
                    landmark,
                );
            }
        }
        self.deck(footprint, top, register, landmark);
    }

    /// An open deck: hall cells joined to every deck neighbour on the same level.
    fn deck(
        &mut self,
        footprint: &[(u16, u16)],
        level: u8,
        register: ArchitectureRegister,
        landmark: Landmark,
    ) {
        let cells: BTreeSet<HexCoord> = footprint.iter().map(|&(q, r)| c(q, r, level)).collect();
        for &at in &cells {
            self.put(
                at,
                HexSpace::Hall,
                HexArchetype::Expanse,
                register,
                landmark,
            );
        }
        for &at in &cells {
            for face in HexFace::LATERAL {
                if CONFIG
                    .grid()
                    .neighbor(at, face)
                    .is_some_and(|next| cells.contains(&next))
                {
                    self.join(at, face);
                }
            }
        }
    }

    /// A straight walkway of `length` hall cells leaving `from` through `face`, joined
    /// at both ends. The far end must already be built.
    fn span(&mut self, from: HexCoord, face: HexFace, length: u8, register: ArchitectureRegister) {
        let landmark = self.landmarks[&from];
        let mut at = from;
        for _ in 0..length {
            let next = CONFIG
                .grid()
                .neighbor(at, face)
                .expect("span stays on lattice");
            self.put(
                next,
                HexSpace::Hall,
                HexArchetype::Straight,
                register,
                landmark,
            );
            self.join(at, face);
            at = next;
        }
        self.join(at, face);
    }

    /// A stair flight: entered from `from` through `face`, it climbs one level inside
    /// the next cell and leaves its head through the same heading. Returns the head.
    fn flight(
        &mut self,
        from: HexCoord,
        face: HexFace,
        register: ArchitectureRegister,
    ) -> HexCoord {
        let landmark = self.landmarks[&from];
        let foot = CONFIG
            .grid()
            .neighbor(from, face)
            .expect("flight on lattice");
        let head = CONFIG
            .grid()
            .neighbor(foot, HexFace::Up)
            .expect("head on lattice");
        self.put(
            foot,
            HexSpace::Hall,
            HexArchetype::RampUp,
            register,
            landmark,
        );
        self.put(
            head,
            HexSpace::Hall,
            HexArchetype::RampHead,
            register,
            landmark,
        );
        self.join(from, face);
        self.placements.get_mut(&foot).expect("foot").up = PortClass::RampOpen;
        self.placements.get_mut(&head).expect("head").down = PortClass::RampOpen;
        head
    }

    /// Turn a deck cell into an enclosed pavilion that keeps its doors.
    fn pavilion(&mut self, at: HexCoord) {
        let cell = self.placements.get_mut(&at).expect("pavilion cell");
        assert!(
            cell.space == HexSpace::Hall,
            "a pavilion is raised on a deck"
        );
        cell.space = HexSpace::Room;
        cell.archetype = HexArchetype::Room;
    }
}

impl Vista {
    /// The one authored vista. Deterministic: there is nothing random in it.
    #[must_use]
    pub fn authored() -> Self {
        use ArchitectureRegister as Reg;
        use HexFace as F;
        use Landmark as L;
        let mut a = Author::new();

        // The Bastion: four sealed storeys, a terrace on the fifth. You start here.
        a.tower(&[(5, 8), (6, 8), (5, 9)], 0, 4, Reg::Monolith, L::Bastion);

        // Lantern Isle: one storey and a deck, floating. A pavilion on its east cell.
        for &(q, r) in &[(9, 8), (9, 9)] {
            a.put(
                c(q, r, 3),
                HexSpace::Room,
                HexArchetype::Room,
                Reg::Megastructure,
                L::LanternIsle,
            );
        }
        a.deck(
            &[(9, 8), (10, 8), (9, 9)],
            4,
            Reg::Megastructure,
            L::LanternIsle,
        );
        a.pavilion(c(10, 8, 4));

        // Railed walkway, Bastion to Isle, level 4.
        a.span(c(6, 8, 4), F::East, 2, Reg::Megastructure);

        // The Needle: a single-cell tower, storeys 1..=4, deck at 5.
        a.tower(&[(13, 4)], 1, 5, Reg::FacetMonument, L::Needle);

        // From the Isle, a flight climbs north-east to level 5, then an unrailed span
        // runs on to the Needle.
        let head = a.flight(c(9, 8, 4), F::NorthEast, Reg::FacetMonument);
        a.span(head, F::NorthEast, 2, Reg::FacetMonument);

        // The Gallery: two storeys and a deck, south of the Isle by a railed span.
        for level in 2..4 {
            for &(q, r) in &[(9, 12), (10, 12), (9, 13)] {
                a.put(
                    c(q, r, level),
                    HexSpace::Room,
                    HexArchetype::Room,
                    Reg::Institutional,
                    L::Gallery,
                );
            }
        }
        a.deck(
            &[(9, 12), (10, 12), (9, 13)],
            4,
            Reg::Institutional,
            L::Gallery,
        );
        a.span(c(9, 9, 4), F::SouthEast, 2, Reg::Institutional);

        // The far shore: one cell of air east of the Gallery, and no bridge.
        a.deck(
            &[(12, 12), (13, 12), (12, 13)],
            4,
            Reg::Institutional,
            L::FarShore,
        );
        a.put(
            c(13, 12, 3),
            HexSpace::Room,
            HexArchetype::Room,
            Reg::Institutional,
            L::FarShore,
        );
        a.pavilion(c(13, 12, 4));

        // The understory: two floating decks at level 1 and a railed span between.
        a.deck(
            &[(7, 10), (8, 10), (7, 11)],
            1,
            Reg::Wellshaft,
            L::Understory,
        );
        a.deck(
            &[(11, 10), (12, 10), (11, 11)],
            1,
            Reg::Wellshaft,
            L::Understory,
        );
        a.span(c(8, 10, 1), F::East, 2, Reg::Wellshaft);

        // The Summit: seven storeys, a deck at the top of the lattice, far north-east.
        a.tower(
            &[(17, 2), (18, 2), (17, 3)],
            0,
            7,
            Reg::FacetMonument,
            L::Summit,
        );

        // The distance: a western massif and floaters at the edge of sight.
        a.tower(
            &[(1, 6), (2, 6), (1, 7), (2, 7), (1, 8)],
            0,
            6,
            Reg::Monolith,
            L::Distance,
        );
        a.tower(&[(3, 14), (4, 14)], 1, 2, Reg::Monolith, L::Distance);
        a.deck(&[(15, 15)], 3, Reg::Megastructure, L::Distance);
        a.deck(&[(7, 2), (8, 2)], 5, Reg::Megastructure, L::Distance);
        a.tower(&[(11, 1)], 2, 3, Reg::FacetMonument, L::Distance);
        a.deck(&[(6, 13)], 0, Reg::Wellshaft, L::Distance);
        a.tower(
            &[(19, 9), (20, 9), (19, 10)],
            0,
            4,
            Reg::Megastructure,
            L::Distance,
        );

        let mut world = HexWfcWorld {
            seed: 0,
            generation: 0,
            config: CONFIG,
            placements: a.placements,
            blueprints: Vec::new(),
            architecture: a.architecture,
            cell_revisions: BTreeMap::new(),
            last_attempts: 0,
            authored_pins: BTreeSet::new(),
            space_mix: SpaceMix::baseline(),
            route_corridors: false,
            carve_unrouted: false,
            open_air: false,
            sealed: false,
        };
        let air_cells = world.mark_open_air();

        let tour = vec![
            c(5, 8, 4),
            c(6, 8, 4),
            c(7, 8, 4),
            c(8, 8, 4),
            c(9, 8, 4),
            c(10, 7, 4),
            c(10, 7, 5),
            c(11, 6, 5),
            c(12, 5, 5),
            c(13, 4, 5),
        ];
        Self {
            world,
            landmarks: a.landmarks,
            summit: c(17, 2, 7),
            spawn: c(5, 8, 4),
            tour,
            air_cells,
        }
    }

    /// Every built cell, in coordinate order.
    pub fn built(&self) -> impl Iterator<Item = &HexPlacement> {
        self.world
            .placements
            .values()
            .filter(|placement| placement.space.built())
    }

    #[must_use]
    pub fn register(&self, at: HexCoord) -> ArchitectureRegister {
        self.world
            .architecture
            .get(&at)
            .copied()
            .unwrap_or(ArchitectureRegister::Monolith)
    }

    /// The authored camera poses, in the order the capture shoots them.
    #[must_use]
    pub fn vantages() -> Vec<Vantage> {
        let floor = |at: HexCoord| {
            let [x, y, z] = observed_hex::hex_origin(at);
            [x, y + observed_hex::FLOOR_SLAB_TOP, z]
        };
        let eye = |at: HexCoord, dx: f32, dz: f32| {
            let [x, y, z] = floor(at);
            [x + dx, y + 1.6, z + dz]
        };
        let target = |at: HexCoord, dy: f32| {
            let [x, y, z] = floor(at);
            [x, y + dy, z]
        };
        vec![
            Vantage {
                slug: "bastion_terrace",
                title: "The Bastion terrace, looking out along the railed span",
                eye: eye(c(6, 8, 4), 1.5, 2.5),
                look_at: target(c(9, 9, 3), -2.0),
                standing: true,
            },
            Vantage {
                slug: "mid_span",
                title: "Mid-span: the understory twenty-four metres down",
                eye: eye(c(8, 8, 4), -2.0, 0.0),
                look_at: target(c(10, 10, 1), 0.0),
                standing: true,
            },
            Vantage {
                slug: "stair_foot",
                title: "The flight up to the unrailed span and the Needle",
                eye: eye(c(9, 8, 4), -3.0, 3.0),
                look_at: target(c(12, 5, 5), 2.0),
                standing: true,
            },
            Vantage {
                slug: "needle_edge",
                title: "The Needle's unrailed edge, looking back at the Bastion's cliff",
                eye: eye(c(13, 4, 5), -5.0, -1.0),
                look_at: target(c(5, 9, 1), 0.0),
                standing: true,
            },
            Vantage {
                slug: "gallery_gap",
                title: "The Gallery: the far shore is one cell of air away",
                eye: eye(c(10, 12, 4), 4.0, 0.0),
                look_at: target(c(12, 12, 4), 2.0),
                standing: true,
            },
            Vantage {
                slug: "under_the_isle",
                title: "Under Lantern Isle: the keel and the span's truss",
                eye: target(c(7, 10, 2), 3.0),
                look_at: target(c(9, 8, 3), -4.0),
                standing: false,
            },
            Vantage {
                slug: "from_outside",
                title: "From outside: sheer faces in the moonlight",
                eye: target(c(4, 13, 5), 12.0),
                look_at: target(c(10, 7, 3), 0.0),
                standing: false,
            },
        ]
    }
}
