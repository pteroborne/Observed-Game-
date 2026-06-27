//! Phase 18: **progression & cosmetics** — the final carried-forward feasibility
//! lab. It is a persistence layer for unlocks and cosmetics that, by construction,
//! **never touches simulation determinism**.
//!
//! A `Profile` accrues XP from match placements, levels up, and unlocks cosmetics
//! by level or win thresholds; cosmetics are equipped per slot; the whole profile
//! serializes to a compact string and round-trips. The defining property is
//! *orthogonality*: nothing here feeds the match. The match is run by the proven
//! [`competitive_facility`] brain, which takes no profile and reads no cosmetics, so
//! the result is identical no matter what is unlocked or equipped — proven by a test
//! that plays the match before and after changing the profile.

use std::collections::{BTreeMap, BTreeSet};

/// Cumulative XP required to *reach* each level. `level_for_xp` walks this.
const THRESHOLDS: [u32; 8] = [0, 100, 250, 450, 700, 1000, 1400, 1900];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Slot {
    Color,
    Trail,
    Badge,
}

impl Slot {
    pub fn label(self) -> &'static str {
        match self {
            Slot::Color => "Color",
            Slot::Trail => "Trail",
            Slot::Badge => "Badge",
        }
    }
}

/// What unlocks a cosmetic. Progression is the *only* input; never the reverse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unlock {
    Level(u32),
    Wins(u32),
}

#[derive(Clone, Copy, Debug)]
pub struct Cosmetic {
    pub id: u16,
    pub name: &'static str,
    pub slot: Slot,
    pub unlock: Unlock,
}

/// The authored, data-driven cosmetic catalog.
pub fn catalog() -> Vec<Cosmetic> {
    vec![
        Cosmetic {
            id: 0,
            name: "Ash",
            slot: Slot::Color,
            unlock: Unlock::Level(0),
        },
        Cosmetic {
            id: 1,
            name: "Ember",
            slot: Slot::Color,
            unlock: Unlock::Level(2),
        },
        Cosmetic {
            id: 2,
            name: "Cobalt",
            slot: Slot::Color,
            unlock: Unlock::Level(4),
        },
        Cosmetic {
            id: 3,
            name: "Void",
            slot: Slot::Color,
            unlock: Unlock::Wins(3),
        },
        Cosmetic {
            id: 4,
            name: "No Trail",
            slot: Slot::Trail,
            unlock: Unlock::Level(0),
        },
        Cosmetic {
            id: 5,
            name: "Spark",
            slot: Slot::Trail,
            unlock: Unlock::Level(1),
        },
        Cosmetic {
            id: 6,
            name: "Comet",
            slot: Slot::Trail,
            unlock: Unlock::Level(3),
        },
        Cosmetic {
            id: 7,
            name: "Rookie",
            slot: Slot::Badge,
            unlock: Unlock::Level(0),
        },
        Cosmetic {
            id: 8,
            name: "Veteran",
            slot: Slot::Badge,
            unlock: Unlock::Level(5),
        },
        Cosmetic {
            id: 9,
            name: "Champion",
            slot: Slot::Badge,
            unlock: Unlock::Wins(1),
        },
    ]
}

pub fn cosmetic(id: u16) -> Option<Cosmetic> {
    catalog().into_iter().find(|c| c.id == id)
}

pub fn level_for_xp(xp: u32) -> u32 {
    THRESHOLDS
        .iter()
        .rev()
        .position(|t| xp >= *t)
        .map(|p| (THRESHOLDS.len() - 1 - p) as u32)
        .unwrap_or(0)
}

/// XP toward the next level and what that level needs, for a progress bar.
pub fn level_progress(xp: u32) -> (u32, u32) {
    let level = level_for_xp(xp) as usize;
    let base = THRESHOLDS[level];
    if level + 1 < THRESHOLDS.len() {
        (xp - base, THRESHOLDS[level + 1] - base)
    } else {
        (1, 1) // maxed
    }
}

/// XP awarded for a finishing placement (`Some(rank)`) or being absorbed (`None`).
pub fn placement_xp(placement: Option<u8>) -> u32 {
    let base = 20;
    base + match placement {
        Some(1) => 120,
        Some(2) => 80,
        Some(_) => 50,
        None => 30,
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub xp: u32,
    pub matches_played: u32,
    pub wins: u32,
    pub unlocked: BTreeSet<u16>,
    pub equipped: BTreeMap<Slot, u16>,
    pub last_event: String,
}

impl Default for Profile {
    fn default() -> Self {
        Self::new()
    }
}

impl Profile {
    pub fn new() -> Self {
        let mut profile = Self {
            xp: 0,
            matches_played: 0,
            wins: 0,
            unlocked: BTreeSet::new(),
            equipped: BTreeMap::new(),
            last_event: "New profile — play a match to earn XP.".to_string(),
        };
        profile.refresh_unlocks();
        // Equip the default (level-0) cosmetic in each slot.
        for slot in [Slot::Color, Slot::Trail, Slot::Badge] {
            if let Some(c) = catalog()
                .into_iter()
                .find(|c| c.slot == slot && profile.unlocked.contains(&c.id))
            {
                profile.equipped.insert(slot, c.id);
            }
        }
        profile
    }

    pub fn level(&self) -> u32 {
        level_for_xp(self.xp)
    }

    fn meets(&self, unlock: Unlock) -> bool {
        match unlock {
            Unlock::Level(l) => self.level() >= l,
            Unlock::Wins(w) => self.wins >= w,
        }
    }

    /// Recompute which cosmetics are unlocked from the current level/wins.
    fn refresh_unlocks(&mut self) -> Vec<u16> {
        let mut newly = Vec::new();
        for c in catalog() {
            if self.meets(c.unlock) && self.unlocked.insert(c.id) {
                newly.push(c.id);
            }
        }
        newly
    }

    /// Record a finished match for the local team and apply progression. Returns the
    /// cosmetics newly unlocked by this match.
    pub fn award_match(&mut self, placement: Option<u8>) -> Vec<u16> {
        self.matches_played += 1;
        if placement == Some(1) {
            self.wins += 1;
        }
        self.xp += placement_xp(placement);
        let newly = self.refresh_unlocks();
        self.last_event = match placement {
            Some(1) => format!("Won the match! +{} XP.", placement_xp(placement)),
            Some(n) => format!("Placed {n} — +{} XP.", placement_xp(placement)),
            None => format!("Absorbed — +{} XP.", placement_xp(placement)),
        };
        if !newly.is_empty() {
            let names: Vec<&str> = newly
                .iter()
                .filter_map(|id| cosmetic(*id))
                .map(|c| c.name)
                .collect();
            self.last_event = format!("{} Unlocked: {}.", self.last_event, names.join(", "));
        }
        newly
    }

    pub fn is_unlocked(&self, id: u16) -> bool {
        self.unlocked.contains(&id)
    }

    pub fn is_equipped(&self, id: u16) -> bool {
        cosmetic(id).is_some_and(|c| self.equipped.get(&c.slot) == Some(&id))
    }

    /// Equip a cosmetic; only an unlocked one, into its slot. Returns success.
    pub fn equip(&mut self, id: u16) -> bool {
        let Some(c) = cosmetic(id) else {
            return false;
        };
        if !self.is_unlocked(id) {
            self.last_event = format!("{} is locked.", c.name);
            return false;
        }
        self.equipped.insert(c.slot, id);
        self.last_event = format!("Equipped {} ({}).", c.name, c.slot.label());
        true
    }

    /// Serialize to a compact, deterministic save string.
    pub fn serialize(&self) -> String {
        let unlocked: Vec<String> = self.unlocked.iter().map(|id| id.to_string()).collect();
        let equipped: Vec<String> = self
            .equipped
            .iter()
            .map(|(slot, id)| format!("{}:{}", *slot as u8, id))
            .collect();
        format!(
            "v1;xp={};played={};wins={};unlocked={};equipped={}",
            self.xp,
            self.matches_played,
            self.wins,
            unlocked.join(","),
            equipped.join(",")
        )
    }

    /// Parse a save string. Returns `None` on malformed input.
    pub fn parse(save: &str) -> Option<Self> {
        let mut xp = None;
        let mut played = None;
        let mut wins = None;
        let mut unlocked = BTreeSet::new();
        let mut equipped = BTreeMap::new();
        let body = save.strip_prefix("v1;")?;
        for field in body.split(';') {
            let (key, value) = field.split_once('=')?;
            match key {
                "xp" => xp = Some(value.parse().ok()?),
                "played" => played = Some(value.parse().ok()?),
                "wins" => wins = Some(value.parse().ok()?),
                "unlocked" => {
                    for id in value.split(',').filter(|s| !s.is_empty()) {
                        unlocked.insert(id.parse().ok()?);
                    }
                }
                "equipped" => {
                    for pair in value.split(',').filter(|s| !s.is_empty()) {
                        let (slot_raw, id_raw) = pair.split_once(':')?;
                        let slot = match slot_raw.parse::<u8>().ok()? {
                            0 => Slot::Color,
                            1 => Slot::Trail,
                            2 => Slot::Badge,
                            _ => return None,
                        };
                        equipped.insert(slot, id_raw.parse().ok()?);
                    }
                }
                _ => return None,
            }
        }
        Some(Self {
            xp: xp?,
            matches_played: played?,
            wins: wins?,
            unlocked,
            equipped,
            last_event: "Loaded profile.".to_string(),
        })
    }

    /// Two profiles are equal in *state* (ignoring the cosmetic last-event text).
    pub fn same_state(&self, other: &Self) -> bool {
        self.xp == other.xp
            && self.matches_played == other.matches_played
            && self.wins == other.wins
            && self.unlocked == other.unlocked
            && self.equipped == other.equipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earning_xp_levels_up_and_unlocks_cosmetics() {
        let mut p = Profile::new();
        assert_eq!(p.level(), 0);
        // Level-0 defaults are unlocked from the start.
        assert!(p.is_unlocked(0) && p.is_unlocked(4) && p.is_unlocked(7));
        assert!(!p.is_unlocked(1), "Ember (level 2) starts locked");

        // Win several matches; level rises and level-gated cosmetics unlock.
        for _ in 0..3 {
            p.award_match(Some(1));
        }
        assert!(p.level() >= 2, "got level {}", p.level());
        assert!(p.is_unlocked(5), "Spark (level 1) unlocked");
        assert!(p.is_unlocked(1), "Ember (level 2) unlocked");
    }

    #[test]
    fn win_count_unlocks_win_gated_cosmetics() {
        let mut p = Profile::new();
        assert!(!p.is_unlocked(9), "Champion (1 win) starts locked");
        p.award_match(Some(1));
        assert_eq!(p.wins, 1);
        assert!(p.is_unlocked(9), "Champion unlocks after a win");
    }

    #[test]
    fn placement_awards_more_xp_for_better_finishes() {
        assert!(placement_xp(Some(1)) > placement_xp(Some(2)));
        assert!(placement_xp(Some(2)) > placement_xp(Some(3)));
        assert!(placement_xp(Some(3)) > placement_xp(None));
    }

    #[test]
    fn equipping_requires_an_unlock() {
        let mut p = Profile::new();
        assert!(!p.equip(2), "cannot equip locked Cobalt");
        assert!(p.equip(0), "can equip unlocked Ash");
        assert!(p.is_equipped(0));
        // Equipping another color replaces the slot.
        for _ in 0..6 {
            p.award_match(Some(1));
        }
        assert!(p.equip(1));
        assert!(p.is_equipped(1) && !p.is_equipped(0));
    }

    #[test]
    fn the_profile_serializes_and_round_trips() {
        let mut p = Profile::new();
        for _ in 0..4 {
            p.award_match(Some(2));
        }
        p.award_match(Some(1));
        p.equip(1);
        let save = p.serialize();
        let loaded = Profile::parse(&save).expect("valid save parses");
        assert!(p.same_state(&loaded), "round-trip preserves state");
        assert_eq!(save, loaded.serialize(), "re-serialization is stable");
    }

    #[test]
    fn malformed_saves_are_rejected() {
        assert!(Profile::parse("garbage").is_none());
        assert!(Profile::parse("v1;xp=notanumber").is_none());
        assert!(Profile::parse("v2;xp=10").is_none());
    }

    #[test]
    fn awarding_is_deterministic() {
        let mut a = Profile::new();
        let mut b = Profile::new();
        for placement in [Some(1), Some(2), None, Some(1), Some(3)] {
            a.award_match(placement);
            b.award_match(placement);
        }
        assert!(a.same_state(&b));
    }
}
