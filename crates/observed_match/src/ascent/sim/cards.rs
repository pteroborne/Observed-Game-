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

/// A floor's district (design section 6): its architecture and a card constraint, one a
/// floor, climbing [`ArchitectureRegister::CLIMB`]. Floors sharing a register on the climb
/// share a district, and so its cards.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct District(ArchitectureRegister);

impl District {
    /// The ground floor's: the Backrooms, on a facility of any height.
    pub const GROUND: Self = Self(ArchitectureRegister::CLIMB[0]);
    pub const LIBRARY: Self = Self(ArchitectureRegister::InfiniteGallery);
    pub const ZEN: Self = Self(ArchitectureRegister::ShadowScreen);
    pub const REACTOR: Self = Self(ArchitectureRegister::Megastructure);

    /// The district of floor `level` of a facility `levels` tall.
    #[must_use]
    pub const fn for_floor(level: u8, levels: u8) -> Self {
        Self(ArchitectureRegister::for_floor(level, levels))
    }

    /// Every district of a facility `levels` tall, once each, from the ground up.
    #[must_use]
    pub fn climb(levels: u8) -> Vec<Self> {
        let mut climb: Vec<Self> = Vec::new();
        for level in 0..levels.max(1) {
            let district = Self::for_floor(level, levels);
            if !climb.contains(&district) {
                climb.push(district);
            }
        }
        climb
    }

    /// The place a player knows the floor as. The register's own name says how it is
    /// built; this says where you are.
    #[must_use]
    pub const fn label(self) -> &'static str {
        use ArchitectureRegister as R;
        match self.0 {
            R::LiminalGrid => "Backrooms",
            R::InfiniteGallery => "Library",
            R::OverlitGrid => "Lumen",
            R::ShadowScreen => "Zen",
            R::FacetMonument => "Monument",
            R::Megastructure => "Reactor",
            R::Thinning => "Sky",
            R::Monolith => "Monolith",
            R::Institutional => "Institutional",
            R::Wellshaft => "Silo",
        }
    }

    #[must_use]
    pub const fn register(self) -> ArchitectureRegister {
        self.0
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
    /// An ascent: a climb composition, three cells of flight from the cell played on and
    /// a landing above the last, turned to the direction of the climb. The only card that
    /// builds the way up.
    Stair,
    /// A multi-tile liminal room: a vast 3-hex water basin and column hall placed
    /// atomically by the Architect. Sibling faces are opened with `Expanse` geometry.
    Cistern,
    /// One Reactor card places the fabrication, transfer and receiving spaces.
    Chargeworks,
    /// The Rogue's: send the major Guardians to the cell played on (`sim::directive`).
    Directive,
    /// The Rogue's: install a sensor on the cell played on (`sim::sensor`).
    Sensor,
    /// The Rogue's: raise floor disturbance and hasten a pending retraction (`sim::instability`).
    Surge,
    /// One Library card places a reading well and its elevated gallery circuit.
    ArchiveWell,
    /// One Zen card places a sheltered veranda around an indoor rain garden.
    RainCourt,
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
            (CardKind::Cistern, Some(district)) => format!("cistern - {}", district.label()),
            (CardKind::Cistern, None) => "cistern".to_string(),
            (CardKind::Chargeworks, Some(district)) => {
                format!("chargeworks - {}", district.label())
            }
            (CardKind::Chargeworks, None) => "chargeworks".to_string(),
            (CardKind::ArchiveWell, Some(district)) => {
                format!("archive well - {}", district.label())
            }
            (CardKind::ArchiveWell, None) => "archive well".to_string(),
            (CardKind::RainCourt, Some(district)) => format!("rain court - {}", district.label()),
            (CardKind::RainCourt, None) => "rain court".to_string(),
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
    /// The facility's districts from the ground up ([`District::climb`]).
    climb: Vec<District>,
    /// How many of them, from the ground, a refill deals. A team's deck opens a district
    /// as its bodies reach that floor (design section 6); every other deck deals them all.
    reach: usize,
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

    /// Two of each of `shapes` and `stairs` stair cards in each district, Library Archive Well, Backrooms Cistern and Reactor
    /// Chargeworks cards, plus doors and deployable recharge stations.
    #[must_use]
    pub fn with_stairs_and_wonders(
        seed: u64,
        levels: u8,
        shapes: &[TileShape],
        stairs: u8,
        wonders: u8,
    ) -> Self {
        Self::composed(
            seed,
            levels,
            shapes,
            stairs,
            wonders,
            &Self::loyal_extras(stairs),
            usize::MAX,
        )
    }

    /// A deck of `shapes` and `stairs` stair cards in each district, plus doors
    /// and, for the first-person game, deployable recharge stations.
    #[must_use]
    pub fn with_stairs(seed: u64, levels: u8, shapes: &[TileShape], stairs: u8) -> Self {
        Self::with_stairs_and_wonders(seed, levels, shapes, stairs, 0)
    }

    /// A team's deck: [`Self::with_stairs_and_wonders`], dealing only the ground floor's district until
    /// the team's bodies reach another ([`Self::open_through`]). Includes one multi-tile
    /// Library Archive Well, Zen Rain Court, Backrooms Cistern and Reactor Chargeworks. Seven districts' tiles dealt from the start would leave most
    /// of a hand for floors nobody can reach.
    #[must_use]
    pub fn for_team(seed: u64, levels: u8, shapes: &[TileShape], stairs: u8) -> Self {
        Self::for_team_with_wonders(seed, levels, shapes, stairs, 1)
    }

    /// A team's deck with an explicit number of each district's authored wonder cards.
    #[must_use]
    pub fn for_team_with_wonders(
        seed: u64,
        levels: u8,
        shapes: &[TileShape],
        stairs: u8,
        wonders: u8,
    ) -> Self {
        Self::composed(
            seed,
            levels,
            shapes,
            stairs,
            wonders,
            &Self::loyal_extras(stairs),
            1,
        )
    }

    fn loyal_extras(stairs: u8) -> Vec<(CardKind, u8)> {
        if stairs == 0 {
            vec![(CardKind::Door, 4)]
        } else {
            vec![(CardKind::Door, 4), (CardKind::Station, 4)]
        }
    }

    /// The Rogue's deck (design section 7): two of each of `shapes` in each district,
    /// one of each authored wonder where its district exists, the doors, and the machinery -
    /// Guardian directives, sensors and surges - but no way up.
    #[must_use]
    pub fn rogue(seed: u64, levels: u8, shapes: &[TileShape]) -> Self {
        Self::composed(
            seed,
            levels,
            shapes,
            0,
            1,
            &[
                (CardKind::Door, 3),
                (CardKind::Directive, ROGUE_DIRECTIVES),
                (CardKind::Sensor, ROGUE_SENSORS),
                (CardKind::Surge, ROGUE_SURGES),
            ],
            usize::MAX,
        )
    }

    /// Two of each of `shapes` and `stairs` stairs in each district, district wonders,
    /// and `extra` cards of no district, shuffled and dealt from the first `reach` districts.
    fn composed(
        seed: u64,
        levels: u8,
        shapes: &[TileShape],
        stairs: u8,
        wonders: u8,
        extra: &[(CardKind, u8)],
        reach: usize,
    ) -> Self {
        let climb = District::climb(levels);
        let mut cards = Vec::new();
        let mut next_id = 0;
        for &district in &climb {
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
            let wonder = if district == District::GROUND {
                Some(CardKind::Cistern)
            } else if district == District::REACTOR {
                Some(CardKind::Chargeworks)
            } else {
                None
            };
            for kind in wonder
                .into_iter()
                .flat_map(|kind| std::iter::repeat_n(kind, usize::from(wonders)))
            {
                cards.push(Card {
                    id: CardId(next_id),
                    kind,
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
        // Append new content so existing card identities retain their district and kind.
        if climb.contains(&District::LIBRARY) {
            for _ in 0..wonders {
                cards.push(Card {
                    id: CardId(next_id),
                    kind: CardKind::ArchiveWell,
                    district: Some(District::LIBRARY),
                });
                next_id += 1;
            }
        }
        if climb.contains(&District::ZEN) {
            for _ in 0..wonders {
                cards.push(Card {
                    id: CardId(next_id),
                    kind: CardKind::RainCourt,
                    district: Some(District::ZEN),
                });
                next_id += 1;
            }
        }
        let mut deck = Self {
            hand: Vec::new(),
            draw: cards,
            discard: Vec::new(),
            rng: Prng(seed ^ 0xA8C4_17EC_700D_0001),
            reach: reach.min(climb.len()),
            climb,
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
        self.stage_matching(|card| card.kind == kind)
    }

    /// A fixture needing a local play must not depend on the shuffled hand's
    /// district. Staging still exchanges a real card from the finite deck.
    #[cfg(test)]
    pub(crate) fn stage_in_district(&mut self, kind: CardKind, district: District) -> bool {
        self.stage_matching(|card| card.kind == kind && card.district == Some(district))
    }

    fn stage_matching(&mut self, matches: impl Fn(&Card) -> bool) -> bool {
        if self.hand.iter().any(&matches) {
            return true;
        }
        if self.hand.is_empty() {
            return false;
        }
        for pile in [&mut self.draw, &mut self.discard] {
            if let Some(index) = pile.iter().position(&matches) {
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

    /// Open every district up to and including `district`'s for refills: the deck's team
    /// has reached its floor. Never closes one.
    pub(crate) fn open_through(&mut self, district: District) {
        if let Some(index) = self.climb.iter().position(|&open| open == district) {
            self.reach = self.reach.max(index + 1);
        }
    }

    /// Whether a refill may deal `card`: no district, or one the deck has opened.
    fn dealt(&self, card: &Card) -> bool {
        card.district
            .is_none_or(|district| self.climb[..self.reach].contains(&district))
    }

    /// Top the hand up from the end of the draw pile, passing over any card of a district
    /// not yet open; reshuffle the discard in when the pile holds none to deal.
    pub(crate) fn refill(&mut self) {
        while self.hand.len() < HAND_SIZE {
            let next = self.draw.iter().rposition(|card| self.dealt(card));
            if let Some(index) = next {
                let card = self.draw.remove(index);
                self.hand.push(card);
                continue;
            }
            if !self.discard.iter().any(|card| self.dealt(card)) {
                break;
            }
            self.draw.append(&mut self.discard);
            self.shuffle_draw();
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
    use std::collections::BTreeSet;

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
        let upper = Some(District::for_floor(1, 2));
        let corridor = CardKind::Tile(TileShape::Corridor);
        for (hand, gives_up) in [
            (
                vec![
                    (CardKind::Station, None),
                    (corridor, upper),
                    (CardKind::Door, None),
                    (CardKind::Stair, None),
                    (corridor, upper),
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
            assert!(deck.offer_any_tile(District::GROUND));
            assert!(deck.has_tile_for(District::GROUND));
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

    /// One district a floor, from the Backrooms to the sky; floors sharing a register
    /// share a district.
    #[test]
    fn an_eight_floor_climb_has_seven_districts() {
        let climb = District::climb(8);
        assert_eq!(climb.len(), 7);
        assert_eq!(climb[0], District::GROUND);
        assert_eq!(climb[0].label(), "Backrooms");
        assert_eq!(climb[6].label(), "Sky");
        assert_eq!(District::for_floor(4, 8), District::for_floor(5, 8));
        assert_eq!(District::climb(1), vec![District::GROUND]);
    }

    #[test]
    fn cistern_wonders_belong_only_to_the_backrooms() {
        for deck in [
            Deck::for_team(7, 8, &TileShape::AUTHORED, 3),
            Deck::rogue(7, 8, &TileShape::AUTHORED),
        ] {
            let cisterns: Vec<_> = deck
                .hand
                .iter()
                .chain(&deck.draw)
                .filter(|card| card.kind == CardKind::Cistern)
                .collect();
            assert_eq!(cisterns.len(), 1);
            assert_eq!(cisterns[0].district, Some(District::GROUND));
        }
    }

    #[test]
    fn chargeworks_is_a_single_reactor_wonder_in_both_finite_decks() {
        for deck in [
            Deck::for_team(7, 8, &TileShape::AUTHORED, 3),
            Deck::rogue(7, 8, &TileShape::AUTHORED),
        ] {
            let cards: Vec<_> = deck
                .hand
                .iter()
                .chain(&deck.draw)
                .filter(|c| c.kind == CardKind::Chargeworks)
                .collect();
            assert_eq!(cards.len(), 1);
            assert_eq!(cards[0].district, Some(District::REACTOR));
        }
        let deck = Deck::for_team(7, 1, &TileShape::AUTHORED, 3);
        assert!(
            !deck
                .hand
                .iter()
                .chain(&deck.draw)
                .any(|c| c.kind == CardKind::Chargeworks)
        );
    }

    #[test]
    fn archive_is_a_single_library_wonder_in_both_finite_decks() {
        for deck in [
            Deck::for_team(7, 8, &TileShape::AUTHORED, 3),
            Deck::rogue(7, 8, &TileShape::AUTHORED),
        ] {
            let archive: Vec<_> = deck
                .hand
                .iter()
                .chain(&deck.draw)
                .filter(|c| c.kind == CardKind::ArchiveWell)
                .collect();
            assert_eq!(archive.len(), 1);
            assert_eq!(archive[0].district, Some(District::LIBRARY));
        }
        let deck = Deck::for_team(7, 1, &TileShape::AUTHORED, 3);
        assert!(
            !deck
                .hand
                .iter()
                .chain(&deck.draw)
                .any(|c| c.kind == CardKind::ArchiveWell)
        );
    }

    /// A team's deck deals the ground floor's tiles and none of a floor its bodies have
    /// not reached, however often it is dealt; reaching a floor opens it and those below.
    #[test]
    fn a_team_deals_only_the_districts_it_has_reached() {
        let tiles_of = |deck: &Deck| {
            deck.hand
                .iter()
                .filter_map(|card| card.district)
                .collect::<BTreeSet<_>>()
        };
        let mut deck = Deck::for_team(7, 8, &TileShape::AUTHORED, 3);
        let mut seen = BTreeSet::new();
        for _ in 0..40 {
            seen.extend(tiles_of(&deck));
            let card = deck.hand[0].id;
            assert!(deck.spend(card));
            assert_eq!(deck.hand.len(), HAND_SIZE);
        }
        assert_eq!(seen, BTreeSet::from([District::GROUND]));

        let third = District::for_floor(2, 8);
        deck.open_through(third);
        let mut seen = BTreeSet::new();
        for _ in 0..80 {
            seen.extend(tiles_of(&deck));
            let card = deck.hand[0].id;
            assert!(deck.spend(card));
        }
        assert_eq!(
            seen,
            District::climb(8)[..3]
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
        );

        // A Rogue deals every district from the start.
        let rogue = Deck::rogue(7, 8, &TileShape::AUTHORED);
        assert_eq!(rogue.reach, 7);
    }
    #[test]
    fn rain_court_is_finite_zen_content_appended_after_existing_card_ids() {
        for deck in [
            Deck::for_team(7, 8, &TileShape::AUTHORED, 8),
            Deck::rogue(7, 8, &TileShape::AUTHORED),
        ] {
            let all: Vec<_> = deck
                .hand
                .iter()
                .chain(&deck.draw)
                .chain(&deck.discard)
                .collect();
            let rain: Vec<_> = all
                .iter()
                .filter(|c| c.kind == CardKind::RainCourt)
                .collect();
            assert_eq!(rain.len(), 1);
            assert_eq!(rain[0].district, Some(District::ZEN));
            assert!(
                all.iter()
                    .filter(|c| c.kind != CardKind::RainCourt)
                    .all(|c| c.id.0 < rain[0].id.0)
            );
        }
        let deck = Deck::for_team(7, 1, &TileShape::AUTHORED, 1);
        assert!(
            !deck
                .hand
                .iter()
                .chain(&deck.draw)
                .any(|c| c.kind == CardKind::RainCourt)
        );
    }
}
