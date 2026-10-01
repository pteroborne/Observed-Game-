//! The Rogue Architect's deterministic finite deck and tile vocabulary.

use observed_content::ArchitectureRegister;
use observed_facility::hex_wfc::HexArchetype;

use super::{HAND_SIZE, Prng};

/// Guardian directives in the Rogue's deck.
const ROGUE_DIRECTIVES: u8 = 4;
/// Sensors in the Rogue's deck.
const ROGUE_SENSORS: u8 = 4;
/// Instability surges in the Rogue's deck.
const ROGUE_SURGES: u8 = 3;

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
    /// Deploy a powered-floor recharge station on a standable built cell.
    Station,
    /// An ascent: a ramp pair climbing from the cell played on to the one above, turned
    /// to the direction of the climb. The only card that builds the way up.
    Stair,
    /// The Rogue's: send the major Guardians to the cell played on (`sim::directive`).
    Directive,
    /// The Rogue's: install a sensor on the cell played on (`sim::sensor`).
    Sensor,
    /// The Rogue's: raise floor disturbance and hasten a pending retraction (`sim::instability`).
    Surge,
}

impl CardKind {
    /// Whether only the Rogue's deck deals it: an order to the facility's machinery, not
    /// architecture.
    #[must_use]
    pub const fn rogue_only(self) -> bool {
        matches!(self, Self::Directive | Self::Sensor | Self::Surge)
    }
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
            (CardKind::Station, _) => "recharge station".to_string(),
            (CardKind::Stair, Some(district)) => format!("stair - {}", district.label()),
            (CardKind::Stair, None) => "stair".to_string(),
            (CardKind::Tile(shape), None) => shape.label().to_string(),
            (CardKind::Directive, _) => "Guardian directive".to_string(),
            (CardKind::Sensor, _) => "sensor".to_string(),
            (CardKind::Surge, _) => "instability surge".to_string(),
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
        Self::with_stairs(seed, levels, shapes, 0)
    }

    /// A deck of `shapes` and `stairs` stair cards in each district, plus doors
    /// and, for the first-person game, deployable recharge stations.
    #[must_use]
    pub fn with_stairs(seed: u64, levels: u8, shapes: &[TileShape], stairs: u8) -> Self {
        let extra = if stairs == 0 {
            vec![(CardKind::Door, 4)]
        } else {
            vec![(CardKind::Door, 4), (CardKind::Station, 4)]
        };
        Self::composed(seed, levels, shapes, stairs, &extra)
    }

    /// The Rogue's deck (design section 7): two of each of `shapes` in each district, the
    /// doors, and the machinery - Guardian directives, sensors and surges - but no way up.
    #[must_use]
    pub fn rogue(seed: u64, levels: u8, shapes: &[TileShape]) -> Self {
        Self::composed(
            seed,
            levels,
            shapes,
            0,
            &[
                (CardKind::Door, 3),
                (CardKind::Directive, ROGUE_DIRECTIVES),
                (CardKind::Sensor, ROGUE_SENSORS),
                (CardKind::Surge, ROGUE_SURGES),
            ],
        )
    }

    /// Two of each of `shapes` and `stairs` stairs in each district, and `extra` cards of
    /// no district, shuffled and dealt.
    fn composed(
        seed: u64,
        levels: u8,
        shapes: &[TileShape],
        stairs: u8,
        extra: &[(CardKind, u8)],
    ) -> Self {
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
            for _ in 0..stairs {
                cards.push(Card {
                    id: CardId(next_id),
                    kind: CardKind::Stair,
                    district: Some(district),
                });
                next_id += 1;
            }
        }
        for &(kind, count) in extra {
            for _ in 0..count {
                cards.push(Card {
                    id: CardId(next_id),
                    kind,
                    district: None,
                });
                next_id += 1;
            }
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

    /// Bring a card of `kind` into the hand, from the draw pile or failing that the
    /// discard, in place of the hand's first card, unless one is held already; whether
    /// the hand now holds one. For evidence captures and tests, as the `stage_*` helpers
    /// are: play draws only by refill.
    pub fn stage_kind(&mut self, kind: CardKind) -> bool {
        if self.hand.iter().any(|card| card.kind == kind) {
            return true;
        }
        if self.hand.is_empty() {
            return false;
        }
        for pile in [&mut self.draw, &mut self.discard] {
            if let Some(index) = pile.iter().position(|card| card.kind == kind) {
                std::mem::swap(&mut self.hand[0], &mut pile[index]);
                return true;
            }
        }
        false
    }

    /// Offer an effect to a live bot hand, preserving a tile when there is a
    /// non-tile card to exchange. A bot with nobody detected needs a sensor to
    /// watch for prey, even when the shuffled five cards dealt it none.
    pub(crate) fn offer_kind(&mut self, kind: CardKind) -> bool {
        if self.hand.iter().any(|card| card.kind == kind) {
            return true;
        }
        let Some(replace) = self
            .hand
            .iter()
            .position(|card| !matches!(card.kind, CardKind::Tile(_)))
            .or_else(|| (!self.hand.is_empty()).then_some(0))
        else {
            return false;
        };
        for pile in [&mut self.draw, &mut self.discard] {
            if let Some(index) = pile.iter().position(|card| card.kind == kind) {
                std::mem::swap(&mut self.hand[replace], &mut pile[index]);
                return true;
            }
        }
        false
    }

    /// Whether the hand holds a tile of `district`.
    #[must_use]
    pub fn has_tile_for(&self, district: District) -> bool {
        self.hand
            .iter()
            .any(|card| matches!(card.kind, CardKind::Tile(_)) && card.district == Some(district))
    }

    /// Bring a tile of `district` into the hand from the draw pile, or failing that the
    /// discard, in place of the hand's least useful card ([`Self::least_useful`]); whether
    /// there was one. Deterministic: the first such card in the pile.
    pub(crate) fn offer_any_tile(&mut self, district: District) -> bool {
        let Some(replace) = self.least_useful(district) else {
            return false;
        };
        let matches =
            |card: &Card| matches!(card.kind, CardKind::Tile(_)) && card.district == Some(district);
        for pile in [&mut self.draw, &mut self.discard] {
            if let Some(index) = pile.iter().position(matches) {
                std::mem::swap(&mut self.hand[replace], &mut pile[index]);
                return true;
            }
        }
        false
    }

    /// The card a hand needing a tile of `district` gives up for one: a tile for another
    /// floor, then a door, then anything but a station, which is how a team recharges and
    /// may be the one card it is holding for. The first such card; `None` for no hand.
    fn least_useful(&self, district: District) -> Option<usize> {
        let first = |wanted: &dyn Fn(&Card) -> bool| self.hand.iter().position(wanted);
        first(&|card| matches!(card.kind, CardKind::Tile(_)) && card.district != Some(district))
            .or_else(|| first(&|card| card.kind == CardKind::Door))
            .or_else(|| first(&|card| card.kind != CardKind::Station))
            .or_else(|| (!self.hand.is_empty()).then_some(0))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A first-person deck whose hand is `kinds` (a tile's district given), the rest back
    /// in the draw pile.
    fn deck_holding(kinds: &[(CardKind, Option<District>)]) -> Deck {
        let mut deck = Deck::with_stairs(7, 2, &TileShape::AUTHORED, 3);
        deck.draw.append(&mut deck.hand);
        for &(kind, district) in kinds {
            let index = deck
                .draw
                .iter()
                .position(|card| {
                    card.kind == kind && (district.is_none() || card.district == district)
                })
                .expect("the deck holds such a card");
            let card = deck.draw.remove(index);
            deck.hand.push(card);
        }
        deck
    }

    fn stations(deck: &Deck) -> usize {
        deck.hand
            .iter()
            .filter(|card| card.kind == CardKind::Station)
            .count()
    }

    /// A hand with nothing for its team's floor draws a tile in, and gives up the least
    /// useful card for it: another floor's tile, then a door, then a stair, never the
    /// station a team recharges by.
    #[test]
    fn a_hand_short_of_a_floors_tile_keeps_its_station() {
        let grid = Some(District::LiminalGrid);
        let corridor = CardKind::Tile(TileShape::Corridor);
        for (hand, gives_up) in [
            (
                vec![
                    (CardKind::Station, None),
                    (corridor, grid),
                    (CardKind::Door, None),
                    (CardKind::Stair, None),
                    (corridor, grid),
                ],
                corridor,
            ),
            (
                vec![
                    (CardKind::Station, None),
                    (CardKind::Stair, None),
                    (CardKind::Door, None),
                    (CardKind::Stair, None),
                    (CardKind::Station, None),
                ],
                CardKind::Door,
            ),
            (
                vec![
                    (CardKind::Station, None),
                    (CardKind::Stair, None),
                    (CardKind::Station, None),
                    (CardKind::Stair, None),
                    (CardKind::Station, None),
                ],
                CardKind::Stair,
            ),
        ] {
            let mut deck = deck_holding(&hand);
            let before = deck.hand.clone();
            assert!(deck.offer_any_tile(District::Institutional));
            assert!(deck.has_tile_for(District::Institutional));
            assert_eq!(
                stations(&deck),
                before
                    .iter()
                    .filter(|c| c.kind == CardKind::Station)
                    .count()
            );
            let lost: Vec<_> = before
                .iter()
                .filter(|card| !deck.hand.contains(card))
                .collect();
            assert_eq!(lost.len(), 1, "{hand:?}");
            assert_eq!(lost[0].kind, gives_up, "{hand:?}");
        }
    }
}
