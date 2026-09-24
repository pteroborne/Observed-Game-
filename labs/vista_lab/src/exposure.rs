//! What each built cell shows to the air around it.
//!
//! This is the whole of the lab's architectural judgement, and it is deliberately a
//! pure function of the facility: a cell's *form* and its *exposure* follow from its
//! space, its ports, and which of its neighbours [`HexWfcWorld::mark_open_air`] called
//! sky. The renderer draws what this module says and nothing else, so a vista that
//! looks wrong is either an authoring mistake or a rule written down here.
//!
//! The rules, in the order they bite:
//!
//! * A face that borders **air** — or the edge of the lattice, which the facility
//!   also treats as open — is *sheer*. Stack sheer faces and you get a cliff.
//! * **Rock does not count.** A face against sealed rock is buried: nobody will
//!   ever see it, and drawing it as a cliff would put a window into stone.
//! * A cell with no structure under it hangs: it gets a keel, and its drop is
//!   measured down through the air to whatever would catch a fall — or to nothing.
//! * A straight hall cell whose four flanks are all air is a **span**: it is drawn
//!   narrow, because nothing beside it needs its full width.
use observed_facility::hex_wfc::{
    HexArchetype, HexCoord, HexFace, HexPlacement, HexSpace, HexWfcWorld, PortClass,
};

/// What kind of architecture a built cell is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Form {
    /// An open floor with nothing enclosing it: a terrace or a floating deck.
    Deck,
    /// An enclosed room with sky over it: walls and a roof.
    Pavilion,
    /// An enclosed storey with more structure on top: one layer of a sheer stack.
    Storey,
    /// A narrow walkway along `axis` (and its opposite), with air on both flanks.
    Span { axis: HexFace },
    /// A stair climbing one level, entered through `entry` and rising toward its
    /// opposite face.
    Flight { entry: HexFace },
    /// The top of a flight. Its floor is the flight's last tread.
    Landing,
}

/// How far a cell hangs over what is below it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Drop {
    /// Structure directly beneath: nothing hangs.
    Supported,
    /// `levels` storeys of open air, then `onto` — or true void if `None`.
    Hanging { levels: u8, onto: Option<HexCoord> },
}

impl Drop {
    /// Metres from this cell's floor to what would catch a fall, or `None` for void.
    #[must_use]
    pub fn metres(self) -> Option<f32> {
        match self {
            Self::Supported => Some(0.0),
            // `levels` counts the air cells in between; floor to floor is one more
            // storey than that.
            Self::Hanging { levels, onto } => {
                onto.map(|_| f32::from(levels + 1) * observed_hex::TILE_LEVEL_HEIGHT)
            }
        }
    }

    #[must_use]
    pub const fn hangs(self) -> bool {
        matches!(self, Self::Hanging { .. })
    }
}

/// One built cell's form and exposure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Exposure {
    pub coord: HexCoord,
    pub form: Form,
    /// Lateral faces that border open air, bit `face.index()`.
    pub sheer: u8,
    pub drop: Drop,
    /// Whether this cell's edges are railed. Low levels are; high ones are not.
    pub railed: bool,
}

impl Exposure {
    #[must_use]
    pub const fn is_sheer(&self, face: HexFace) -> bool {
        face.is_lateral() && self.sheer & (1 << face.index()) != 0
    }

    /// Number of sheer lateral faces.
    #[must_use]
    pub const fn sheer_count(&self) -> u32 {
        self.sheer.count_ones()
    }
}

/// Whether the cell behind `face` is open air. Outside the lattice counts: that is
/// exactly how `mark_open_air` seeds the sky.
#[must_use]
pub fn opens_to_air(world: &HexWfcWorld, at: HexCoord, face: HexFace) -> bool {
    match world.config.grid().neighbor(at, face) {
        None => true,
        Some(next) => world
            .placements
            .get(&next)
            .is_none_or(|placement| placement.space == HexSpace::Air),
    }
}

fn built_at(world: &HexWfcWorld, at: Option<HexCoord>) -> bool {
    at.and_then(|at| world.placements.get(&at))
        .is_some_and(|placement| placement.space.built())
}

fn form(world: &HexWfcWorld, placement: &HexPlacement) -> Form {
    let at = placement.coord;
    let above = world.config.grid().neighbor(at, HexFace::Up);
    let open: Vec<HexFace> = HexFace::LATERAL
        .into_iter()
        .filter(|&face| placement.is_open(face))
        .collect();
    match placement.archetype {
        HexArchetype::RampUp if placement.up == PortClass::RampOpen => {
            if let [entry] = open[..] {
                return Form::Flight { entry };
            }
        }
        HexArchetype::RampHead if placement.down == PortClass::RampOpen => return Form::Landing,
        _ => {}
    }
    match placement.space {
        HexSpace::Room if built_at(world, above) => Form::Storey,
        HexSpace::Room => Form::Pavilion,
        _ => match open[..] {
            [a, b]
                if b == a.opposite()
                    && HexFace::LATERAL
                        .into_iter()
                        .filter(|&face| face != a && face != b)
                        .all(|face| opens_to_air(world, at, face)) =>
            {
                Form::Span { axis: a }
            }
            _ => Form::Deck,
        },
    }
}

fn drop_below(world: &HexWfcWorld, at: HexCoord) -> Drop {
    let grid = world.config.grid();
    let mut levels = 0u8;
    let mut here = at;
    loop {
        match grid.neighbor(here, HexFace::Down) {
            None => return Drop::Hanging { levels, onto: None },
            Some(below) => match world.placements.get(&below).map(|p| p.space) {
                // Rock catches a fall as surely as a floor does; it just is not one.
                Some(space) if space.built() || space == HexSpace::Void => {
                    return if levels == 0 {
                        Drop::Supported
                    } else {
                        Drop::Hanging {
                            levels,
                            onto: Some(below),
                        }
                    };
                }
                _ => {
                    levels += 1;
                    here = below;
                }
            },
        }
    }
}

/// The exposure of one built cell. `None` for anything unbuilt.
#[must_use]
pub fn exposure(world: &HexWfcWorld, at: HexCoord, unsafe_from: u8) -> Option<Exposure> {
    let placement = world.placements.get(&at)?;
    if !placement.space.built() {
        return None;
    }
    let sheer = HexFace::LATERAL
        .into_iter()
        .filter(|&face| opens_to_air(world, at, face))
        .fold(0u8, |mask, face| mask | (1 << face.index()));
    let form = form(world, placement);
    // A flight belongs to the level it arrives at: that is where its drop is worst.
    let rail_level = match form {
        Form::Flight { .. } => at.level + 1,
        _ => at.level,
    };
    Some(Exposure {
        coord: at,
        form,
        sheer,
        drop: drop_below(world, at),
        railed: rail_level < unsafe_from,
    })
}

/// Every built cell's exposure, in coordinate order.
#[must_use]
pub fn survey(world: &HexWfcWorld, unsafe_from: u8) -> Vec<Exposure> {
    world
        .placements
        .keys()
        .filter_map(|&at| exposure(world, at, unsafe_from))
        .collect()
}
