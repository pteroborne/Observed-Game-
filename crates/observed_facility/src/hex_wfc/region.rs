//! Regions: the areas a facility is *made of*, and where they are meant to meet.
//!
//! A district today is a colour. [`super::relayout::district_of`] answers which
//! [`ArchitectureRegister`] owns a cell, `initial_architecture` paints the
//! solved lattice with it, and nothing else consults it — a district cannot
//! hold a rule, decide what may be built inside it, or say where its edge is.
//! So the facility has visual regions and no structural ones.
//!
//! That gap is measurable, and it is the reason the room graph is a clique. A
//! survey of 24 production seeds found 22 carrying a single hall component with
//! all thirty rooms hanging off it: connectivity is not scarce, it is total, and
//! "which rooms connect" therefore carries no information. Neither existing
//! control can change that. The composition profile only scales weights and by
//! its own first invariant can never remove a variant, so it makes connective
//! fabric rarer and never absent. Authored pins arrive too late — both
//! `stamp_blueprints_with_pins` and `forced_route_edges` run before
//! `pins::resolved_pins` and are blind to it, so a pinned wall contradicts
//! geometry that is already committed and burns the whole retry budget.
//!
//! This module is the missing vocabulary, and only the vocabulary. It answers
//! two questions about a facility that has not been solved yet:
//!
//! - **What are the regions?** Derived from the district anchors, so they
//!   inherit the property that matters: `district_sites` excludes relayout
//!   generation deliberately, so a region is a function of `(seed, config)`
//!   alone and cannot drift under a player mid-match.
//! - **Where do two regions meet?** The [`Gateway`] cells on their shared
//!   frontier — the places a crossing between them could legitimately be, as
//!   opposed to the whole boundary being equally crossable, which is what
//!   "mush" means concretely.
//!
//! Nothing here is wired into the solve, and that is deliberate. Constraining
//! stamping or routing by a region plan re-pins every seed and moves the
//! content hash, so it wants its own decision, its own evidence, and its own
//! sequencing against Arc T. What this buys first is the ability to *measure*
//! how permeable the regions currently are, which is the number any such change
//! would have to move.

use std::collections::{BTreeMap, BTreeSet};

use observed_content::ArchitectureRegister;
use observed_hex::HexCoord;

use super::relayout::{district_of, district_sites};
use super::{HexFace, HexWfcConfig};

/// One region: a register's territory through the whole height of the facility.
///
/// This was per level once, on the reasoning that `district_of` resolves against
/// anchors on the cell's own level, so the same register on two floors is two
/// territories and merging them names something discontiguous. True, and it made
/// the regions useless: a survey found that sealing all but nine lateral
/// gateways still left one corridor running through 99 of the hundred regions
/// and all ten floors, because a per-level region has no vertical boundary at
/// all. Every shaft and ramp was a hole straight through it, and no rule
/// attached to a lateral frontier could reach one.
///
/// So a region is a volume. `cells` is therefore **not** contiguous in general -
/// a register can own unrelated pockets on different floors - and the thing that
/// makes the type useful is not contiguity but that a vertical step out of a
/// region is now a [`Gateway`] like any other, and can be counted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Region {
    pub register: ArchitectureRegister,
    pub cells: BTreeSet<HexCoord>,
}

impl Region {
    /// The region's identity, independent of how many cells it holds.
    #[must_use]
    pub const fn key(&self) -> RegionKey {
        RegionKey {
            register: self.register,
        }
    }
}

/// A region's stable name.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RegionKey {
    pub register: ArchitectureRegister,
}

/// Where two regions touch.
///
/// `frontier` is every cell pair across the shared edge, which is the *whole*
/// boundary rather than a chosen crossing. Narrowing it to a few intended
/// crossings is the decision a later stage would make; naming the candidates is
/// what makes that decision expressible at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gateway {
    pub a: RegionKey,
    pub b: RegionKey,
    /// `(cell in a, cell in b)`, ordered so the pair is stable.
    pub frontier: Vec<(HexCoord, HexCoord)>,
}

/// The regions of a facility and the frontiers between them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionPlan {
    pub regions: Vec<Region>,
    pub gateways: Vec<Gateway>,
}

impl RegionPlan {
    #[must_use]
    pub fn region(&self, key: RegionKey) -> Option<&Region> {
        self.regions.iter().find(|region| region.key() == key)
    }

    /// The gateway between two regions, in either order.
    #[must_use]
    pub fn gateway(&self, a: RegionKey, b: RegionKey) -> Option<&Gateway> {
        self.gateways.iter().find(|gateway| {
            (gateway.a == a && gateway.b == b) || (gateway.a == b && gateway.b == a)
        })
    }
}

/// Derive the regions and frontiers for a facility.
///
/// Pure in `(seed, config)` and free of the attempt RNG, so it can be computed
/// before a solve, after one, or at any relayout generation and will agree every
/// time. That is what makes it usable as a contract rather than as a report:
/// stamped anchors are pinned across a relayout, so a plan derived this way does
/// not move when the geometry around it does.
#[must_use]
pub fn region_plan(seed: u64, config: HexWfcConfig) -> RegionPlan {
    let sites = district_sites(seed, config);
    let grid = config.grid();

    let mut owners: BTreeMap<HexCoord, RegionKey> = BTreeMap::new();
    let mut regions: BTreeMap<RegionKey, BTreeSet<HexCoord>> = BTreeMap::new();
    for level in 0..config.levels {
        for r in 0..config.rows {
            for q in 0..config.cols {
                let coord = HexCoord { q, r, level };
                let Some(register) = district_of(coord, &sites) else {
                    continue;
                };
                let key = RegionKey { register };
                owners.insert(coord, key);
                regions.entry(key).or_default().insert(coord);
            }
        }
    }

    // Frontiers: every adjacency, lateral or vertical, whose two cells belong to
    // different regions.
    //
    // Vertical faces are included, and that is the whole point of the region
    // being a volume. While regions were per-level a vertical neighbour was
    // always a different region, so counting those would have made every cell a
    // frontier; excluding them instead left the vertical direction completely
    // unpoliced, which is what a survey caught. Now a vertical step is interior
    // when the floor above belongs to the same register and a gateway when it
    // does not, which is the same rule the lateral direction already followed.
    let mut frontiers: BTreeMap<(RegionKey, RegionKey), Vec<(HexCoord, HexCoord)>> =
        BTreeMap::new();
    for (&coord, &key) in &owners {
        for face in HexFace::ALL {
            let Some(neighbor) = grid.neighbor(coord, face) else {
                continue;
            };
            let Some(&other) = owners.get(&neighbor) else {
                continue;
            };
            if other == key || key > other {
                // `key > other` keeps one entry per unordered pair.
                continue;
            }
            frontiers
                .entry((key, other))
                .or_default()
                .push((coord, neighbor));
        }
    }

    RegionPlan {
        regions: regions
            .into_iter()
            .map(|(key, cells)| Region {
                register: key.register,
                cells,
            })
            .collect(),
        gateways: frontiers
            .into_iter()
            .map(|((a, b), frontier)| Gateway { a, b, frontier })
            .collect(),
    }
}
