//! Does the authored catalog cover what the solver can ask for?
//!
//! A match dies at load with `MissingTile` when the solver places a cell the
//! catalog has no geometry for. That failure is cheap to prevent and expensive
//! to discover, so this module answers it while an author is still tuning.
//!
//! # Why this mirrors the projector instead of inventing a rule
//!
//! [`observed_match::hex_wfc::geometry`] resolves a cell to a tile by a specific
//! rule: the register comes from `world.architecture`, except for `stair_tower`
//! which resolves at level 0 so a whole column agrees; then the lookup is exact
//! `(archetype, register, signature)`, falling back to
//! `(archetype, "generic", signature)`, and failing otherwise.
//!
//! [`resolve`] reproduces that rule deliberately, because a coverage report that
//! reasoned differently from the projector would confidently disagree with the
//! thing it is predicting. To keep the mirror honest, [`CoverageReport::build`]
//! also records what the *real* projector returned, and a test asserts the two
//! never disagree.

use std::collections::BTreeMap;

use observed_authoring::{RoomPrototype, TilePrototype};
use observed_facility::hex_wfc::{
    HexCoord, HexWfcWorld, PortSignature, blueprint_for_role, placement_tile_archetype,
};
use observed_match::hex_wfc::{HexGeometryError, HexWfcGeometrySnapshot};

/// How a demanded `(archetype, register, signature)` is satisfied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Supply {
    /// A tile authored for exactly this register.
    Exact,
    /// Only the shared `generic` kit covers it. Legal, but the district has no
    /// geometry of its own here.
    GenericFallback,
    /// Nothing covers it. A match placing this cell fails to load.
    Missing,
}

impl Supply {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Exact => "ok",
            Self::GenericFallback => "generic",
            Self::Missing => "MISSING",
        }
    }
}

/// One distinct thing the live layout asks the catalog for.
#[derive(Clone, Debug)]
pub struct DemandRow {
    pub archetype: &'static str,
    pub register: String,
    pub signature: PortSignature,
    pub supply: Supply,
    /// Cells in the current layout that want this.
    pub cells: u32,
    /// One of them, so the author can go look.
    pub example: HexCoord,
}

/// Whether the real projector accepted this layout. The authoritative answer;
/// everything else here is prediction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectorVerdict {
    Projected,
    Failed(String),
    /// The catalog itself could not be loaded, so there was nothing to project.
    CorpusUnavailable,
}

/// Per-role room-module variety.
///
/// Every committed room module is `archetype: "sanctuary"`, `variant: 0`, one
/// per role, so every Decision room in every district is the same room. That is
/// `docs/bug_backlog.md` #25 in one line, and it is a first-class row here
/// rather than a footnote.
#[derive(Clone, Debug)]
pub struct RoomVarietyRow {
    pub role: &'static str,
    /// Runtime prototypes for this role. Not a variety measure on its own:
    /// `runtime_catalog` expands one authored module with `register_scope:
    /// ["all"]` into one prototype per register, so a role backed by a single
    /// `.map` still reports ten.
    pub prototypes: usize,
    /// Distinct authored modules (by source id) behind those prototypes. *This*
    /// is the variety number — how many different rooms an author drew.
    pub modules: usize,
    pub registers: usize,
}

impl RoomVarietyRow {
    /// One authored module means the same room every time, everywhere.
    #[must_use]
    pub fn is_thin(&self) -> bool {
        self.modules <= 1
    }
}

/// What the catalog holds that the solver never asks for.
#[derive(Clone, Debug, Default)]
pub struct NeverPlaced {
    /// Weight zero: present in the catalog and unselectable by construction.
    pub zero_weight: Vec<String>,
    /// Present, selectable, and not chosen anywhere in the current layout.
    pub unused_here: usize,
    pub corpus_cells: usize,
}

#[derive(Clone, Debug, Default)]
pub struct CoverageReport {
    pub demanded: Vec<DemandRow>,
    pub unmet: Vec<DemandRow>,
    pub room_variety: Vec<RoomVarietyRow>,
    pub never_placed: NeverPlaced,
    pub verdict: Option<ProjectorVerdict>,
}

/// The projector's lookup index, rebuilt here so coverage and projection agree.
struct SupplyIndex<'a> {
    by_key: BTreeMap<(&'a str, &'a str, PortSignature), Vec<&'a TilePrototype>>,
}

impl<'a> SupplyIndex<'a> {
    fn new(prototypes: &'a [TilePrototype]) -> Self {
        let mut by_key: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for prototype in prototypes {
            by_key
                .entry((
                    prototype.key.archetype.as_str(),
                    prototype.key.register.as_str(),
                    prototype.signature,
                ))
                .or_default()
                .push(prototype);
        }
        Self { by_key }
    }

    /// Exact register, then the shared `generic` kit — the projector's order.
    fn resolve(&self, archetype: &str, register: &str, signature: PortSignature) -> Supply {
        if self.by_key.contains_key(&(archetype, register, signature)) {
            Supply::Exact
        } else if self.by_key.contains_key(&(archetype, "generic", signature)) {
            Supply::GenericFallback
        } else {
            Supply::Missing
        }
    }
}

/// The register a cell resolves against.
///
/// `stair_tower` resolves at level 0 so an entire column agrees on one tower
/// shape; picking per cell would let a flight top out under the deck above.
fn register_for(world: &HexWfcWorld, coord: HexCoord, archetype: &str) -> Option<String> {
    let identity = if archetype == "stair_tower" {
        HexCoord { level: 0, ..coord }
    } else {
        coord
    };
    world
        .architecture
        .get(&identity)
        .map(|register| register.slug().to_string())
}

impl CoverageReport {
    /// Build the report for a solved layout against a corpus.
    #[must_use]
    pub fn build(
        world: &HexWfcWorld,
        cells: &[TilePrototype],
        rooms: &[RoomPrototype],
        projected: Option<&HexWfcGeometrySnapshot>,
        projection_error: Option<&HexGeometryError>,
    ) -> Self {
        let index = SupplyIndex::new(cells);
        let mut grouped: BTreeMap<(&'static str, String, PortSignature), (Supply, u32, HexCoord)> =
            BTreeMap::new();

        for (coord, placement) in &world.placements {
            // `None` means the cell is not per-cell geometry: Void, or a Room
            // that a stamped blueprint supplies instead.
            let Some(archetype) = placement_tile_archetype(placement) else {
                continue;
            };
            let Some(register) = register_for(world, *coord, archetype) else {
                continue;
            };
            let signature = placement_signature(placement);
            let supply = index.resolve(archetype, &register, signature);
            let entry = grouped
                .entry((archetype, register, signature))
                .or_insert((supply, 0, *coord));
            entry.1 += 1;
        }

        let demanded: Vec<DemandRow> = grouped
            .into_iter()
            .map(
                |((archetype, register, signature), (supply, cells, example))| DemandRow {
                    archetype,
                    register,
                    signature,
                    supply,
                    cells,
                    example,
                },
            )
            .collect();

        let unmet = demanded
            .iter()
            .filter(|row| row.supply == Supply::Missing)
            .cloned()
            .collect();

        let verdict = match (projected, projection_error) {
            (Some(_), _) => Some(ProjectorVerdict::Projected),
            (None, Some(error)) => Some(ProjectorVerdict::Failed(format!("{error:?}"))),
            (None, None) => None,
        };

        Self {
            demanded,
            unmet,
            room_variety: room_variety(rooms),
            never_placed: never_placed(cells, projected),
            verdict,
        }
    }

    /// Rows an author should act on, worst first.
    #[must_use]
    pub fn headline(&self) -> String {
        if !self.unmet.is_empty() {
            return format!("{} demand(s) UNCOVERED", self.unmet.len());
        }
        let thin = self.room_variety.iter().filter(|row| row.is_thin()).count();
        let generic = self
            .demanded
            .iter()
            .filter(|row| row.supply == Supply::GenericFallback)
            .count();
        format!("covered; {generic} on generic fallback, {thin} thin room role(s)")
    }
}

/// The port signature a placement presents, in the projector's own terms.
fn placement_signature(placement: &observed_facility::hex_wfc::HexPlacement) -> PortSignature {
    use observed_facility::hex_wfc::{HexFace, PortClass};
    let mut ports = [PortClass::Sealed; 8];
    for face in HexFace::LATERAL {
        if placement.is_open(face) {
            ports[face.index()] = PortClass::Door;
        }
    }
    ports[HexFace::Up.index()] = placement.up;
    ports[HexFace::Down.index()] = placement.down;
    PortSignature::try_from_ports(ports).unwrap_or(PortSignature(0))
}

fn room_variety(rooms: &[RoomPrototype]) -> Vec<RoomVarietyRow> {
    use observed_facility::map_spec::RoomRole;

    const ROLES: [RoomRole; 11] = [
        RoomRole::Start,
        RoomRole::Exit,
        RoomRole::Decision,
        RoomRole::DecoherenceFork,
        RoomRole::AnchorCheckpoint,
        RoomRole::TeleportRelay,
        RoomRole::Keystone,
        RoomRole::DualStation,
        RoomRole::GuardianControl,
        RoomRole::Monitor,
        RoomRole::Recovery,
    ];

    ROLES
        .into_iter()
        .map(|role| {
            let name = blueprint_for_role(role).name;
            let matching: Vec<&RoomPrototype> = rooms
                .iter()
                .filter(|room| room.room_role.eq_ignore_ascii_case(name))
                .collect();
            let mut registers: Vec<&str> = matching
                .iter()
                .map(|room| room.key.register.as_str())
                .collect();
            registers.sort_unstable();
            registers.dedup();
            // The source id survives register expansion, so distinct ids count
            // distinct authored rooms rather than distinct copies of one.
            let mut modules: Vec<&str> = matching.iter().map(|room| room.id.as_str()).collect();
            modules.sort_unstable();
            modules.dedup();
            RoomVarietyRow {
                role: name,
                prototypes: matching.len(),
                modules: modules.len(),
                registers: registers.len(),
            }
        })
        .collect()
}

fn never_placed(
    cells: &[TilePrototype],
    projected: Option<&HexWfcGeometrySnapshot>,
) -> NeverPlaced {
    let zero_weight: Vec<String> = cells
        .iter()
        .filter(|prototype| prototype.weight == 0)
        .map(|prototype| {
            format!(
                "{}/{}/{}",
                prototype.key.archetype, prototype.key.register, prototype.key.variant
            )
        })
        .collect();

    let used: usize = projected.map_or(0, |snapshot| {
        let mut keys: Vec<&observed_authoring::TileKey> = snapshot
            .pieces
            .iter()
            .filter_map(|piece| piece.tile.as_ref())
            .collect();
        keys.sort_by(|a, b| {
            (&a.archetype, &a.register, a.variant).cmp(&(&b.archetype, &b.register, b.variant))
        });
        keys.dedup();
        keys.len()
    });

    NeverPlaced {
        zero_weight,
        unused_here: cells.len().saturating_sub(used),
        corpus_cells: cells.len(),
    }
}
