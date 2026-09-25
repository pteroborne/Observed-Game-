//! What an architect holds and plays.
//!
//! A tile is a **cell's whole port signature** — which of its six faces are
//! doorways — rather than a single boundary. That is the WFC unit, and it is
//! what makes a hand of tiles mean something: a `Corridor` and a `Junction` are
//! different offers, and choosing between them is a decision. Boundary-level
//! plays would be simpler and would test nothing, because every play would be
//! the same play.
//!
//! It also gives the model the rooms-versus-corridors distinction the north
//! star is built on, which a lattice of identical cells could not express.

use observed_hex::coords::HexCoord;
use observed_hex::faces::HexFace;
use observed_hex::ports::PortClass;

/// The shapes an architect can hold. Named by what they do to movement, not by
/// how they look, because that is what a player is choosing between.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TileShape {
    /// No doorways. Floor nobody can reach or leave.
    Sealed,
    /// One doorway: a pocket you can only back out of.
    DeadEnd,
    /// Two opposite doorways — the through-route.
    Corridor,
    /// Two adjacent doorways — a corner that turns you.
    Bend,
    /// Three alternating doorways: a real choice of exits.
    Junction,
    /// Four doorways. Somewhere to decide in.
    Hall,
    /// Every doorway open. Maximum connection, minimum control.
    Room,
}

impl TileShape {
    pub const ALL: [TileShape; 7] = [
        TileShape::Sealed,
        TileShape::DeadEnd,
        TileShape::Corridor,
        TileShape::Bend,
        TileShape::Junction,
        TileShape::Hall,
        TileShape::Room,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            TileShape::Sealed => "sealed",
            TileShape::DeadEnd => "dead end",
            TileShape::Corridor => "corridor",
            TileShape::Bend => "bend",
            TileShape::Junction => "junction",
            TileShape::Hall => "hall",
            TileShape::Room => "room",
        }
    }

    /// Which lateral faces are doorways at rotation zero, indexed by
    /// [`HexFace::index`].
    #[must_use]
    pub const fn doors(self) -> [bool; 6] {
        match self {
            TileShape::Sealed => [false; 6],
            TileShape::DeadEnd => [true, false, false, false, false, false],
            TileShape::Corridor => [true, false, false, true, false, false],
            TileShape::Bend => [true, true, false, false, false, false],
            TileShape::Junction => [true, false, true, false, true, false],
            TileShape::Hall => [true, true, false, true, true, false],
            TileShape::Room => [true; 6],
        }
    }

    #[must_use]
    pub const fn doorway_count(self) -> usize {
        let doors = self.doors();
        let mut count = 0;
        let mut i = 0;
        while i < 6 {
            if doors[i] {
                count += 1;
            }
            i += 1;
        }
        count
    }

    /// Number of visually distinct sixth-turn orientations.
    ///
    /// Symmetric cards should never make a player press Rotate only to see the
    /// same card again. A room and a sealed cell have one orientation, the
    /// alternating junction has two, opposite corridors and four-door halls
    /// have three, and the asymmetric shapes use all six.
    #[must_use]
    pub const fn rotation_period(self) -> u8 {
        match self {
            TileShape::Sealed | TileShape::Room => 1,
            TileShape::Junction => 2,
            TileShape::Corridor | TileShape::Hall => 3,
            TileShape::DeadEnd | TileShape::Bend => 6,
        }
    }

    /// Collapse any sixth-turn rotation onto this shape's distinct range.
    #[must_use]
    pub const fn normalize_rotation(self, rotation: u8) -> u8 {
        rotation % self.rotation_period()
    }

    /// The port on `face` once the shape is turned by `rotation` sixths.
    #[must_use]
    pub fn port(self, rotation: u8, face: HexFace) -> PortClass {
        let index = (face.index() + 6 - self.normalize_rotation(rotation) as usize) % 6;
        if self.doors()[index] {
            PortClass::Door
        } else {
            PortClass::Sealed
        }
    }
}

/// One card, played at one cell, turned one way.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TilePlay {
    pub cell: HexCoord,
    pub shape: TileShape,
    pub rotation: u8,
}

/// Why a play was not allowed. Refusals are where an architect's skill lives,
/// so they are named rather than silently dropped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// Off the hexagon.
    OffBoard,
    /// Somebody is standing there or looking at it. This is observe-to-freeze
    /// seen from the other side of the table: the architect may not rebuild
    /// what the operatives are holding, including their own.
    Held,
    /// Flags and prisons are fixed points; rebuilding around them makes a match
    /// unwinnable by accident.
    Protected,
    /// It would strand part of the facility.
    WouldDisconnect,
    /// The card already matches every interior boundary at this cell. Spending
    /// a scarce card for no visible result is never a useful successful play.
    NoEffect,
    /// That shape is not in hand.
    NotInHand,
    /// The architect has already played its allowance this turn.
    NoPlaysLeft,
}

impl Refusal {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Refusal::OffBoard => "not on the board",
            Refusal::Held => "held - somebody is standing there or watching it",
            Refusal::Protected => "a flag or prison sits there",
            Refusal::WouldDisconnect => "it would strand part of the facility",
            Refusal::NoEffect => "the tile already has that shape",
            Refusal::NotInHand => "that tile is not in hand",
            Refusal::NoPlaysLeft => "no plays left this turn",
        }
    }
}

/// An architect's hand and the deck behind it.
///
/// The hand is the cadence throttle, and it is also the fix for a problem this
/// lab already measured: unbounded churn produced fifty-odd changes a turn and
/// read as noise. A player holding four tiles cannot flood the board even if
/// they want to, so what lands is always few enough to be read as intent.
#[derive(Clone, Debug)]
pub struct Hand {
    pub cards: Vec<TileShape>,
    pub deck: Vec<TileShape>,
    pub discard: Vec<TileShape>,
    /// How many may be played per turn.
    pub plays_per_turn: u8,
    pub played_this_turn: u8,
    pub size: u8,
    /// Cards the operatives have earned and the architect has not yet drawn.
    ///
    /// Nothing refills on a clock. An architect who spends without their squad
    /// achieving anything simply runs out, which is what points the dependency
    /// the right way round: the operatives generate the resource and the
    /// architect spends it on their behalf. A free trickle would make the
    /// architect self-sufficient and the operatives optional.
    pub owed: u8,
}

impl Hand {
    /// A starting deck weighted toward the middling shapes, because a deck of
    /// `Sealed` and `Room` is a deck of two buttons.
    #[must_use]
    pub fn new(size: u8, plays_per_turn: u8) -> Self {
        let mut deck = Vec::new();
        for (shape, copies) in [
            (TileShape::Corridor, 5),
            (TileShape::Bend, 5),
            (TileShape::Junction, 4),
            (TileShape::DeadEnd, 3),
            (TileShape::Hall, 3),
            (TileShape::Sealed, 2),
            (TileShape::Room, 2),
        ] {
            for _ in 0..copies {
                deck.push(shape);
            }
        }
        Self {
            cards: Vec::new(),
            deck,
            discard: Vec::new(),
            plays_per_turn,
            played_this_turn: 0,
            size,
            owed: 0,
        }
    }

    #[must_use]
    pub fn holds(&self, shape: TileShape) -> bool {
        self.cards.contains(&shape)
    }

    pub fn spend(&mut self, shape: TileShape) -> bool {
        if let Some(index) = self.cards.iter().position(|card| *card == shape) {
            self.cards.remove(index);
            self.discard.push(shape);
            self.played_this_turn += 1;
            true
        } else {
            false
        }
    }

    /// Deal the opening hand. Only used at setup; afterwards a card arrives
    /// because somebody earned it.
    pub fn deal_opening(&mut self, rng: &mut crate::sim::prng::Prng) {
        self.owed = self.size;
        self.draw_owed(rng);
    }

    /// Draw whatever the squad has earned, up to hand size, reshuffling the
    /// discard when the deck runs dry so a long match never stops offering
    /// tiles entirely.
    pub fn draw_owed(&mut self, rng: &mut crate::sim::prng::Prng) {
        while self.owed > 0 && self.cards.len() < self.size as usize {
            self.owed -= 1;
            if self.deck.is_empty() {
                if self.discard.is_empty() {
                    break;
                }
                self.deck.append(&mut self.discard);
                rng.shuffle(&mut self.deck);
            }
            let Some(index) = rng.below(self.deck.len()) else {
                break;
            };
            self.cards.push(self.deck.remove(index));
        }
        // Earning beyond a full hand is not banked indefinitely; a hand is a
        // cadence, not a warehouse.
        self.owed = self.owed.min(self.size);
    }
}
