//! The Rogue Architect's deterministic finite deck and tile vocabulary.

use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexArchetype;

use super::{HAND_SIZE, Prng};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CardId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum District {
    Institutional,
    LiminalGrid,
}

impl District {
    #[must_use]
    pub const fn for_level(level: u8) -> Self {
        if level == 0 {
            Self::Institutional
        } else {
            Self::LiminalGrid
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Institutional => "Institutional",
            Self::LiminalGrid => "Liminal Grid",
        }
    }

    #[must_use]
    pub const fn register(self) -> ArchitectureRegister {
        match self {
            Self::Institutional => ArchitectureRegister::Institutional,
            Self::LiminalGrid => ArchitectureRegister::LiminalGrid,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileShape {
    DeadEnd,
    Corridor,
    Bend,
    Junction,
    Hall,
}

impl TileShape {
    pub const ALL: [Self; 5] = [
        Self::DeadEnd,
        Self::Corridor,
        Self::Bend,
        Self::Junction,
        Self::Hall,
    ];

    /// The shapes the authored corpus builds as a flat hall. It has no one-door hall, so a
    /// first-person facility deals no dead end.
    pub const AUTHORED: [Self; 4] = [Self::Corridor, Self::Bend, Self::Junction, Self::Hall];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DeadEnd => "dead end / 1",
            Self::Corridor => "corridor / 2",
            Self::Bend => "bend / 2",
            Self::Junction => "junction / 3",
            Self::Hall => "hall / 4",
        }
    }

    #[must_use]
    pub const fn archetype(self) -> HexArchetype {
        match self {
            Self::DeadEnd | Self::Corridor => HexArchetype::Straight,
            Self::Bend => HexArchetype::Corner,
            Self::Junction | Self::Hall => HexArchetype::Junction,
        }
    }

    #[must_use]
    pub const fn base_doors(self) -> u8 {
        match self {
            Self::DeadEnd => 0b00_0001,
            Self::Corridor => 0b00_1001,
            Self::Bend => 0b00_0011,
            Self::Junction => 0b01_0101,
            Self::Hall => 0b01_1011,
        }
    }

    #[must_use]
    pub const fn doors(self, rotation: u8) -> u8 {
        let rotation = rotation % 6;
        let mask = self.base_doors();
        ((mask << rotation) | (mask >> (6 - rotation))) & 0b00_111111
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardKind {
    Tile(TileShape),
    Door,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Card {
    pub id: CardId,
    pub kind: CardKind,
    pub district: Option<District>,
}

impl Card {
    #[must_use]
    pub fn label(self) -> String {
        match (self.kind, self.district) {
            (CardKind::Tile(shape), Some(district)) => {
                format!("{} - {}", shape.label(), district.label())
            }
            (CardKind::Door, _) => "deployable door".to_string(),
            (CardKind::Tile(shape), None) => shape.label().to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Deck {
    pub hand: Vec<Card>,
    draw: Vec<Card>,
    discard: Vec<Card>,
    rng: Prng,
}

impl Deck {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::for_levels(seed, 2)
    }

    #[must_use]
    pub fn for_levels(seed: u64, levels: u8) -> Self {
        Self::with_shapes(seed, levels, &TileShape::ALL)
    }

    /// Two of each of `shapes` per district, and four doors.
    #[must_use]
    pub fn with_shapes(seed: u64, levels: u8, shapes: &[TileShape]) -> Self {
        let mut cards = Vec::new();
        let mut next_id = 0;
        for district in [District::Institutional, District::LiminalGrid]
            .into_iter()
            .take(usize::from(levels).min(2))
        {
            for &shape in shapes {
                for _ in 0..2 {
                    cards.push(Card {
                        id: CardId(next_id),
                        kind: CardKind::Tile(shape),
                        district: Some(district),
                    });
                    next_id += 1;
                }
            }
        }
        for _ in 0..4 {
            cards.push(Card {
                id: CardId(next_id),
                kind: CardKind::Door,
                district: None,
            });
            next_id += 1;
        }
        let mut deck = Self {
            hand: Vec::new(),
            draw: cards,
            discard: Vec::new(),
            rng: Prng(seed ^ 0xA8C4_17EC_700D_0001),
        };
        deck.shuffle_draw();
        deck.refill();
        deck
    }

    pub(super) fn offer_tile(&mut self, shape: TileShape, district: District) {
        let matches =
            |card: &Card| card.kind == CardKind::Tile(shape) && card.district == Some(district);
        if let Some(index) = self.hand.iter().position(matches) {
            self.hand.swap(0, index);
        } else if let Some(index) = self.draw.iter().position(matches) {
            std::mem::swap(&mut self.hand[0], &mut self.draw[index]);
        }
    }

    fn shuffle_draw(&mut self) {
        for index in (1..self.draw.len()).rev() {
            let swap = self.rng.below(index + 1);
            self.draw.swap(index, swap);
        }
    }

    pub(crate) fn refill(&mut self) {
        while self.hand.len() < HAND_SIZE {
            if self.draw.is_empty() {
                if self.discard.is_empty() {
                    break;
                }
                self.draw.append(&mut self.discard);
                self.shuffle_draw();
            }
            if let Some(card) = self.draw.pop() {
                self.hand.push(card);
            }
        }
    }

    pub(crate) fn emergency_refill(&mut self) {
        if self.hand.len() == HAND_SIZE {
            self.discard.append(&mut self.hand);
        }
        self.refill();
    }

    pub(super) fn retire_district(&mut self, district: District) {
        self.hand.retain(|card| card.district != Some(district));
        self.draw.retain(|card| card.district != Some(district));
        self.discard.retain(|card| card.district != Some(district));
        self.refill();
    }

    pub(super) fn spend(&mut self, id: CardId) -> bool {
        let Some(index) = self.hand.iter().position(|card| card.id == id) else {
            return false;
        };
        self.discard.push(self.hand.remove(index));
        self.refill();
        true
    }
}
